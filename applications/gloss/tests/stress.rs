//! Autopilot: drive the whole app hard and check it stays lean.
//!
//! Three bugs once showed up here as the same symptom — the app wedging under
//! fast clicks — and none of them were reachable from a scripted test, because
//! each needed a *specific unlucky interleaving* rather than a specific click:
//! a `Process` pipe deadlock on notes larger than the pipe buffer, a stale
//! title draft that turned every switch into a rename, and full-window clip
//! masks allocated per clipped element per frame.
//!
//! So this doesn't script clicks. It hands the harness a fake `gloss` CLI
//! (§12.6: fixtures, never a real binary) and lets the autopilot press whatever
//! the view offers, resolving effects in shuffled order (§12.3) — which is what
//! reaches the interleavings. The invariants below are the part that matters:
//! whatever it does, the app settles, and nothing grows without bound.
//!
//! `soak_with_real_subprocesses` is the same shape against the real runtime.
//! It is `#[ignore]`d — §12.6 keeps real binaries out of the default suite —
//! and exists because the pipe deadlock lived in the effects runtime itself,
//! where no fixture can see it.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use gloss::{Gloss, STORE_KEY};
use gloss_core::{Op, Patch};
use libra_core::{Effect, FolderPick, KvResult, ProcessResult, ProcessStatus};
use libra_test::{Budget, Harness, Reply};

mod common;

const NOTES: usize = 443;
const ROOT: &str = "/tmp/gloss-autopilot";

/// An in-memory stand-in for `gloss-cli`, shaped like a working vault: notes
/// across 13 areas three levels deep, mostly a few hundred bytes, with 64 KB-
/// class outliers — the size that used to overflow a subprocess pipe buffer.
///
/// It *models* the vault rather than stubbing responses, so `list` reflects
/// what `apply` did. That is what makes "no note was renamed or lost" a real
/// assertion after a run that pressed rename a few dozen times.
struct FakeVault {
    notes: BTreeMap<String, String>,
    /// Renames applied. The rename-cascade bug fired one of these per note
    /// *selection*, so this number against the step count is what catches it.
    renames: usize,
    creates: usize,
    /// Renames whose destination already existed. `fs::rename` clobbers, so
    /// the real CLI loses a note here too — worth counting, not asserting on.
    clobbers: usize,
}

impl FakeVault {
    fn new(n: usize) -> FakeVault {
        let mut notes = BTreeMap::new();
        for i in 0..n {
            let path = format!("{:02} - Area{}/sub{}/note{i}.md", i % 13, i % 13, i % 3);
            let lines = match i % 10 {
                9 => 900,
                8 => 20,
                _ => 7,
            };
            let mut body = format!("---\ncreated: 2026-04-08\ntags: []\n---\n\n# note{i}\n\n");
            for l in 0..lines {
                // Wikilinks every 20th line. A note that linked on every line
                // would be a link farm, and the graph panel's cost would track
                // the fixture's oddity instead of the app's behaviour.
                if l % 20 == 0 {
                    body.push_str(&format!("See [[note{}]] for context.\n", (l + i) % n.max(1)));
                }
                body.push_str(&format!(
                    "Line {l} with **bold**, [a link](https://x.test) and some prose.\n"
                ));
            }
            notes.insert(path, body);
        }
        FakeVault {
            notes,
            renames: 0,
            creates: 0,
            clobbers: 0,
        }
    }

    fn run(&mut self, args: &[String]) -> ProcessResult {
        let flag = |name: &str| {
            args.iter()
                .position(|a| a == name)
                .and_then(|i| args.get(i + 1))
                .cloned()
                .unwrap_or_default()
        };
        let stdout = match args.first().map(String::as_str) {
            Some("list") => self.notes.keys().cloned().collect::<Vec<_>>().join("\n"),
            Some("cat") => self.notes.get(&flag("--path")).cloned().unwrap_or_default(),
            // Backlinks are a read the graph panel makes; any stable answer
            // keeps the panel's state machine moving.
            Some("backlinks") => self.notes.keys().take(3).cloned().collect::<Vec<_>>().join("\n"),
            Some("apply") => {
                self.apply(&flag("--patch-json"));
                String::new()
            }
            Some("undo") => String::new(),
            _ => String::new(),
        };
        ProcessResult {
            status: ProcessStatus::Exited(0),
            stdout: stdout.into_bytes(),
            stderr: vec![],
            finished_at_unix_ms: 1,
        }
    }

    /// Apply the ops gloss encodes into `--patch-json`, so writes are visible
    /// to later reads. Deserializing gloss's own `Patch` keeps this honest:
    /// if the patch format changes, this stops compiling instead of silently
    /// ignoring every write.
    fn apply(&mut self, json: &str) {
        let Ok(patch) = serde_json::from_str::<Patch>(json) else {
            return;
        };
        for op in patch.ops {
            match op {
                Op::CreateNote { path, content, .. } => {
                    self.creates += 1;
                    self.notes.insert(path.as_str().to_string(), content);
                }
                Op::RenameNote { note, to, .. } => {
                    self.renames += 1;
                    if let Some(body) = self.notes.remove(note.path.as_str()) {
                        if self.notes.insert(to.as_str().to_string(), body).is_some() {
                            self.clobbers += 1;
                        }
                    }
                }
                Op::ReplaceSpan { note, text, .. } => {
                    self.notes.insert(note.path.as_str().to_string(), text);
                }
                Op::DeleteNote { note } => {
                    self.notes.remove(note.path.as_str());
                }
                Op::SetFrontmatter { .. } => {}
            }
        }
    }
}

/// A harness whose effects are answered by an in-memory vault. Returns the
/// vault too, so assertions can read what the run did to it.
fn autopiloted(seed: u64) -> (Harness<Gloss>, Rc<RefCell<FakeVault>>) {
    let vault = Rc::new(RefCell::new(FakeVault::new(NOTES)));
    let fake = Rc::clone(&vault);
    let h = Harness::<Gloss>::with_view_dir(gloss::VIEW, &common::view_dir(), 1280, 800)
        .seed(seed)
        .responder(move |effect| match effect {
            Effect::Process { command, .. } => Reply::Process(fake.borrow_mut().run(&command.args)),
            // The startup read of persisted workspace settings. Answering it
            // with a saved vault means the run starts in a loaded app instead
            // of spending its steps hunting for the folder picker.
            Effect::KvGet { key, .. } if key == STORE_KEY => Reply::KvGet(Some(
                serde_json::to_vec(&serde_json::json!({
                    "last_path": ROOT,
                    "vaults": [{ "path": ROOT, "name": "autopilot" }],
                }))
                .expect("workspace json"),
            )),
            Effect::KvGet { .. } => Reply::KvGet(None),
            Effect::KvSet { .. } => Reply::KvSet(KvResult::Ok),
            Effect::KvScan { .. } => Reply::KvScan(vec![]),
            Effect::PickFolder { .. } => Reply::FolderPick(FolderPick::Picked(ROOT.into())),
            Effect::Timer { .. } => Reply::Timer,
            _ => Reply::Drop,
        });
    (h, vault)
}

#[test]
fn autopilot_keeps_the_app_lean() {
    let (mut h, vault) = autopiloted(0xC0FFEE);

    let run = h
        .autopilot(500)
        // The folder sheet resolves to the same root every time, so pressing
        // it is a no-op that crowds out real coverage.
        .avoid_handlers(["browseVault"])
        // Deliberately unlike any note title — see the draft invariant below.
        .typing("zz-autopilot")
        .invariant("no I/O turn is left hanging", |s| s.io == gloss::IoKind::Idle)
        .invariant("no work left queued", |s| s.pending == gloss::Pending::None)
        // The title field is a draft *of the selected note*. Left to lag the
        // selection while a cat is in flight, it holds the *previous* note's
        // stem — so the next click sees draft != stem and files a rename,
        // which is how clicking quickly renamed notes to each other's titles.
        //
        // Every step, not just at rest: the cat landing repairs the draft, so
        // a settled check never sees the window the next click acts inside.
        // Excused whenever a title edit is genuinely in progress: the field has
        // focus, or the rename it produced is queued or running. In those
        // states the draft is the user's, not a stale pointer at another note.
        .always_at("title draft tracks the selection", |s, step| {
            let editing = step.focused == Some("note-title")
                || s.rename_to.is_some()
                || matches!(s.pending, gloss::Pending::Rename(_));
            editing
                || s.selected_path.is_empty()
                || s.title_draft == gloss::file_stem(&s.selected_path)
        })
        // Every one of these is a list the app appends to. Unbounded growth in
        // any of them is the leak that outlives a frame.
        .bounded("history_back", 2_000, |s| s.history_back.len())
        .bounded("history_forward", 2_000, |s| s.history_forward.len())
        .bounded("open_tabs", 100, |s| s.open_tabs.len())
        .bounded("bookmarks", 1_000, |s| s.bookmarks.len())
        .bounded("expanded", 5_000, |s| s.expanded.len())
        // Rows are projections of the vault, so they track it rather than the
        // number of clicks. A rename cascade shows up here first.
        .bounded("file_rows", NOTES * 4, |s| s.file_rows.len())
        // The graph panel renders one row per link. A note with hundreds is
        // what turned a click into a multi-second frame while this was being
        // written, so the row count is the guard, not the frame time.
        .bounded("outgoing_rows", 200, |s| s.outgoing_rows.len())
        .bounded("backlink_rows", 200, |s| s.backlink_rows.len())
        // Generous on purpose: wall-clock, and §22 budgets assume a pinned
        // machine. This catches a stall in kind — the clip-mask regression put
        // single frames past a second — not a few ms of drift.
        .budget(Budget::new().p95_ms(150.0).max_ms(1_500.0))
        .run();

    let vault = vault.borrow();
    println!(
        "[autopilot] {run} · {} renames, {} creates, {} clobbers, {} notes",
        vault.renames,
        vault.creates,
        vault.clobbers,
        vault.notes.len()
    );
    assert!(
        run.steps > 100,
        "autopilot ran out of affordances after {} steps — the view collapsed",
        run.steps
    );
    // The bug this file exists for renamed a note on *every selection*, so a
    // run that mostly clicks around produced renames by the hundred. Renames
    // should track deliberate title edits, which are a small share of steps.
    assert!(
        vault.renames * 10 < run.steps,
        "{} renames in {} steps — switching notes is renaming them",
        vault.renames,
        run.steps
    );
}

/// Same run from a second seed. Cheap, and the point of seeding: one arbitrary
/// interleaving proves less than two.
#[test]
fn autopilot_is_lean_from_another_seed() {
    let (mut h, _vault) = autopiloted(0x5EED);
    let run = h
        .autopilot(250)
        .avoid_handlers(["browseVault"])
        .invariant("no I/O turn is left hanging", |s| s.io == gloss::IoKind::Idle)
        .invariant("no work left queued", |s| s.pending == gloss::Pending::None)
        .bounded("history_back", 2_000, |s| s.history_back.len())
        .bounded("open_tabs", 100, |s| s.open_tabs.len())
        .run();
    println!("[autopilot] {run}");
}

/// The same pressure against the real effects runtime and real `gloss`
/// subprocesses.
///
/// `#[ignore]`d by design: §12.6 keeps real binaries out of the default suite,
/// and this needs a built `gloss` on PATH plus tens of seconds. It stays
/// because the pipe deadlock lived *inside* the effects runtime, and a fixture
/// responder cannot see a pipe. Run with:
///
/// ```sh
/// cargo test -p gloss --test stress -- --ignored --nocapture
/// ```
#[test]
#[ignore = "real subprocesses (§12.6) — opt-in soak, see the doc comment"]
fn soak_with_real_subprocesses() {
    soak::run();
}

mod soak {
    use crate::common;
    use gloss::{Action, Gloss};
    use libra_core::{App, FrameInfo, Instant, ReflectAction, ReflectState};
    use std::sync::mpsc;
    use std::sync::Arc;

    const NOTES: usize = 443;
    const VIEWPORT: (f32, f32) = (1280.0, 800.0);
    const SCALE: f32 = 2.0;

    fn rss_mb() -> f64 {
        let out = std::process::Command::new("ps")
            .args(["-o", "rss=", "-p", &std::process::id().to_string()])
            .output()
            .expect("ps");
        String::from_utf8_lossy(&out.stdout)
            .trim()
            .parse::<f64>()
            .unwrap_or(0.0)
            / 1024.0
    }

    fn child_count() -> usize {
        let out = std::process::Command::new("pgrep")
            .args(["-P", &std::process::id().to_string()])
            .output()
            .expect("pgrep");
        String::from_utf8_lossy(&out.stdout).lines().count()
    }

    fn make_vault(dir: &std::path::Path, n: usize) {
        for i in 0..n {
            let sub = dir
                .join(format!("{:02} - Area{}", i % 13, i % 13))
                .join(format!("sub{}", i % 3));
            std::fs::create_dir_all(&sub).expect("vault dir");
            let lines = match i % 10 {
                9 => 900,
                8 => 20,
                _ => 7,
            };
            let mut body = format!("---\ncreated: 2026-04-08\ntags: []\n---\n\n# note{i}\n\n");
            for l in 0..lines {
                body.push_str(&format!(
                    "Line {l} with **bold**, [[note{}]] and [a link](https://x.test) plus prose.\n",
                    (l + i) % n.max(1)
                ));
            }
            std::fs::write(sub.join(format!("note{i}.md")), body).expect("note");
        }
    }

    fn laid_of(
        app: &Gloss,
        ast: &libra_markup::Node,
        registry: &libra_markup::ComponentRegistry,
        renderer: &libra_raster::Renderer,
        theme: &libra_draw::Theme,
    ) -> libra_components::LaidTree {
        let field = |p: &str| app.field(p);
        let has_action = |m: &str| Action::action_by_name(m).is_some();
        let has_str_action = |m: &str| Action::action_with_str(m, String::new()).is_some();
        let collection_rows = |p: &str| app.collection_rows(p);
        let has_frame_info_action =
            |m: &str| Action::action_with_frame_info(m, FrameInfo::at(0.0)).is_some();
        let has_instant_action =
            |m: &str| Action::action_with_instant(m, Instant::at(0.0)).is_some();
        let scope = libra_interp::Scope {
            field: &field,
            has_action: &has_action,
            has_str_action: &has_str_action,
            collection_rows: &collection_rows,
            has_frame_info_action: &has_frame_info_action,
            has_instant_action: &has_instant_action,
            has_stream_event_action: &|_| false,
            binding_ctx: "State",
        };
        let view = libra_interp::resolve(ast, &scope, registry).expect("resolve");
        libra_components::layout(&view, VIEWPORT, renderer, theme).expect("layout")
    }

    pub fn run() {
        // Subprocesses resolve `gloss` from PATH the way the shipped app does.
        let exe = std::path::Path::new(env!("CARGO_BIN_EXE_gloss"));
        let bin_dir = exe.parent().expect("bin dir").display().to_string();
        let path = std::env::var("PATH").unwrap_or_default();
        std::env::set_var("PATH", format!("{bin_dir}:{path}"));

        let vault = tempfile::tempdir().expect("vault");
        make_vault(vault.path(), NOTES);
        let kv_dir = tempfile::tempdir().expect("kv dir");
        let kv = libra_kv::Kv::open(&kv_dir.path().join("kv.redb")).expect("kv");

        let (tx, rx) = mpsc::channel::<Action>();
        let sink: libra_effects::Sink<Action> = Arc::new(move |m| {
            let _ = tx.send(m);
        });
        let effects = libra_effects::Effects::new(sink, Some(kv))
            .with_process_allowlist(["gloss", "gloss-cli"]);

        let (mut app, init) = Gloss::init();
        effects.execute_all(init);

        let renderer = libra_raster::Renderer::new();
        let theme = libra_draw::Theme::dark();
        let ast = libra_markup::parse(gloss::VIEW).expect("parse view");
        let registry = libra_markup::ComponentRegistry::load_from_dir(&common::view_dir())
            .expect("component registry");
        let (pw, ph) = ((VIEWPORT.0 * SCALE) as u32, (VIEWPORT.1 * SCALE) as u32);

        // One shell turn: reduce, run effects, relayout, paint, rasterize.
        let mut raster: Vec<std::time::Duration> = Vec::new();
        let turn = |app: &mut Gloss, msg: Action, raster: &mut Vec<std::time::Duration>| {
            effects.execute_all(app.update(msg));
            let laid = laid_of(app, &ast, &registry, &renderer, &theme);
            let cmds = laid.paint_full(&renderer, effects.store(), None, None, &theme, SCALE);
            let started = std::time::Instant::now();
            std::hint::black_box(renderer.render(&cmds, pw, ph));
            raster.push(started.elapsed());
        };

        turn(
            &mut app,
            Action::VaultPathChanged(vault.path().display().to_string()),
            &mut raster,
        );
        turn(&mut app, Action::OpenVault, &mut raster);

        let settle = std::time::Instant::now();
        while settle.elapsed() < std::time::Duration::from_secs(30) {
            match rx.recv_timeout(std::time::Duration::from_millis(600)) {
                Ok(msg) => turn(&mut app, msg, &mut raster),
                Err(_) => break,
            }
        }
        assert_eq!(
            app.note_paths.len(),
            NOTES,
            "vault listing did not complete — a `gloss list` subprocess stalled"
        );

        let paths: Vec<String> = app.note_paths.iter().take(10).cloned().collect();
        let baseline_rss = rss_mb();
        for _ in 0..6 {
            for p in &paths {
                // The shell fires the focused Input's onSubmit before the press.
                turn(&mut app, Action::TitleSubmitted, &mut raster);
                turn(&mut app, Action::SelectFile(p.clone()), &mut raster);
                while let Ok(msg) = rx.try_recv() {
                    turn(&mut app, msg, &mut raster);
                }
            }
            let drain = std::time::Instant::now();
            while drain.elapsed() < std::time::Duration::from_secs(30) {
                match rx.recv_timeout(std::time::Duration::from_millis(700)) {
                    Ok(msg) => turn(&mut app, msg, &mut raster),
                    Err(_) => break,
                }
            }
        }
        let growth = rss_mb() - baseline_rss;
        raster.sort();
        println!(
            "{} frames, raster p50 {:?} p95 {:?} max {:?}, rss {:.1} MB (+{growth:.1} MB)",
            raster.len(),
            raster[raster.len() / 2],
            raster[raster.len() * 95 / 100],
            raster.last().expect("frames"),
            rss_mb(),
        );

        // The selection must land on the last note clicked. A stalled pipe or a
        // rename cascade both strand it on an earlier note.
        assert_eq!(
            app.selected_path,
            *paths.last().expect("paths"),
            "selection did not keep up with the clicks (status: {:?}, io: {:?}, pending: {:?})",
            app.status,
            app.io,
            app.pending
        );
        assert_eq!(app.io, gloss::IoKind::Idle, "an I/O turn never completed");
        assert_eq!(app.pending, gloss::Pending::None, "work left queued");
        assert_eq!(app.note_paths.len(), NOTES, "a note was renamed or lost");
        assert_eq!(child_count(), 0, "leaked subprocesses");
        // Steady-state switching allocates nothing that outlives a frame. This
        // caught full-window clip masks (~4 MB each, ~90 per frame) as runaway RSS.
        assert!(
            growth < 64.0,
            "rss grew {growth:.1} MB over 60 note switches"
        );
    }
}
