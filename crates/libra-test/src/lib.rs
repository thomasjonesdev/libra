//! libra-test — the in-process test harness (ARCHITECTURE.md §15).
//!
//! Tier 2: build the real view via the real interpreter, lay it out at a
//! virtual viewport, query by semantics, dispatch through real hit
//! testing and the app's real `update`.
//!
//! Tier 3: every `Cmd` returned by `init`/`update` is intercepted into a
//! script queue instead of executed. Tests assert on them
//! (`expect_cmd`) and hand-feed results back (`resolve_*`) in any order
//! — including adversarial orders. No mocks anywhere: the cmd is data,
//! the fixture is data, the dispatch is the app's real `update`.
//!
//! Tier 4: render through the canonical executor and compare PNG bytes
//! exactly. Missing snapshot → recorded (first run); LIBRA_BLESS=1 →
//! re-record after intentional changes; otherwise any byte difference
//! fails with expected/actual/diff-heatmap paths.

// ReflectModel/ReflectMsg methods resolve through App's supertrait bounds.
use libra_core::{App, Cmd, HttpResult, KvResult, ReflectMsg, SeriesId};
use libra_data::SeriesStore;
use libra_interp::Scope;
use libra_markup::Node;
use libra_raster::Renderer;
use libra_draw::Theme;
use libra_text::{EditOp, EditState};
use libra_components::{InputFocus, LaidTree, Role};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct Harness<A: App> {
    app: A,
    ast: Node,
    renderer: Renderer,
    laid: LaidTree,
    viewport: (u32, u32),
    snapshot_dir: PathBuf,
    /// Tier 3: intercepted, unexecuted cmds in emission order.
    pending: VecDeque<Cmd<A::Msg>>,
    /// The data plane, fed synchronously (tapes) — never by threads.
    store: Arc<SeriesStore>,
    /// §10.1 focus: the focused Input's identity + editing state +
    /// its on_change handler (retained outside the tree until the
    /// reconciler brings structural IDs).
    focus: Option<(String, EditState, String)>,
    /// §11: the theme this harness renders under — part of snapshot
    /// identity (a snapshot's name should say which theme it is).
    theme: Theme,
}

#[derive(Clone, Debug)]
pub enum Selector {
    TestId(String),
    Text(String),
    Role(Role),
}

pub mod by {
    use super::Selector;
    use libra_components::Role;

    pub fn test_id(id: &str) -> Selector {
        Selector::TestId(id.to_string())
    }
    pub fn text(t: &str) -> Selector {
        Selector::Text(t.to_string())
    }
    pub fn role(r: Role) -> Selector {
        Selector::Role(r)
    }
}

/// Cmd matchers for `expect_cmd` (§15.4).
pub mod cmd {
    #[derive(Clone, Debug)]
    pub enum Matcher {
        Any,
        Http,
        /// Exact key.
        KvGet(Option<String>),
        /// Exact key.
        KvSet(Option<String>),
        KvScan,
        Timer,
        Random,
        SetTheme,
    }

    pub fn any() -> Matcher {
        Matcher::Any
    }
    pub fn http() -> Matcher {
        Matcher::Http
    }
    pub fn kv_get(key: &str) -> Matcher {
        Matcher::KvGet(Some(key.to_string()))
    }
    pub fn any_kv_get() -> Matcher {
        Matcher::KvGet(None)
    }
    pub fn kv_set(key: &str) -> Matcher {
        Matcher::KvSet(Some(key.to_string()))
    }
    pub fn any_kv_set() -> Matcher {
        Matcher::KvSet(None)
    }
    pub fn kv_scan() -> Matcher {
        Matcher::KvScan
    }
    pub fn timer() -> Matcher {
        Matcher::Timer
    }
    pub fn random() -> Matcher {
        Matcher::Random
    }
    pub fn set_theme() -> Matcher {
        Matcher::SetTheme
    }
}

fn matches<M>(matcher: &cmd::Matcher, c: &Cmd<M>) -> bool {
    use cmd::Matcher;
    match (matcher, c) {
        (Matcher::Any, _) => true,
        (Matcher::Http, Cmd::Http { .. }) => true,
        (Matcher::KvGet(want), Cmd::KvGet { key, .. }) => {
            want.as_deref().map_or(true, |w| w == key)
        }
        (Matcher::KvSet(want), Cmd::KvSet { key, .. }) => {
            want.as_deref().map_or(true, |w| w == key)
        }
        (Matcher::KvScan, Cmd::KvScan { .. }) => true,
        (Matcher::Timer, Cmd::Timer { .. }) => true,
        (Matcher::Random, Cmd::Random { .. }) => true,
        (Matcher::SetTheme, Cmd::SetTheme { .. }) => true,
        _ => false,
    }
}

impl<A: App> Harness<A> {
    /// Build from `.lx` source at a default 400×240 viewport.
    pub fn new(lx_source: &str) -> Self {
        Self::with_viewport(lx_source, 400, 240)
    }

    pub fn with_viewport(lx_source: &str, w: u32, h: u32) -> Self {
        let ast = libra_markup::parse(lx_source)
            .unwrap_or_else(|e| panic!("markup parse error: {e}"));
        let (app, init_cmds) = A::init();
        let renderer = Renderer::new();
        let theme = Theme::dark();
        let laid = rebuild(&app, &ast, &renderer, (w, h), &theme);
        Harness {
            app,
            ast,
            renderer,
            laid,
            viewport: (w, h),
            snapshot_dir: PathBuf::from(
                std::env::var("LIBRA_SNAPSHOT_DIR").unwrap_or_else(|_| "tests/snapshots".into()),
            ),
            pending: init_cmds.into(),
            store: Arc::new(SeriesStore::new()),
            focus: None,
            theme,
        }
    }

    /// Render under a named theme (§11): `.theme("libra-light")`.
    /// Panics on unknown names — a typo'd theme is a broken test.
    pub fn theme(mut self, name: &str) -> Self {
        self.theme = Theme::by_name(name)
            .unwrap_or_else(|| panic!("unknown theme '{name}' — built in: libra-dark, libra-light"));
        self.laid = rebuild(&self.app, &self.ast, &self.renderer, self.viewport, &self.theme);
        self
    }

    /// Feed a recorded tape into a series, synchronously (§14.4a): every
    /// sample lands before this returns — no threads, no clocks. The
    /// deterministic input side of chart snapshot tests.
    pub fn feed_tape(&mut self, series: SeriesId, path: impl AsRef<Path>) -> usize {
        let n = self
            .store
            .feed_tape_sync(series, path.as_ref())
            .unwrap_or_else(|e| panic!("feed_tape: {e}"));
        // Data changed: rebuild so paint sees it (widgets read at paint,
        // but a rebuild keeps laid geometry current too).
        self.laid = rebuild(&self.app, &self.ast, &self.renderer, self.viewport, &self.theme);
        n
    }

    pub fn store(&self) -> &Arc<SeriesStore> {
        &self.store
    }

    pub fn model(&self) -> &A {
        &self.app
    }

    /// Dispatch a msg directly through the real update, then rebuild.
    /// Returned cmds are intercepted into the Tier-3 queue.
    pub fn dispatch(&mut self, msg: A::Msg) {
        let cmds = self.app.update(msg);
        for cmd in &cmds {
            // SetTheme is presentation state: apply it (snapshots see
            // it) *and* queue it (Tier-3 can assert on it).
            if let Cmd::SetTheme { name } = cmd {
                if let Some(t) = Theme::by_name(name) {
                    self.theme = t;
                }
            }
        }
        self.pending.extend(cmds);
        self.laid = rebuild(&self.app, &self.ast, &self.renderer, self.viewport, &self.theme);
    }

    /// Hit-test a synthetic click at the center of the selected item and
    /// dispatch its handler through the app's real msg resolution.
    /// Clicking a TextInput focuses it (§10.1) instead of dispatching.
    pub fn click(&mut self, sel: Selector) {
        let item = self.find(&sel).unwrap_or_else(|| panic!("click: no item matches {sel:?}"));
        let (cx, cy) = item.rect.center();
        let hit = self
            .laid
            .hit(cx, cy)
            .unwrap_or_else(|| panic!("click: {sel:?} is not interactive at ({cx},{cy})"));
        let name = hit.on_press.clone().expect("hit item has a handler");
        if hit.semantics.role == Role::TextInput {
            let id = hit.semantics.test_id.clone().expect("Input has a testId (load-enforced)");
            self.focus = Some((id, EditState::new(hit.semantics.text.clone()), name));
            return;
        }
        let msg = A::Msg::msg_by_name(&name)
            .unwrap_or_else(|| panic!("handler '{name}' resolved to no Msg — reflection out of sync"));
        self.dispatch(msg);
    }

    // ------------------------------------------------------------------
    // Text entry (§10.1): the harness drives the same pure editing core
    // the shell drives.
    // ------------------------------------------------------------------

    /// Focus the selected Input and type, one char at a time — each
    /// keystroke runs the editing core and dispatches the input's
    /// onChange msg through the real update, exactly like the shell.
    pub fn type_text(&mut self, sel: Selector, text: &str) {
        self.click(sel);
        for ch in text.chars() {
            self.edit(EditOp::Insert(ch));
        }
    }

    /// Apply one editing op to the focused Input (§10.1 harness verb):
    /// text-changing ops dispatch onChange; navigation just moves the
    /// caret/selection.
    pub fn edit(&mut self, op: EditOp) {
        let Some((id, mut edit, on_change)) = self.focus.take() else {
            panic!("edit: no Input is focused — click or type_text first");
        };
        let before = edit.text().to_string();
        edit.apply(op);
        let after = edit.text().to_string();
        self.focus = Some((id.clone(), edit, on_change.clone()));
        if before != after {
            let msg = A::Msg::msg_with_str(&on_change, after)
                .unwrap_or_else(|| panic!("'{on_change}' is not a String-payload Msg variant"));
            self.dispatch(msg);
            // Controlled input: the model is the source of truth. If
            // update transformed the text, resync the editing state.
            self.sync_focus_from_model();
        }
    }

    fn sync_focus_from_model(&mut self) {
        let Some((id, edit, on_change)) = self.focus.take() else { return };
        let current = self
            .find(&Selector::TestId(id.clone()))
            .map(|i| i.semantics.text.clone())
            .unwrap_or_default();
        let edit = if current == edit.text() {
            edit
        } else {
            EditState::new(current) // model rewrote the value: caret to end
        };
        self.focus = Some((id, edit, on_change));
    }

    /// The focused Input's editing state (caret/selection assertions).
    pub fn focused_edit(&self) -> Option<&EditState> {
        self.focus.as_ref().map(|(_, e, _)| e)
    }

    pub fn blur(&mut self) {
        self.focus = None;
    }

    fn focus_paint(&self) -> Option<InputFocus> {
        self.focus.as_ref().map(|(id, e, _)| InputFocus {
            test_id: id.clone(),
            caret: e.caret(),
            selection: e.selection(),
        })
    }

    // ------------------------------------------------------------------
    // Tier 3 — cmd interception & scripted results (§15.4)
    // ------------------------------------------------------------------

    /// Remove and return the first pending cmd matching `matcher`.
    /// Panics (listing the queue) if none matches.
    pub fn expect_cmd(&mut self, matcher: cmd::Matcher) -> Cmd<A::Msg> {
        let pos = self.pending.iter().position(|c| matches(&matcher, c));
        match pos {
            Some(i) => self.pending.remove(i).expect("position is valid"),
            None => panic!(
                "expect_cmd: nothing matches {matcher:?}; pending queue: {:?}",
                self.pending
            ),
        }
    }

    /// Assert the cmd queue is fully drained — the flow produced nothing
    /// the test didn't account for.
    pub fn expect_no_cmds(&self) {
        assert!(
            self.pending.is_empty(),
            "expected no pending cmds, found: {:?}",
            self.pending
        );
    }

    pub fn pending_cmds(&self) -> impl Iterator<Item = &Cmd<A::Msg>> {
        self.pending.iter()
    }

    /// Feed an HTTP fixture into an intercepted `Cmd::Http` — constructs
    /// the msg via the cmd's own ctor and dispatches it for real.
    pub fn resolve_http(&mut self, cmd: Cmd<A::Msg>, result: HttpResult) {
        match cmd {
            Cmd::Http { on_done, .. } => self.dispatch(on_done(result)),
            other => panic!("resolve_http: not an Http cmd: {other:?}"),
        }
    }

    pub fn resolve_kv_get(&mut self, cmd: Cmd<A::Msg>, value: Option<Vec<u8>>) {
        match cmd {
            Cmd::KvGet { on_done, .. } => self.dispatch(on_done(value)),
            other => panic!("resolve_kv_get: not a KvGet cmd: {other:?}"),
        }
    }

    /// Acked sets dispatch their result; fire-and-forget sets are a no-op.
    pub fn resolve_kv_set(&mut self, cmd: Cmd<A::Msg>, result: KvResult) {
        match cmd {
            Cmd::KvSet { on_done: Some(ctor), .. } => self.dispatch(ctor(result)),
            Cmd::KvSet { on_done: None, .. } => {}
            other => panic!("resolve_kv_set: not a KvSet cmd: {other:?}"),
        }
    }

    pub fn resolve_kv_scan(&mut self, cmd: Cmd<A::Msg>, hits: Vec<(String, Vec<u8>)>) {
        match cmd {
            Cmd::KvScan { on_done, .. } => self.dispatch(on_done(hits)),
            other => panic!("resolve_kv_scan: not a KvScan cmd: {other:?}"),
        }
    }

    /// Deliver an intercepted timer immediately — tests control time.
    pub fn fire_timer(&mut self, cmd: Cmd<A::Msg>) {
        match cmd {
            Cmd::Timer { msg, .. } => self.dispatch(msg),
            other => panic!("fire_timer: not a Timer cmd: {other:?}"),
        }
    }

    pub fn resolve_random(&mut self, cmd: Cmd<A::Msg>, bytes: Vec<u8>) {
        match cmd {
            Cmd::Random { on_done, .. } => self.dispatch(on_done(bytes)),
            other => panic!("resolve_random: not a Random cmd: {other:?}"),
        }
    }

    // ------------------------------------------------------------------
    // Queries (Tier 2)
    // ------------------------------------------------------------------

    pub fn query(&self, sel: Selector) -> NodeRef<'_> {
        let item = self.find(&sel).unwrap_or_else(|| panic!("query: no item matches {sel:?}"));
        NodeRef { text: item.semantics.text.clone(), _life: std::marker::PhantomData }
    }

    fn find(&self, sel: &Selector) -> Option<&libra_components::Item> {
        self.laid.items.iter().find(|i| match sel {
            Selector::TestId(id) => i.semantics.test_id.as_deref() == Some(id.as_str()),
            Selector::Text(t) => i.semantics.text == *t,
            Selector::Role(r) => i.semantics.role == *r,
        })
    }

    // ------------------------------------------------------------------
    // Snapshots (Tier 4)
    // ------------------------------------------------------------------

    /// Byte-exact snapshot assertion (see module docs). On mismatch,
    /// writes `<name>.actual.png` and `<name>.diff.png` (heatmap: red
    /// where pixels differ, dimmed expected elsewhere).
    pub fn assert_snapshot(&self, name: &str) {
        let cmds = self.laid.paint(&self.renderer, &self.store, self.focus_paint().as_ref(), &self.theme);
        let actual = self.renderer.render_png(&cmds, self.viewport.0, self.viewport.1);
        let path = self.snapshot_dir.join(format!("{name}.png"));

        let bless = std::env::var("LIBRA_BLESS").ok().as_deref() == Some("1");
        if bless || !path.exists() {
            std::fs::create_dir_all(&self.snapshot_dir).expect("snapshot dir");
            std::fs::write(&path, &actual).expect("write snapshot");
            if bless {
                eprintln!("[libra-test] blessed snapshot '{name}' ({} bytes)", actual.len());
            } else {
                eprintln!("[libra-test] recorded new snapshot '{name}' ({} bytes)", actual.len());
            }
            return;
        }

        let expected = std::fs::read(&path).expect("read snapshot");
        if expected != actual {
            let actual_path = self.snapshot_dir.join(format!("{name}.actual.png"));
            std::fs::write(&actual_path, &actual).expect("write actual");
            let diff_note = match write_diff_heatmap(
                &expected,
                &actual,
                &self.snapshot_dir.join(format!("{name}.diff.png")),
            ) {
                Some((diff_path, changed)) => {
                    format!("\n  diff:     {} ({changed} px changed)", diff_path.display())
                }
                None => String::new(),
            };
            panic!(
                "snapshot '{name}' differs byte-for-byte\n  expected: {}\n  actual:   {}{diff_note}\n  (LIBRA_BLESS=1 to accept the new rendering)",
                path.display(),
                actual_path.display()
            );
        }
    }

    /// Raw PNG bytes of the current frame (for hashing/inspection).
    pub fn frame_png(&self) -> Vec<u8> {
        let cmds = self.laid.paint(&self.renderer, &self.store, self.focus_paint().as_ref(), &self.theme);
        self.renderer.render_png(&cmds, self.viewport.0, self.viewport.1)
    }
}

/// Failure artifact: red where pixels differ, dimmed expected elsewhere.
/// Returns the path and differing-pixel count, or None if either PNG
/// fails to decode or dimensions differ.
fn write_diff_heatmap(
    expected_png: &[u8],
    actual_png: &[u8],
    path: &std::path::Path,
) -> Option<(PathBuf, usize)> {
    let expected = tiny_skia::Pixmap::decode_png(expected_png).ok()?;
    let actual = tiny_skia::Pixmap::decode_png(actual_png).ok()?;
    if expected.width() != actual.width() || expected.height() != actual.height() {
        return None;
    }
    let mut diff = tiny_skia::Pixmap::new(expected.width(), expected.height())?;
    let (e, a, d) = (expected.pixels(), actual.pixels(), diff.pixels_mut());
    let mut changed = 0usize;
    for i in 0..e.len() {
        d[i] = if e[i] != a[i] {
            changed += 1;
            tiny_skia::PremultipliedColorU8::from_rgba(255, 0, 0, 255).expect("opaque red")
        } else {
            // Dimmed grayscale of expected, so the changed region reads
            // in context.
            let l = ((e[i].red() as u16 + e[i].green() as u16 + e[i].blue() as u16) / 12) as u8;
            tiny_skia::PremultipliedColorU8::from_rgba(l, l, l, 255).expect("opaque gray")
        };
    }
    let bytes = diff.encode_png().ok()?;
    std::fs::write(path, bytes).ok()?;
    Some((path.to_path_buf(), changed))
}

pub struct NodeRef<'a> {
    text: String,
    _life: std::marker::PhantomData<&'a ()>,
}

impl<'a> NodeRef<'a> {
    pub fn assert_text(&self, expected: &str) {
        assert_eq!(
            self.text, expected,
            "semantic text mismatch: expected {expected:?}, found {:?}",
            self.text
        );
    }
    pub fn text(&self) -> &str {
        &self.text
    }
}

fn rebuild<A: App>(
    app: &A,
    ast: &Node,
    renderer: &Renderer,
    viewport: (u32, u32),
    theme: &Theme,
) -> LaidTree {
    let field = |p: &str| app.field(p);
    let has_msg = |m: &str| A::Msg::msg_by_name(m).is_some();
    let has_str_msg = |m: &str| A::Msg::msg_with_str(m, String::new()).is_some();
    let scope = Scope { field: &field, has_msg: &has_msg, has_str_msg: &has_str_msg };
    let view = libra_interp::resolve(ast, &scope)
        .unwrap_or_else(|e| panic!("interp error: {e}"));
    libra_components::layout(&view, (viewport.0 as f32, viewport.1 as f32), renderer, theme)
        .unwrap_or_else(|e| panic!("{e}"))
}
