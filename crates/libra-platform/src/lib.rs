//! libra-platform — the winit/softbuffer windowed shell. Presentation glue
//! only: it drives the same pipeline the harness drives, then blits the frame;
//! it is not on the determinism paint path. Also handles effects dispatch (via
//! a winit user event), `.lx` hot reload, state rehydration, and a11y.
//!
//! Display loop (§12.5): while any `<Frames>` / `Sub::Frames` is mounted the
//! shell runs `ControlFlow::Poll` and fires handlers on the monitor refresh
//! schedule — the browser `requestAnimationFrame` equivalent. Apps declare
//! `<Frames onTick={…}/>`; they never manage the event loop. (Plain
//! `WaitUntil` does not wake idle macOS AppKit without input.)
//!
//! Architecture: ARCHITECTURE.md §19 (platform integration), §13.2 (state
//! snapshots), §17 (accessibility), §12.5 (sources).

pub mod a11y;

use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use libra_core::{shortcut_fires_while_focused, App, Effect, FrameInfo, Instant as LxInstant, ReflectAction, Sub};
use libra_text::{EditOp, EditState};
use libra_draw::{Color, DrawCmd, FontStyle, Rect, Theme};
use libra_effects::Effects;
use libra_interp::Scope;
use libra_kv::Kv;
use libra_markup::Node;
use libra_raster::{Damage, Renderer};
use tiny_skia::Pixmap;
use libra_components::{Cursor, InputFocus, ItemKind, LaidTree, Role, SourceDecl};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{CursorIcon, Window, WindowId};

const WATCH_INTERVAL: Duration = Duration::from_millis(250);

/// Dev-only frame pacing diagnostics (PLAN-DASHBOARD-PERF.md Stage 0).
/// Gated by `LIBRA_FRAME_STATS`; a no-op otherwise (a bool check per
/// present, no allocation when disabled). Every `FRAME_STATS_WINDOW`
/// presents, prints one stderr line with present-to-present interval
/// and present() CPU-duration percentiles — interval shows pacing
/// (should track the monitor's refresh period); present duration shows
/// raster+blit cost.
struct FrameStats {
    enabled: bool,
    last_present: Option<Instant>,
    intervals_ms: Vec<f64>,
    present_ms: Vec<f64>,
}

const FRAME_STATS_WINDOW: usize = 120;

impl FrameStats {
    fn new() -> Self {
        FrameStats {
            enabled: std::env::var_os("LIBRA_FRAME_STATS").is_some(),
            last_present: None,
            intervals_ms: Vec::with_capacity(FRAME_STATS_WINDOW),
            present_ms: Vec::with_capacity(FRAME_STATS_WINDOW),
        }
    }

    fn record(&mut self, present_dur: Duration) {
        if !self.enabled {
            return;
        }
        let now = Instant::now();
        if let Some(last) = self.last_present {
            self.intervals_ms.push((now - last).as_secs_f64() * 1000.0);
        }
        self.last_present = Some(now);
        self.present_ms.push(present_dur.as_secs_f64() * 1000.0);
        if self.present_ms.len() >= FRAME_STATS_WINDOW {
            report_frame_stats(&mut self.intervals_ms, &mut self.present_ms);
        }
    }
}

fn report_frame_stats(intervals_ms: &mut Vec<f64>, present_ms: &mut Vec<f64>) {
    intervals_ms.sort_by(f64::total_cmp);
    present_ms.sort_by(f64::total_cmp);
    let p = |xs: &[f64], q: f64| {
        xs.get(((xs.len().max(1) - 1) as f64 * q) as usize).copied().unwrap_or(0.0)
    };
    eprintln!(
        "[libra] frame p50/p95/max interval {:.2}/{:.2}/{:.2} ms · present {:.2}/{:.2}/{:.2} ms",
        p(intervals_ms, 0.5),
        p(intervals_ms, 0.95),
        intervals_ms.last().copied().unwrap_or(0.0),
        p(present_ms, 0.5),
        p(present_ms, 0.95),
        present_ms.last().copied().unwrap_or(0.0),
    );
    intervals_ms.clear();
    present_ms.clear();
}

/// How to run an app in a window.
pub struct Options {
    pub title: String,
    /// Initial inner size, logical pixels.
    pub logical_size: (f32, f32),
    /// App identity for the KV store location (§13.1). None disables
    /// persistence (Kv cmds resolve to errors/None).
    pub app_id: Option<String>,
    /// Dev mode: re-parse this `.lx` file when its mtime changes.
    pub watch: Option<PathBuf>,
    /// §11: load (and hot-reload) a TOML theme table from this path.
    pub theme_file: Option<PathBuf>,
    /// §12.6: programs the effects runtime may spawn for this app.
    pub process_allowlist: Vec<String>,
}

impl Options {
    pub fn new(title: &str, logical_size: (f32, f32)) -> Self {
        Options {
            title: title.into(),
            logical_size,
            app_id: None,
            watch: None,
            theme_file: None,
            process_allowlist: vec![],
        }
    }
    pub fn app_id(mut self, id: &str) -> Self {
        self.app_id = Some(id.into());
        self
    }
    pub fn watch(mut self, path: impl Into<PathBuf>) -> Self {
        self.watch = Some(path.into());
        self
    }
    pub fn theme_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.theme_file = Some(path.into());
        self
    }
    pub fn process_allowlist(mut self, programs: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.process_allowlist = programs.into_iter().map(Into::into).collect();
        self
    }
}

/// Run an app in a native window until it is closed (embedded view,
/// no persistence) — the Phase 0 signature, kept for simple apps.
pub fn run<A: App + 'static>(
    lx_source: &str,
    title: &str,
    logical_size: (f32, f32),
) -> Result<(), Box<dyn std::error::Error>>
where
    A::Msg: Send + 'static,
{
    run_with::<A>(lx_source, Options::new(title, logical_size))
}

pub fn run_with<A: App + 'static>(
    lx_source: &str,
    opts: Options,
) -> Result<(), Box<dyn std::error::Error>>
where
    A::Msg: Send + 'static,
{
    run_inner::<A>(lx_source, opts, None, None)
}

/// `run_with` plus §18.2 model rehydration: on exit the model snapshots
/// to `<app-data>/model.json`; the next launch restores it (then still
/// runs `init`'s cmds — startup effects are the app's declared contract
/// either way). A snapshot that no longer parses (model shape changed)
/// is discarded with a note — defaults win, the session doesn't crash.
/// Requires `app_id` (for the snapshot location).
pub fn run_rehydrating<A>(
    lx_source: &str,
    opts: Options,
) -> Result<(), Box<dyn std::error::Error>>
where
    A: App + serde::Serialize + serde::de::DeserializeOwned + 'static,
    A::Msg: Send + 'static,
{
    let app_id = opts.app_id.clone().ok_or("run_rehydrating requires Options::app_id")?;
    let path = snapshot_path(&app_id).ok_or("no home directory for model snapshot")?;

    let rehydrated: Option<A> = match std::fs::read_to_string(&path) {
        Ok(json) => match serde_json::from_str(&json) {
            Ok(model) => Some(model),
            Err(e) => {
                eprintln!("[libra] model snapshot no longer parses ({e}); starting fresh");
                None
            }
        },
        Err(_) => None, // first run
    };

    let save_path = path.clone();
    let save_hook: SaveHook<A> = Box::new(move |app: &A| match serde_json::to_string(app) {
        Ok(json) => {
            if let Some(parent) = save_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Err(e) = std::fs::write(&save_path, json) {
                eprintln!("[libra] model snapshot failed: {e}");
            }
        }
        Err(e) => eprintln!("[libra] model snapshot failed to serialize: {e}"),
    });

    run_inner::<A>(lx_source, opts, rehydrated, Some(save_hook))
}

fn snapshot_path(app_id: &str) -> Option<std::path::PathBuf> {
    directories::ProjectDirs::from("", "", app_id).map(|d| d.data_dir().join("model.json"))
}

type SaveHook<A> = Box<dyn Fn(&A)>;

fn run_inner<A: App + 'static>(
    lx_source: &str,
    opts: Options,
    rehydrated: Option<A>,
    save_hook: Option<SaveHook<A>>,
) -> Result<(), Box<dyn std::error::Error>>
where
    A::Msg: Send + 'static,
{
    let ast = libra_markup::parse(lx_source).map_err(|e| format!("markup parse error: {e}"))?;
    let registry = opts
        .watch
        .as_ref()
        .and_then(|p| p.parent())
        .map(|dir| libra_markup::ComponentRegistry::load_from_dir(dir).unwrap_or_default())
        .unwrap_or_default();
    let (app, init_cmds) = A::init();
    let app = match rehydrated {
        Some(model) => {
            eprintln!("[libra] model rehydrated from previous session");
            model
        }
        None => app,
    };

    let event_loop = EventLoop::<A::Msg>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();
    let sink: libra_effects::Sink<A::Msg> = Arc::new(move |msg| {
        // The window may be closing; a dead proxy just drops the msg.
        let _ = proxy.send_event(msg);
    });
    let kv = match &opts.app_id {
        Some(id) => match Kv::open_default(id) {
            Ok(kv) => Some(kv),
            Err(e) => {
                eprintln!("[libra] kv store unavailable ({e}); running without persistence");
                None
            }
        },
        None => None,
    };
    let a11y_sink = Arc::clone(&sink);
    let effects = Effects::new(sink, kv).with_process_allowlist(opts.process_allowlist.clone());
    effects.execute_all(init_cmds);

    let watch_mtime = opts.watch.as_deref().and_then(|p| std::fs::metadata(p).ok()?.modified().ok());
    let theme_mtime =
        opts.theme_file.as_deref().and_then(|p| std::fs::metadata(p).ok()?.modified().ok());
    let theme = match &opts.theme_file {
        Some(path) => match std::fs::read_to_string(path).map_err(|e| e.to_string()).and_then(|s| Theme::from_toml_str(&s)) {
            Ok(t) => {
                eprintln!("[libra] theme '{}' loaded from {}", t.name, path.display());
                t
            }
            Err(e) => {
                eprintln!("[libra] theme file {}: {e}; using libra-dark", path.display());
                Theme::dark()
            }
        },
        None => Theme::dark(),
    };
    // §19.2: validate the app's shortcut declarations up front — a bad
    // chord or action name is a startup error, not a silent dead key.
    let shortcut_table = libra_core::build_shortcut_table::<A::Msg>(A::shortcuts())
        .unwrap_or_else(|e| panic!("[libra] invalid shortcuts: {e}"));
    let mut shell = Shell {
        opts,
        app,
        ast,
        registry,
        effects,
        renderer: Renderer::new(),
        laid: None,
        overlay: None,
        cursor: (0.0, 0.0),
        cursor_icon: Cursor::Default,
        scroll_state: libra_components::ScrollState::new(),
        shortcut_table,
        drag: None,
        menu_anchor: (0.0, 0.0),
        gfx: None,
        watch_mtime,
        next_watch_check: Instant::now() + WATCH_INTERVAL,
        save_hook,
        frame_cache: None,
        frames_active: false,
        markup_frame_handlers: Vec::new(),
        markup_intervals: Vec::new(),
        running_intervals: Vec::new(),
        markup_streams: Vec::new(),
        focus: None,
        modifiers: ModifiersState::empty(),
        theme,
        theme_mtime,
        a11y_sink,
        a11y_snapshot: Arc::new(std::sync::Mutex::new(a11y::A11ySnapshot::default())),
        frame_index: 0,
        started: Instant::now(),
        next_frame_at: Instant::now(),
        frame_period: DEFAULT_FRAME_PERIOD,
        pending_relayout: false,
        frame_stats: FrameStats::new(),
    };
    event_loop.run_app(&mut shell)?;
    Ok(())
}

/// Frame-tick fallback for `Sub::Frames` (§12.2) when the monitor's real
/// refresh rate isn't known yet (before the first `resumed`). Once a
/// window exists, `Shell::refresh_frame_period` replaces this with the
/// actual refresh period so ticks stop beating against vsync
/// (PLAN-DASHBOARD-PERF.md Stage 1).
const DEFAULT_FRAME_PERIOD: Duration = Duration::from_millis(16);

struct Gfx {
    window: Arc<Window>,
    // Kept alive for the surface; softbuffer ties surface validity to it.
    _context: softbuffer::Context<Arc<Window>>,
    surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
    /// §17: the AccessKit adapter exporting the semantics tree to AT.
    a11y: accesskit_winit::Adapter,
}

/// What a pointer drag is attached to (M5 Split divider / Slider track).
#[derive(Clone)]
enum DragTarget {
    Split(String),
    Slider(String),
}

/// §19: Libra's closed `Cursor` set → winit's icon vocabulary. The
/// translation lives in the shell so `libra-components` can derive the
/// affordance (`LaidTree::cursor_at`) without knowing about winit — the
/// same split the rest of the presentation layer keeps.
fn cursor_icon(cursor: Cursor) -> CursorIcon {
    match cursor {
        Cursor::Default => CursorIcon::Default,
        Cursor::Pointer => CursorIcon::Pointer,
        Cursor::Text => CursorIcon::Text,
        Cursor::ColResize => CursorIcon::ColResize,
        Cursor::NotAllowed => CursorIcon::NotAllowed,
    }
}

struct Shell<A: App> {
    opts: Options,
    app: A,
    ast: Node,
    /// §7.2 composed components discovered alongside the root view file.
    registry: libra_markup::ComponentRegistry,
    effects: Effects<A::Msg>,
    renderer: Renderer,
    laid: Option<LaidTree>,
    /// Dev diagnostic drawn over the last good frame (§18.2).
    overlay: Option<String>,
    /// Last cursor position, physical pixels.
    cursor: (f64, f64),
    /// §19: the pointer shape currently set on the window. Cached so a
    /// pointer sweep only calls into the platform when the affordance
    /// under it actually changes.
    cursor_icon: Cursor,
    /// PLAN-SCROLL: scroll positions by `<Scroll>` test_id, driven by the
    /// wheel. Applied to the laid tree in `relayout` (like the harness's
    /// `scroll()`); empty = every viewport at the top.
    scroll_state: libra_components::ScrollState,
    /// §19.2: the app's validated chord → action table (built once at
    /// startup from `A::shortcuts()`).
    shortcut_table: Vec<(libra_core::Chord, libra_core::Shortcut)>,
    /// Pointer drag target (M5 Split divider / Slider track), keyed by
    /// the control's test_id while the left button is down.
    drag: Option<DragTarget>,
    /// M7 `<ContextMenu>` anchor — the last right-click point (logical
    /// px), a deterministic layout input like scroll offsets.
    menu_anchor: (f32, f32),
    gfx: Option<Gfx>,
    watch_mtime: Option<std::time::SystemTime>,
    next_watch_check: Instant,
    /// Rehydration (§18.2): snapshots the model on exit.
    save_hook: Option<SaveHook<A>>,
    /// Damage tracking (§9.3): previous frame's cmds + pixels.
    frame_cache: Option<libra_raster::FrameCache>,
    /// Sub::Frames (§12.2): diffed from subscriptions after each relayout.
    frames_active: bool,
    /// §12.5: `onTick` handler names from every mounted `<Frames>` in
    /// the current laid tree, refreshed by `relayout`. Unioned with
    /// `App::subscriptions()`'s `Sub::Frames` — either form (or both)
    /// keeps the display loop running.
    markup_frame_handlers: Vec<String>,
    /// §12.5: `(every_ms, onTick)` for every mounted `<Interval>` in
    /// the current laid tree, refreshed by `relayout`. Diffed against
    /// `running_intervals` in `sync_intervals` — mount starts a timer
    /// thread, unmount cancels it.
    markup_intervals: Vec<(u64, String)>,
    /// Timer threads backing `markup_intervals`, keyed by identity
    /// `(every_ms, handler)` — mirrors `libra-effects`' one-shot Timer
    /// dedicated-thread pattern, just recurring. The `AtomicBool` is
    /// each thread's cancel flag.
    running_intervals: Vec<(u64, String, Arc<AtomicBool>)>,
    /// §12.5: `(id, onEvent)` for every mounted `<Stream>` in the
    /// current laid tree — grammar + mount + query only in v1 (no
    /// event delivery yet, PLAN-SOURCES), collected so `has_source`/a
    /// future differ can see them.
    markup_streams: Vec<(String, String)>,
    /// §10.1 focus: focused Input id + editing state + onChange name.
    focus: Option<(String, EditState, String)>,
    modifiers: ModifiersState,
    /// §11: current theme; Effect::SetTheme swaps it live.
    theme: Theme,
    theme_mtime: Option<std::time::SystemTime>,
    /// §17: msgs from screen-reader actions enter here (same causal
    /// path as effect results).
    a11y_sink: libra_effects::Sink<A::Msg>,
    a11y_snapshot: Arc<std::sync::Mutex<a11y::A11ySnapshot>>,
    frame_index: u64,
    started: Instant,
    /// Absolute schedule for `<Frames>` ticks (Stage 1): advance by
    /// `frame_period`, resync only if more than a period behind.
    next_frame_at: Instant,
    /// Tick cadence for `Sub::Frames` (§12.2): the window's actual
    /// monitor refresh period once known, `DEFAULT_FRAME_PERIOD` until
    /// then. Recomputed on `resumed` and on `Resized`/`ScaleFactorChanged`
    /// (the window may have moved to a different-refresh-rate monitor).
    frame_period: Duration,
    /// Resize coalescing: defer relayout to the next `RedrawRequested`
    /// so a drag that delivers many `Resized` events per frame only
    /// rebuilds view/layout once.
    pending_relayout: bool,
    frame_stats: FrameStats,
}

impl<A: App + 'static> Shell<A>
where
    A::Msg: Send + 'static,
{
    fn scale(&self) -> f32 {
        self.gfx.as_ref().map_or(1.0, |g| g.window.scale_factor() as f32)
    }

    /// Rebuild view → layout at the window's current logical size.
    /// Interp errors keep the last good tree and set the overlay.
    fn relayout(&mut self) {
        let Some(gfx) = &self.gfx else { return };
        let size = gfx.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        let logical: LogicalSize<f32> = size.to_logical(gfx.window.scale_factor());
        let field = |p: &str| self.app.field(p);
        let has_action = |m: &str| A::Msg::action_by_name(m).is_some();
        let has_str_action = |m: &str| A::Msg::action_with_str(m, String::new()).is_some();
        let collection_rows = |p: &str| self.app.collection_rows(p);
        let has_frame_info_action =
            |m: &str| A::Msg::action_with_frame_info(m, FrameInfo::at(0.0)).is_some();
        let has_instant_action =
            |m: &str| A::Msg::action_with_instant(m, LxInstant::at(0.0)).is_some();
        let scope = Scope {
            field: &field,
            has_action: &has_action,
            has_str_action: &has_str_action,
            collection_rows: &collection_rows,
            has_frame_info_action: &has_frame_info_action,
            has_instant_action: &has_instant_action,
            binding_ctx: "State",
        };
        match libra_interp::resolve(&self.ast, &scope, &self.registry) {
            Ok(view) => match libra_components::layout(&view, (logical.width, logical.height), &self.renderer, &self.theme) {
                Ok(laid) => {
                    // §12.5: collect every mounted source element's
                    // declaration so sync_subscriptions/tick_frame/
                    // sync_intervals can union/diff them against
                    // App::subscriptions() and the previously-running
                    // set — the markup-declared and Rust-declared forms
                    // are equally valid.
                    self.markup_frame_handlers = laid
                        .items
                        .iter()
                        .filter_map(|i| match &i.kind {
                            ItemKind::Source(SourceDecl::Frames { on_tick }) => Some(on_tick.clone()),
                            _ => None,
                        })
                        .collect();
                    self.markup_intervals = laid
                        .items
                        .iter()
                        .filter_map(|i| match &i.kind {
                            ItemKind::Source(SourceDecl::Interval { every_ms, on_tick }) => {
                                Some((*every_ms, on_tick.clone()))
                            }
                            _ => None,
                        })
                        .collect();
                    self.markup_streams = laid
                        .items
                        .iter()
                        .filter_map(|i| match &i.kind {
                            ItemKind::Source(SourceDecl::Stream { id, on_event }) => {
                                Some((id.clone(), on_event.clone()))
                            }
                            _ => None,
                        })
                        .collect();
                    // PLAN-SCROLL: offset scrolled viewports by the wheel-driven
                    // positions before storing (rects are natural until now).
                    let mut laid = laid;
                    laid.apply_scroll(&self.scroll_state);
                    // M7: float mounted context menus to the last
                    // right-click point.
                    laid.apply_menu_anchor(self.menu_anchor);
                    self.laid = Some(laid);
                    self.overlay = None;
                    self.push_a11y();
                    self.sync_intervals();
                    // After markup sources are known — never before.
                    // (A prior sync on empty handlers left frames idle forever.)
                    self.sync_subscriptions();
                    // §19: the tree moved under a possibly-still pointer —
                    // a dispatch that opens a dialog, deletes the hovered
                    // row, or disables a control changes the affordance
                    // without any pointer motion to notice it.
                    self.sync_cursor();
                }
                Err(e) => self.overlay = Some(e.to_string()),
            },
            Err(e) => self.overlay = Some(e.to_string()),
        }
    }

    /// §17: refresh the semantic snapshot and, if AT is listening,
    /// push a TreeUpdate.
    fn push_a11y(&mut self) {
        let Some(laid) = &self.laid else { return };
        let scale = self.scale() as f64;
        let focused = self.focus.as_ref().map(|(id, _, _)| id.as_str());
        let snap = a11y::snapshot(laid, scale, focused);
        // Stage 4: build the update from the local borrow first, then
        // move `snap` into the mutex last — one snapshot, not two.
        if let Some(gfx) = &mut self.gfx {
            gfx.a11y.update_if_active(|| a11y::tree_update(&snap));
        }
        *self.a11y_snapshot.lock().expect("a11y snapshot") = snap;
    }

    fn dispatch(&mut self, msg: A::Msg) {
        let effects = self.app.update(msg);
        // SetTheme is presentation state (§11): the shell applies it;
        // ClipboardSet is a UI-thread OS call (arboard) acked inline;
        // everything else goes to the effects runtime.
        let mut rest = Vec::with_capacity(effects.len());
        let mut acks: Vec<A::Msg> = Vec::new();
        for effect in effects {
            if let Effect::SetTheme { name } = &effect {
                match Theme::by_name(name) {
                    Some(t) => self.theme = t,
                    None => eprintln!("[libra] unknown theme '{name}' (built in: libra-dark, libra-light)"),
                }
                continue;
            }
            if let Effect::ClipboardSet { text, on_done } = &effect {
                let result = match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(text.clone())) {
                    Ok(()) => libra_core::ClipboardResult::Ok,
                    Err(e) => libra_core::ClipboardResult::Err(e.to_string()),
                };
                if let Some(ctor) = on_done {
                    acks.push(ctor(result));
                }
                continue;
            }
            rest.push(effect);
        }
        for ack in acks {
            // Re-enter: the ack is an ordinary action (one level deep —
            // clipboard acks don't themselves set the clipboard).
            self.dispatch(ack);
        }
        self.effects.execute_all(rest);
        self.relayout(); // collects mounted sources, then sync_subscriptions
        if let Some(gfx) = &self.gfx {
            gfx.window.request_redraw();
        }
    }

    /// Diff frame subscriptions after relayout (§12.2 / §12.5). Wants the
    /// display loop if any of: `App::subscriptions()`'s `Sub::Frames`, a
    /// mounted `<Frames>`, **or a live data-plane source** — a streaming ring
    /// repaints the view on its own (§14.2), so a `<Chart>` bound to it
    /// animates with no author-wired `<Frames>` pump. Pause (`IngestClose`)
    /// drops the live-ingest term and the loop idles. Activating arms an
    /// immediate first tick; the `request_redraw` chain keeps it alive.
    fn sync_subscriptions(&mut self) {
        let want = self.app.subscriptions().iter().any(|s| matches!(s, Sub::Frames { .. }))
            || !self.markup_frame_handlers.is_empty()
            || self.effects.store().has_live_ingest();
        if want && !self.frames_active {
            self.next_frame_at = Instant::now();
        }
        self.frames_active = want;
    }

    /// §12.5: diff `markup_intervals` against the running set, starting
    /// a dedicated timer thread per newly-mounted identity
    /// `(every_ms, on_tick)` and cancelling threads whose identity is no
    /// longer mounted — "mount = subscribe, unmount = cancel," the same
    /// contract `sync_subscriptions`/`tick_frame` give `<Frames>`, just
    /// per-cadence instead of per-vsync. Mirrors `libra-effects`' Timer
    /// dedicated-thread pattern: a sleeping timer must not occupy a
    /// shared pool worker.
    fn sync_intervals(&mut self) {
        // Dedup: a repeated <Interval> with the same identity still
        // runs just once (same treatment tick_frame gives duplicate
        // <Frames> handlers).
        let mut wanted: Vec<(u64, String)> = Vec::new();
        for id in &self.markup_intervals {
            if !wanted.contains(id) {
                wanted.push(id.clone());
            }
        }

        self.running_intervals.retain(|(every_ms, name, cancel)| {
            let keep = wanted.iter().any(|(ms, n)| ms == every_ms && n == name);
            if !keep {
                cancel.store(true, Ordering::Relaxed);
            }
            keep
        });

        for (every_ms, name) in wanted {
            if self.running_intervals.iter().any(|(ms, n, _)| *ms == every_ms && *n == name) {
                continue;
            }
            let cancel = Arc::new(AtomicBool::new(false));
            let cancel_thread = Arc::clone(&cancel);
            let sink = Arc::clone(&self.a11y_sink);
            let started = self.started;
            let handler = name.clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_millis(every_ms.max(1)));
                if cancel_thread.load(Ordering::Relaxed) {
                    break;
                }
                let t_ms = started.elapsed().as_secs_f64() * 1000.0;
                match A::Msg::action_with_instant(&handler, LxInstant::at(t_ms)) {
                    Some(msg) => sink(msg),
                    None => {
                        eprintln!(
                            "[libra] <Interval onTick={{{handler}}}> resolved to no action — reflection out of sync"
                        );
                        break;
                    }
                }
            });
            self.running_intervals.push((every_ms, name, cancel));
        }
    }

    /// Stage 1: match tick cadence to the window's actual monitor so
    /// display-loop ticks land in step with the compositor instead of
    /// beating against it. Falls back to `DEFAULT_FRAME_PERIOD` when
    /// the platform can't report a rate (`current_monitor`/
    /// `refresh_rate_millihertz` both return `Option`).
    fn refresh_frame_period(&mut self) {
        let mhz = self.gfx.as_ref().and_then(|g| g.window.current_monitor()).and_then(|m| m.refresh_rate_millihertz());
        self.frame_period = match mhz {
            Some(mhz) if mhz > 0 => Duration::from_secs_f64(1000.0 / mhz as f64),
            _ => DEFAULT_FRAME_PERIOD,
        };
    }

    fn tick_frame(&mut self) {
        let info = FrameInfo {
            t_ms: self.started.elapsed().as_secs_f64() * 1000.0,
            index: self.frame_index,
        };
        self.frame_index += 1;

        let ctor = self.app.subscriptions().into_iter().find_map(|s| match s {
            Sub::Frames { on_frame } => Some(on_frame),
        });
        if let Some(on_frame) = ctor {
            self.dispatch(on_frame(info));
        }

        // §12.5: every distinct markup-declared handler also gets this
        // tick, resolved by name via ReflectAction — the same causal
        // channel as every other action (replay/time-travel see it).
        // Cloned up front: `dispatch` re-runs `relayout`, which
        // rewrites `markup_frame_handlers` — iterating the live field
        // while mutating it through `dispatch` would be a borrow
        // conflict (and semantically wrong: this tick should see the
        // set as it was when the tick started, not as it churns).
        let mut handled: Vec<String> = Vec::new();
        for name in self.markup_frame_handlers.clone() {
            if handled.contains(&name) {
                continue;
            }
            handled.push(name.clone());
            match A::Msg::action_with_frame_info(&name, info) {
                Some(msg) => self.dispatch(msg),
                None => eprintln!(
                    "[libra] <Frames onTick={{{name}}}> resolved to no action — reflection out of sync"
                ),
            }
        }
    }

    /// PLAN-SCROLL: route a wheel event to the innermost `<Scroll>` under the
    /// cursor and re-lay-out with the updated position. Scroll is a view input
    /// (like focus/hover) — no action, no entry in the log.
    fn wheel(&mut self, delta: winit::event::MouseScrollDelta) {
        use winit::event::MouseScrollDelta;
        let scale = self.scale();
        // Wheel-down (content moves up, revealing lower items) → offset grows.
        let dy = match delta {
            MouseScrollDelta::LineDelta(_, y) => -y * 48.0, // ~2 rows per notch
            MouseScrollDelta::PixelDelta(p) => -(p.y as f32) / scale,
        };
        if dy == 0.0 {
            return;
        }
        let (lx, ly) = (self.cursor.0 as f32 / scale, self.cursor.1 as f32 / scale);
        let target = self.laid.as_ref().and_then(|laid| {
            laid.items
                .iter()
                .rev()
                .find(|i| matches!(i.kind, ItemKind::Scroll) && i.rect.contains(lx, ly))
                .and_then(|i| i.semantics.test_id.clone())
        });
        let Some(id) = target else { return };
        let pos = self.scroll_state.entry(id).or_insert(0.0);
        *pos = (*pos + dy).max(0.0);
        self.relayout();
        self.request_redraw();
    }

    fn click(&mut self) {
        let scale = self.scale();
        let (lx, ly) = (self.cursor.0 as f32 / scale, self.cursor.1 as f32 / scale);
        let Some(laid) = &self.laid else { return };
        // M5: pressing a `<Split>` divider gutter starts a drag instead
        // of a click (the gutter is not a pressable item).
        let target = laid.items.iter().rev().find_map(|i| match &i.kind {
            ItemKind::SplitDivider { split } if i.rect.contains(lx, ly) => {
                Some(DragTarget::Split(split.clone()))
            }
            ItemKind::SliderCtl { .. }
                if i.rect.contains(lx, ly) && i.clip.map_or(true, |c| c.contains(lx, ly)) =>
            {
                i.semantics.test_id.clone().map(DragTarget::Slider)
            }
            _ => None,
        });
        if let Some(target) = target {
            self.drag = Some(target);
            // Click-to-set: a slider press applies immediately (a drag
            // with a single move).
            self.drag_move();
            return;
        }
        let Some(hit) = laid.hit(lx, ly) else {
            // Clicked empty space: blur (§10.1).
            if self.focus.take().is_some() {
                self.request_redraw();
            }
            return;
        };
        let name = hit.on_press.clone().expect("hit item has a handler");
        if hit.semantics.role == Role::TextInput {
            let id = hit.semantics.test_id.clone().expect("Input has a testId (load-enforced)");
            self.focus = Some((id, EditState::new(hit.semantics.text.clone()), name));
            self.request_redraw();
            return;
        }
        self.focus = None;
        // Item-scoped handler (`onPress={toggle(item)}`, §6.2): dispatch the
        // String-payload action with the resolved loop-binding value.
        let msg = match hit.on_press_arg.clone() {
            Some(arg) => A::Msg::action_with_str(&name, arg).unwrap_or_else(|| {
                panic!("handler '{name}' resolved to no String-payload Action — reflection out of sync")
            }),
            None => A::Msg::action_by_name(&name).unwrap_or_else(|| {
                panic!("handler '{name}' resolved to no Action — reflection out of sync")
            }),
        };
        self.dispatch(msg);
    }

    fn request_redraw(&self) {
        if let Some(gfx) = &self.gfx {
            gfx.window.request_redraw();
        }
    }

    /// §19: point the OS pointer at the affordance under it. Called after
    /// anything that can change either term — the pointer moved, or the
    /// tree under a still pointer was rebuilt.
    ///
    /// A live drag pins its shape: the pointer routinely leaves the divider
    /// gutter mid-resize, and a cursor that flickers back to an arrow there
    /// reads as a dropped grab. Purely presentational — the OS draws the
    /// pointer, so nothing here touches the display list (§21).
    fn sync_cursor(&mut self) {
        let want = match &self.drag {
            Some(DragTarget::Split(_)) => Cursor::ColResize,
            Some(DragTarget::Slider(_)) => Cursor::Pointer,
            None => {
                let scale = self.scale();
                let (lx, ly) = (self.cursor.0 as f32 / scale, self.cursor.1 as f32 / scale);
                self.laid.as_ref().map_or(Cursor::Default, |laid| laid.cursor_at(lx, ly))
            }
        };
        if want == self.cursor_icon {
            return;
        }
        self.cursor_icon = want;
        if let Some(gfx) = &self.gfx {
            gfx.window.set_cursor(cursor_icon(want));
        }
    }

    /// M7: right-click — find the topmost row with an `onMenu` handler
    /// under the pointer, record the anchor, and dispatch it (the app
    /// mounts its `<ContextMenu>` via `<if>`; relayout floats it here).
    fn right_click(&mut self) {
        let scale = self.scale();
        let (lx, ly) = (self.cursor.0 as f32 / scale, self.cursor.1 as f32 / scale);
        let Some(laid) = &self.laid else { return };
        let Some((name, arg)) = laid.items.iter().rev().find_map(|i| match &i.kind {
            ItemKind::ListItem { on_menu: Some((name, arg)), .. }
                if i.rect.contains(lx, ly)
                    && i.clip.map_or(true, |c| c.contains(lx, ly)) =>
            {
                Some((name.clone(), arg.clone()))
            }
            _ => None,
        }) else {
            return;
        };
        self.menu_anchor = (lx, ly);
        let msg = match arg {
            Some(a) => A::Msg::action_with_str(&name, a),
            None => A::Msg::action_by_name(&name),
        }
        .unwrap_or_else(|| {
            panic!("onMenu '{name}' resolved to no Action — reflection out of sync")
        });
        self.dispatch(msg);
    }

    /// Active pointer drag (M5 Split / Slider): re-derive the fraction
    /// from the pointer within the control's rect and dispatch its
    /// String-payload handler (`{:.4}` — identical formatting to the
    /// harness's `resize_split`/`set_slider`, so both paths agree).
    fn drag_move(&mut self) {
        let Some(target) = self.drag.clone() else { return };
        let scale = self.scale();
        let lx = self.cursor.0 as f32 / scale;
        let Some(laid) = &self.laid else { return };
        let found = match &target {
            DragTarget::Split(id) => laid.items.iter().find_map(|i| match &i.kind {
                ItemKind::Split { on_resize }
                    if i.semantics.test_id.as_deref() == Some(id.as_str()) =>
                {
                    Some((i.rect, on_resize.clone(), 0.05f32, 0.95f32))
                }
                _ => None,
            }),
            DragTarget::Slider(id) => laid.items.iter().find_map(|i| match &i.kind {
                ItemKind::SliderCtl { on_change, .. }
                    if i.semantics.test_id.as_deref() == Some(id.as_str()) =>
                {
                    Some((i.rect, on_change.clone(), 0.0f32, 1.0f32))
                }
                _ => None,
            }),
        };
        let Some((rect, handler, lo, hi)) = found else { return };
        if rect.w <= 0.0 {
            return;
        }
        let fraction = ((lx - rect.x) / rect.w).clamp(lo, hi);
        let msg = A::Msg::action_with_str(&handler, format!("{fraction:.4}"))
            .unwrap_or_else(|| {
                panic!("'{handler}' resolved to no String-payload Action — reflection out of sync")
            });
        self.dispatch(msg);
    }

    /// §19.2: map the key event onto the validated shortcut table.
    /// Precedence: a focused text control owns un-modified keys and the
    /// editing-reserved `cmd` chords — only unclaimed `cmd` chords fall
    /// through to the table while typing; unfocused, the whole table is
    /// live. Returns true if a shortcut dispatched.
    fn try_shortcut(&mut self, event: &KeyEvent) -> bool {
        use libra_core::ChordKey;
        if self.shortcut_table.is_empty() {
            return false;
        }
        let mods = self.modifiers;
        let key = match &event.logical_key {
            Key::Named(NamedKey::Escape) => ChordKey::Escape,
            Key::Named(NamedKey::Backspace) => ChordKey::Backspace,
            Key::Named(NamedKey::Delete) => ChordKey::Delete,
            Key::Named(NamedKey::Enter) => ChordKey::Enter,
            Key::Named(NamedKey::Space) => ChordKey::Space,
            Key::Named(NamedKey::Tab) => ChordKey::Tab,
            Key::Named(NamedKey::ArrowUp) => ChordKey::Up,
            Key::Named(NamedKey::ArrowDown) => ChordKey::Down,
            Key::Named(NamedKey::ArrowLeft) => ChordKey::Left,
            Key::Named(NamedKey::ArrowRight) => ChordKey::Right,
            Key::Character(c) => {
                let mut chars = c.as_str().chars();
                match (chars.next(), chars.next()) {
                    // Shift is carried by the modifier flag; the char
                    // itself normalizes to lowercase for matching.
                    (Some(ch), None) => match ch.to_lowercase().next() {
                        Some(lc) => ChordKey::Char(lc),
                        None => return false,
                    },
                    _ => return false,
                }
            }
            _ => return false,
        };
        let chord = libra_core::Chord {
            // v1: the platform-primary modifier — Super or Ctrl both
            // read as `cmd`, matching the §10.1 editing keymap.
            cmd: mods.super_key() || mods.control_key(),
            ctrl: false,
            alt: mods.alt_key(),
            shift: mods.shift_key(),
            key,
        };
        if self.focus.is_some() && !shortcut_fires_while_focused(&chord) {
            return false;
        }
        let Some((_, s)) = self.shortcut_table.iter().find(|(c, _)| *c == chord) else {
            return false;
        };
        let msg = match &s.arg {
            Some(arg) => A::Msg::action_with_str(&s.action, arg.clone()),
            None => A::Msg::action_by_name(&s.action),
        }
        .expect("shortcut table validated at startup");
        self.dispatch(msg);
        self.request_redraw();
        true
    }

    /// §10.1 keyboard model: platform-conventional bindings mapped to
    /// editing-core ops. Text-changing ops dispatch the Input's
    /// onChange msg; navigation just repaints the caret.
    fn key(&mut self, event: &KeyEvent) {
        if event.state != ElementState::Pressed {
            return;
        }
        // §19.2: shortcuts get first look; the precedence check inside
        // guarantees they never shadow the editing keymap.
        if self.try_shortcut(event) {
            return;
        }
        if self.focus.is_none() {
            return;
        }
        let mods = self.modifiers;
        let shift = mods.shift_key();
        let cmd = mods.super_key() || mods.control_key();
        let word = mods.alt_key();
        let op = match &event.logical_key {
            Key::Named(NamedKey::Backspace) => Some(EditOp::Backspace),
            Key::Named(NamedKey::Delete) => Some(EditOp::Delete),
            Key::Named(NamedKey::ArrowLeft) if cmd => Some(EditOp::Home(shift)),
            Key::Named(NamedKey::ArrowRight) if cmd => Some(EditOp::End(shift)),
            Key::Named(NamedKey::ArrowLeft) if word => Some(EditOp::WordLeft(shift)),
            Key::Named(NamedKey::ArrowRight) if word => Some(EditOp::WordRight(shift)),
            Key::Named(NamedKey::ArrowLeft) => Some(EditOp::Left(shift)),
            Key::Named(NamedKey::ArrowRight) => Some(EditOp::Right(shift)),
            Key::Named(NamedKey::Home) => Some(EditOp::Home(shift)),
            Key::Named(NamedKey::End) => Some(EditOp::End(shift)),
            Key::Named(NamedKey::Space) => Some(EditOp::Insert(' ')),
            // Multi-line ops apply only when the focused control is a
            // Textarea (§10.1 stage 1): Enter inserts a newline, and
            // Up/Down navigate lines. In an Input, Enter is inert and
            // Up/Down fall through as no-ops.
            Key::Named(NamedKey::Enter) if self.focused_is_textarea() => {
                self.apply_edit(EditOp::Insert('\n'));
                return;
            }
            Key::Named(NamedKey::ArrowUp) if self.focused_is_textarea() => Some(EditOp::Up(shift)),
            Key::Named(NamedKey::ArrowDown) if self.focused_is_textarea() => {
                Some(EditOp::Down(shift))
            }
            Key::Named(NamedKey::Escape) => {
                self.focus = None;
                self.request_redraw();
                return;
            }
            Key::Character(c) if cmd && c.as_str() == "a" => Some(EditOp::SelectAll),
            // Clipboard (§10.1, arboard): the editing core stays pure —
            // the shell moves text between OS clipboard and EditOps.
            Key::Character(c) if cmd && c.as_str() == "c" => {
                if let Some((_, edit, _)) = &self.focus {
                    let sel = edit.selected_text().to_string();
                    if !sel.is_empty() {
                        if let Ok(mut cb) = arboard::Clipboard::new() {
                            let _ = cb.set_text(sel);
                        }
                    }
                }
                return;
            }
            Key::Character(c) if cmd && c.as_str() == "x" => {
                if let Some((_, edit, _)) = &self.focus {
                    let sel = edit.selected_text().to_string();
                    if !sel.is_empty() {
                        if let Ok(mut cb) = arboard::Clipboard::new() {
                            let _ = cb.set_text(sel);
                        }
                        self.apply_edit(EditOp::Backspace); // deletes the selection
                    }
                }
                return;
            }
            Key::Character(c) if cmd && c.as_str() == "v" => {
                if let Ok(mut cb) = arboard::Clipboard::new() {
                    if let Ok(text) = cb.get_text() {
                        // Single-line fields flatten newlines to spaces;
                        // a Textarea keeps them (normalized to '\n').
                        let text = if self.focused_is_textarea() {
                            text.replace("\r\n", "\n").replace('\r', "\n")
                        } else {
                            text.replace(['\r', '\n'], " ")
                        };
                        self.apply_edit(EditOp::InsertStr(text));
                    }
                }
                return;
            }
            Key::Character(c) if cmd && c.as_str() == "z" && shift => Some(EditOp::Redo),
            Key::Character(c) if cmd && c.as_str() == "z" => Some(EditOp::Undo),
            Key::Character(c) if !cmd => {
                for ch in c.as_str().chars() {
                    self.apply_edit(EditOp::Insert(ch));
                }
                return;
            }
            _ => None,
        };
        if let Some(op) = op {
            self.apply_edit(op);
        }
    }

    fn apply_edit(&mut self, op: EditOp) {
        let Some((id, mut edit, on_change)) = self.focus.take() else { return };
        let before = edit.text().to_string();
        edit.apply(op);
        let changed = edit.text() != before;
        let new_text = edit.text().to_string();
        self.focus = Some((id, edit, on_change.clone()));
        if changed {
            let msg = A::Msg::action_with_str(&on_change, new_text)
                .unwrap_or_else(|| panic!("'{on_change}' is not a String-payload Action variant"));
            self.dispatch(msg); // relayout picks up state's new value
        }
        self.request_redraw();
    }

    fn focus_paint(&self) -> Option<InputFocus> {
        self.focus.as_ref().map(|(id, e, _)| InputFocus {
            test_id: id.clone(),
            caret: e.caret(),
            selection: e.selection(),
        })
    }

    /// Is the focused control a `Textarea`? Decides the multi-line key
    /// bindings (Enter/Up/Down) and paste newline handling (§10.1).
    fn focused_is_textarea(&self) -> bool {
        let Some((id, ..)) = &self.focus else { return false };
        let Some(laid) = &self.laid else { return false };
        laid.items.iter().any(|i| {
            i.semantics.test_id.as_deref() == Some(id.as_str())
                && matches!(i.kind, libra_components::ItemKind::Textarea { .. })
        })
    }

    /// Dev hot reload: swap the AST if the watched file changed; on a
    /// parse error keep the last good view and overlay the diagnostic.
    fn check_watch(&mut self) {
        let Some(path) = self.opts.watch.clone() else { return };
        let Some(mtime) = std::fs::metadata(&path).ok().and_then(|m| m.modified().ok()) else {
            return;
        };
        if Some(mtime) == self.watch_mtime {
            return;
        }
        self.watch_mtime = Some(mtime);
        let source = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                self.overlay = Some(format!("{}: {e}", path.display()));
                return;
            }
        };
        match libra_markup::parse(&source) {
            Ok(ast) => {
                self.ast = ast;
                if let Some(dir) = path.parent() {
                    self.registry =
                        libra_markup::ComponentRegistry::load_from_dir(dir).unwrap_or_default();
                }
                self.relayout(); // model state untouched — that's the point
                eprintln!("[libra] hot-reloaded {}", path.display());
            }
            Err(e) => self.overlay = Some(format!("{}: {e}", path.display())),
        }
        if let Some(gfx) = &self.gfx {
            gfx.window.request_redraw();
        }
    }

    /// §11: theme tables hot-reload like views — edit, save, restyle.
    fn check_theme_file(&mut self) {
        let Some(path) = self.opts.theme_file.clone() else { return };
        let Some(mtime) = std::fs::metadata(&path).ok().and_then(|m| m.modified().ok()) else {
            return;
        };
        if Some(mtime) == self.theme_mtime {
            return;
        }
        self.theme_mtime = Some(mtime);
        match std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|s| Theme::from_toml_str(&s))
        {
            Ok(t) => {
                eprintln!("[libra] theme '{}' hot-reloaded", t.name);
                self.theme = t;
                self.overlay = None;
                self.relayout();
            }
            Err(e) => self.overlay = Some(format!("{}: {e}", path.display())),
        }
        self.request_redraw();
    }

    fn present(&mut self) {
        let t0 = Instant::now();
        let focus_paint = self.focus_paint();
        // Hover as an explicit paint input (ListItem washes): the
        // shell's live cursor in logical px. Presentation-side only —
        // the canonical harness frame defaults to no hover.
        let hover = {
            let s = self.scale();
            Some((self.cursor.0 as f32 / s, self.cursor.1 as f32 / s))
        };
        let theme = &self.theme;
        let Some(laid) = &self.laid else { return };
        let Some(gfx) = &mut self.gfx else { return };
        let size = gfx.window.inner_size();
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return;
        };
        let scale = gfx.window.scale_factor() as f32;

        // Error boundary at the paint boundary (Appendix C): a panic in
        // paint or raster becomes an overlay frame, not a dead window.
        // Extends to Rust components/Canvas when those land (Phase 3).
        // Rendering is damage-tracked (§9.3): only changed regions
        // re-rasterize frame to frame.
        let logical_w = laid.width;
        let renderer = &self.renderer;
        let frame_cache = &mut self.frame_cache;
        let store = self.effects.store();
        let overlay = self.overlay.clone();
        let painted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // Stage 6: charts render at the window's physical density
            // (Retina: 1 column per physical pixel) — presentation-only,
            // off the determinism contract (§21); the canonical density
            // stays 1.0 (`LaidTree::paint`, everywhere else).
            let mut cmds = laid.paint_full(renderer, store, focus_paint.as_ref(), hover, &theme, scale);
            if let Some(text) = &overlay {
                overlay_cmds(&mut cmds, text, logical_w, &theme);
            }
            scale_cmds_in_place(&mut cmds, scale);
            renderer.render_damage(frame_cache, cmds, size.width, size.height)
        }));
        let damage = match painted {
            Ok(damage) => damage,
            Err(payload) => {
                let text = format!("paint panicked: {}", panic_text(&payload));
                eprintln!("[libra] {text}");
                self.overlay = Some(text.clone());
                let mut safe = vec![DrawCmd::Clear(theme.bg)];
                overlay_cmds(&mut safe, &text, logical_w, &theme);
                scale_cmds_in_place(&mut safe, scale);
                let pixmap = self.renderer.render(&safe, size.width, size.height);
                self.frame_cache = None; // cache poisoned mid-paint: discard
                gfx.surface.resize(w, h).expect("surface resize");
                let mut buffer = gfx.surface.buffer_mut().expect("surface buffer");
                blit_rows(&mut buffer, &pixmap, 0, w.get() as usize, 0, h.get() as usize);
                gfx.window.pre_present_notify();
                buffer.present().expect("present");
                self.frame_stats.record(t0.elapsed());
                return;
            }
        };

        // Stage 2 (PLAN-DASHBOARD-PERF.md): convert and present only the
        // damaged region — `Damage::None` touches the surface at all,
        // `Partial` converts just its rect (falling back to a full
        // convert when the buffer's prior contents aren't last frame's,
        // per `Buffer::age`'s contract), `Full` behaves as before.
        if matches!(damage, Damage::None) {
            self.frame_stats.record(t0.elapsed());
            return;
        }
        let Some(cache) = self.frame_cache.as_ref() else { return };
        let pixmap = cache.pixmap();
        gfx.surface.resize(w, h).expect("surface resize");
        let mut buffer = gfx.surface.buffer_mut().expect("surface buffer");
        let (bw, bh) = (w.get(), h.get());

        if let Damage::Partial(r) = damage {
            if buffer.age() == 1 {
                let x0 = (r.x.round() as u32).min(bw);
                let y0 = (r.y.round() as u32).min(bh);
                let x1 = ((r.x + r.w).round() as u32).min(bw).max(x0);
                let y1 = ((r.y + r.h).round() as u32).min(bh).max(y0);
                blit_rows(&mut buffer, pixmap, x0 as usize, x1 as usize, y0 as usize, y1 as usize);
                gfx.window.pre_present_notify();
                if x1 > x0 && y1 > y0 {
                    let damage_rect = softbuffer::Rect {
                        x: x0,
                        y: y0,
                        width: NonZeroU32::new(x1 - x0).expect("nonzero damage width"),
                        height: NonZeroU32::new(y1 - y0).expect("nonzero damage height"),
                    };
                    buffer.present_with_damage(&[damage_rect]).expect("present");
                } else {
                    buffer.present().expect("present");
                }
                self.frame_stats.record(t0.elapsed());
                return;
            }
            // Buffer's prior contents aren't last frame's (age 0 or
            // stale double-buffered content): the partial convert below
            // would leave garbage outside the damaged rect, so fall
            // through to a full convert.
        }

        blit_rows(&mut buffer, pixmap, 0, bw as usize, 0, bh as usize);
        gfx.window.pre_present_notify();
        buffer.present().expect("present");
        self.frame_stats.record(t0.elapsed());
    }
}

impl<A: App + 'static> ApplicationHandler<A::Msg> for Shell<A>
where
    A::Msg: Send + 'static,
{
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return;
        }
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title(self.opts.title.clone())
                        // Created hidden: the AccessKit adapter must
                        // attach before first show.
                        .with_visible(false)
                        .with_inner_size(LogicalSize::new(
                            self.opts.logical_size.0 as f64,
                            self.opts.logical_size.1 as f64,
                        )),
                )
                .expect("create window"),
        );
        let context = softbuffer::Context::new(window.clone()).expect("softbuffer context");
        let surface =
            softbuffer::Surface::new(&context, window.clone()).expect("softbuffer surface");
        let a11y = accesskit_winit::Adapter::with_direct_handlers(
            event_loop,
            &window,
            Activation { snapshot: Arc::clone(&self.a11y_snapshot) },
            Actions::<A> {
                snapshot: Arc::clone(&self.a11y_snapshot),
                sink: Arc::clone(&self.a11y_sink),
            },
            Deactivation,
        );
        window.set_visible(true);
        window.request_redraw();
        self.gfx = Some(Gfx { window, _context: context, surface, a11y });
        self.refresh_frame_period();
        // relayout collects mounted `<Frames>` then sync_subscriptions
        // arms the display loop — do not sync on empty handlers first.
        self.relayout();
    }

    /// Effect results, a11y actions, and `<Interval>` ticks arrive here
    /// (via the EventLoopProxy) and dispatch on the main thread — same
    /// causal path as §4.2's loop. Frame ticks run from `about_to_wait`
    /// while the display loop is active.
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, msg: A::Msg) {
        self.dispatch(msg);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: WindowId,
        event: WindowEvent,
    ) {
        if let Some(gfx) = &mut self.gfx {
            let window = Arc::clone(&gfx.window);
            gfx.a11y.process_event(&window, &event);
        }
        match event {
            WindowEvent::CloseRequested => {
                if let Some(save) = &self.save_hook {
                    save(&self.app);
                }
                event_loop.exit();
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                // The window may have moved to a different-refresh-rate
                // monitor (Stage 1) — cadence is cheap to recompute.
                self.refresh_frame_period();
                self.pending_relayout = true;
                self.request_redraw();
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x, position.y);
                // M5: an active divider drag re-derives the fraction from
                // the pointer and dispatches onResize (the app echoes it
                // into the bound field — the controlled Split shape).
                if self.drag.is_some() {
                    self.drag_move();
                }
                // §19: the pointer shape follows what's under it.
                self.sync_cursor();
                // Hover is a paint input (ListItem washes); damage
                // tracking absorbs the repaint cost when nothing under
                // the pointer actually changes.
                self.request_redraw();
            }
            WindowEvent::ModifiersChanged(m) => self.modifiers = m.state(),
            WindowEvent::KeyboardInput { event, .. } => self.key(&event),
            WindowEvent::MouseWheel { delta, .. } => self.wheel(delta),
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => self.click(),
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                self.drag = None;
                // Releasing a drag un-pins the shape (§19).
                self.sync_cursor();
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => self.right_click(),
            WindowEvent::RedrawRequested => {
                if self.pending_relayout {
                    self.pending_relayout = false;
                    self.relayout();
                }
                self.present();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.opts.watch.is_some() || self.opts.theme_file.is_some() {
            if Instant::now() >= self.next_watch_check {
                self.check_watch();
                self.check_theme_file();
                self.next_watch_check = Instant::now() + WATCH_INTERVAL;
            }
        }

        // Display loop (§12.5): while `<Frames>` / `Sub::Frames` is
        // mounted, Poll the event loop and fire handlers on the refresh
        // schedule — the browser rAF equivalent. Developers only mount
        // `<Frames/>`; they never touch control flow. (WaitUntil alone
        // does not wake idle macOS AppKit; Poll does.)
        if self.frames_active {
            let now = Instant::now();
            if now >= self.next_frame_at {
                self.next_frame_at += self.frame_period;
                if self.next_frame_at < now {
                    self.next_frame_at = now + self.frame_period;
                }
                self.tick_frame();
                // Repaint every tick, not only as a side effect of a handler
                // dispatch: a display loop armed purely by a live data plane
                // (no `<Frames>` handler) still needs the frame presented so
                // the chart shows the newest ring samples (§14.2). Coalesced
                // with any dispatch-driven redraw by winit.
                self.request_redraw();
            }
            event_loop.set_control_flow(ControlFlow::Poll);
            return;
        }

        if self.opts.watch.is_some() || self.opts.theme_file.is_some() {
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_watch_check));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
}

/// Serves the current tree when AT first connects (§17).
struct Activation {
    snapshot: Arc<std::sync::Mutex<a11y::A11ySnapshot>>,
}

impl accesskit::ActivationHandler for Activation {
    fn request_initial_tree(&mut self) -> Option<accesskit::TreeUpdate> {
        Some(a11y::tree_update(&self.snapshot.lock().expect("a11y snapshot")))
    }
}

/// Routes screen-reader actions into the msg queue — an AT Click on a
/// Button dispatches exactly what a pointer click would.
struct Actions<A: App> {
    snapshot: Arc<std::sync::Mutex<a11y::A11ySnapshot>>,
    sink: libra_effects::Sink<A::Msg>,
}

impl<A: App + 'static> accesskit::ActionHandler for Actions<A>
where
    A::Msg: Send + 'static,
{
    fn do_action(&mut self, request: accesskit::ActionRequest) {
        if request.action != accesskit::Action::Click {
            return;
        }
        let snap = self.snapshot.lock().expect("a11y snapshot");
        if let Some(name) = a11y::action_for(&snap, request.target_node.0) {
            if let Some(msg) = A::Msg::action_by_name(name) {
                (self.sink)(msg);
            }
        }
    }
}

struct Deactivation;
impl accesskit::DeactivationHandler for Deactivation {
    fn deactivate_accessibility(&mut self) {}
}

/// Converts pixels within `[x0,x1) × [y0,y1)` from `src` (tiny-skia's
/// premultiplied RGBA, row-major, stride = `src.width()`) into `dst`
/// (softbuffer's 0RGB, same stride) — the one RGBA→XRGB conversion site
/// (PLAN-DASHBOARD-PERF.md Stage 2). Frame is opaque (`Clear` is always
/// first), so premultiplied == straight and alpha is safely dropped.
fn blit_rows(dst: &mut [u32], src: &Pixmap, x0: usize, x1: usize, y0: usize, y1: usize) {
    let stride = src.width() as usize;
    let pixels = src.pixels();
    for y in y0..y1 {
        let row = y * stride;
        for x in x0..x1 {
            let p = pixels[row + x];
            dst[row + x] = ((p.red() as u32) << 16) | ((p.green() as u32) << 8) | p.blue() as u32;
        }
    }
}

fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

/// The dev error overlay: a red-tinted bar at the top of the frame with
/// the diagnostic text, drawn over the last good view (§18.2).
fn overlay_cmds(cmds: &mut Vec<DrawCmd>, text: &str, width: f32, theme: &Theme) {
    const BAR_H: f32 = 28.0;
    cmds.push(DrawCmd::Rect {
        rect: Rect::new(0.0, 0.0, width, BAR_H),
        color: Color { r: 0x8B, g: 0x1A, b: 0x1A, a: 255 },
    });
    cmds.push(DrawCmd::Text {
        x: 8.0,
        y: 6.0,
        content: text.to_string(),
        px: theme.type_sm,
        style: FontStyle::Regular,
        color: Color::rgb(0xFF, 0xE6, 0xE6),
    });
}

/// Scale a logical-space DrawCmd stream to physical pixels in place
/// (Stage 4: the display list is scaled and fed to `render_damage`
/// exactly once per frame — no reason to allocate a second copy). Text
/// scales by px size (re-rasterized by fontdue at the target size), so
/// HiDPI output is crisp, not upsampled.
fn scale_cmds_in_place(cmds: &mut [DrawCmd], s: f32) {
    if s == 1.0 {
        return;
    }
    for cmd in cmds.iter_mut() {
        match cmd {
            DrawCmd::Clear(_) => {}
            DrawCmd::Rect { rect, .. } => *rect = scale_rect(rect, s),
            DrawCmd::RRect { rect, radius, .. } => {
                *rect = scale_rect(rect, s);
                *radius *= s;
            }
            DrawCmd::Text { x, y, px, .. } => {
                *x *= s;
                *y *= s;
                *px *= s;
            }
            DrawCmd::Polyline { points, width, .. } => {
                for (x, y) in points.iter_mut() {
                    *x *= s;
                    *y *= s;
                }
                *width *= s;
            }
            DrawCmd::Strips { x, col_w, strips, .. } => {
                *x *= s;
                *col_w *= s;
                for o in strips.iter_mut() {
                    if let Some((y0, y1)) = o {
                        *y0 *= s;
                        *y1 *= s;
                    }
                }
            }
            // Scale the clip rect with everything else so HiDPI clips align
            // with the scaled geometry (PLAN-SCROLL).
            DrawCmd::PushClip { rect } => *rect = scale_rect(rect, s),
            DrawCmd::PopClip => {}
        }
    }
}

fn scale_rect(r: &Rect, s: f32) -> Rect {
    Rect::new(r.x * s, r.y * s, r.w * s, r.h * s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_by_one_is_identity() {
        let theme = Theme::dark();
        let cmds = vec![
            DrawCmd::Clear(theme.bg),
            DrawCmd::RRect { rect: Rect::new(1.0, 2.0, 3.0, 4.0), radius: 6.0, color: theme.accent },
        ];
        let mut scaled = cmds.clone();
        scale_cmds_in_place(&mut scaled, 1.0);
        assert_eq!(scaled, cmds);
    }

    #[test]
    fn scale_multiplies_geometry_and_px_but_not_color() {
        let mut cmds = vec![DrawCmd::Text {
            x: 10.0,
            y: 20.0,
            content: "x".into(),
            px: 14.0,
            style: FontStyle::Regular,
            color: Theme::dark().text,
        }];
        scale_cmds_in_place(&mut cmds, 2.0);
        match &cmds[0] {
            DrawCmd::Text { x, y, px, color, .. } => {
                assert_eq!((*x, *y, *px), (20.0, 40.0, 28.0));
                assert_eq!(*color, Theme::dark().text);
            }
            other => panic!("unexpected cmd: {other:?}"),
        }
    }

    #[test]
    fn overlay_draws_bar_then_text_over_the_frame() {
        let mut cmds = vec![DrawCmd::Clear(Theme::dark().bg)];
        overlay_cmds(&mut cmds, "views/x.lx: 3:7: no field 'cuont'", 400.0, &Theme::dark());
        assert_eq!(cmds.len(), 3);
        assert!(matches!(&cmds[1], DrawCmd::Rect { rect, .. } if rect.w == 400.0));
        assert!(matches!(&cmds[2], DrawCmd::Text { content, .. } if content.contains("cuont")));
    }
}
