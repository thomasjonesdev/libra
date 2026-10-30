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
pub mod icon;
mod automate;
mod dialog;
mod inspector;
mod files;
mod insets;
mod instance;
mod login_item;
mod menu;
mod notify;
mod open_url;
mod share;
mod tray;
mod update;

use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use crate::automate::Drive;
use libra_components::{
    attach_osk, caret_byte_at, osk_edit, textarea_vertical_caret, Cursor, InputFocus, ItemKind,
    Keyboard, LaidTree, Role, SourceDecl,
};
use libra_core::{
    apply_window_chrome, file_dialog_title, file_name, shortcut_fires_while_focused, App,
    ChromeChange, Effect, FilePick, FrameInfo, Instant as LxInstant, ReflectAction, Rehydrate,
    RehydrateReport, Sub, WindowChrome, WindowOp, WindowPolicy,
};
use libra_draw::{Color, DrawCmd, FontStyle, Rect, Theme};
use libra_effects::Effects;
use libra_interp::Scope;
use libra_kv::Kv;
use libra_markup::Node;
use libra_raster::{Damage, Renderer};
use libra_text::{EditOp, EditState};
use tiny_skia::Pixmap;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{ElementState, Ime, KeyEvent, MouseButton, Touch, TouchPhase, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{CursorIcon, Fullscreen, Window, WindowId};

const WATCH_INTERVAL: Duration = Duration::from_millis(250);
/// Finger dwell before a touch selects the word under it (§10.1).
const LONG_PRESS: Duration = Duration::from_millis(500);
/// Movement inside this radius, in logical px, is still a long-press.
const TOUCH_SLOP: f64 = 8.0;

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
        xs.get(((xs.len().max(1) - 1) as f64 * q) as usize)
            .copied()
            .unwrap_or(0.0)
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

/// Environment variable set by `libra dev` when `libra.toml` declares an icon.
pub const ICON_PATH_ENV: &str = "LIBRA_ICON_PATH";

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
    /// Window / title-bar icon (PNG). Also read from [`ICON_PATH_ENV`] when unset.
    pub icon: Option<PathBuf>,
    /// Transparent title bar on macOS (content under the traffic lights).
    /// On Windows and Linux the same flag removes the native decorations
    /// (`with_decorations(false)`). Window dragging is the native title-bar
    /// strip plus empty `<Toolbar>` chrome (`Window::drag_window`). The
    /// content surface is not movable-by-background — that would steal
    /// Split and text drags.
    pub frameless: bool,
    /// Close button behavior. [`WindowPolicy::QuitOnClose`] is the default.
    pub window_policy: WindowPolicy,
    /// When set, a second launch raises the first process and exits 0.
    /// Requires [`Options::app_id`].
    pub single_instance: bool,
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
            icon: None,
            frameless: false,
            window_policy: WindowPolicy::QuitOnClose,
            single_instance: false,
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
    pub fn process_allowlist(
        mut self,
        programs: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.process_allowlist = programs.into_iter().map(Into::into).collect();
        self
    }
    pub fn icon(mut self, path: impl Into<PathBuf>) -> Self {
        self.icon = Some(path.into());
        self
    }
    pub fn frameless(mut self, on: bool) -> Self {
        self.frameless = on;
        self
    }
    pub fn window_policy(mut self, policy: WindowPolicy) -> Self {
        self.window_policy = policy;
        self
    }
    pub fn single_instance(mut self, on: bool) -> Self {
        self.single_instance = on;
        self
    }
}

/// How the shell obtains a [`ViewNode`] from app state (PLAN-PACKAGE P0).
///
/// Debug builds always interpret the embedded `.lx` string. Release builds
/// compiled with `libra_has_view_codegen` (set by the app's `build.rs`) call
/// the generated `build_view` fn; release builds without that cfg fall back to
/// the interpreter (transitional — e.g. integration-test fixtures).
pub struct ViewDriver<A: 'static> {
    lx: &'static str,
    #[cfg(not(debug_assertions))]
    build: Option<fn(&A) -> libra_components::ViewNode>,
    _marker: std::marker::PhantomData<A>,
}

impl<A> ViewDriver<A> {
    /// Interpret `lx` at runtime (debug always; release when no codegen).
    pub const fn new(lx: &'static str) -> Self {
        Self {
            lx,
            #[cfg(not(debug_assertions))]
            build: None,
            _marker: std::marker::PhantomData,
        }
    }

    /// Release-only: use the `build.rs`-generated view fn.
    #[cfg(not(debug_assertions))]
    pub const fn with_codegen(
        lx: &'static str,
        build: fn(&A) -> libra_components::ViewNode,
    ) -> Self {
        Self {
            lx,
            build: Some(build),
            _marker: std::marker::PhantomData,
        }
    }

    pub fn lx(&self) -> &'static str {
        self.lx
    }

    fn build_fn(&self) -> Option<fn(&A) -> libra_components::ViewNode> {
        #[cfg(not(debug_assertions))]
        {
            self.build
        }
        #[cfg(debug_assertions)]
        {
            None
        }
    }
}

impl<A> From<&'static str> for ViewDriver<A> {
    fn from(lx: &'static str) -> Self {
        ViewDriver::new(lx)
    }
}

/// Newest `.lx` mtime under the view directory (PLAN-SDK S3). `path`
/// is the root view file from `app!`; its parent is polled so a
/// composed component edit reloads too. A directory path is scanned
/// directly.
fn watch_signature(path: &Path) -> Option<SystemTime> {
    let dir = if path.is_dir() { path } else { path.parent()? };
    max_lx_mtime(dir).or_else(|| std::fs::metadata(path).ok()?.modified().ok())
}

fn max_lx_mtime(dir: &Path) -> Option<SystemTime> {
    let mut newest: Option<SystemTime> = None;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                if entry.file_name() != "target" && entry.file_name() != ".git" {
                    stack.push(p);
                }
                continue;
            }
            if p.extension().is_some_and(|e| e == "lx") {
                if let Ok(mtime) = std::fs::metadata(&p).and_then(|m| m.modified()) {
                    newest = Some(match newest {
                        Some(prev) => prev.max(mtime),
                        None => mtime,
                    });
                }
            }
        }
    }
    newest
}

/// Run an app in a native window until it is closed (embedded view,
/// no persistence) — the Phase 0 signature, kept for simple apps.
pub fn run<A: App + 'static>(
    view: ViewDriver<A>,
    title: &str,
    logical_size: (f32, f32),
) -> Result<(), Box<dyn std::error::Error>>
where
    A::Msg: Send + 'static,
{
    run_with::<A>(view, Options::new(title, logical_size))
}

fn is_shell_effect<M>(effect: &Effect<M>) -> bool {
    matches!(
        effect,
        Effect::SetTheme { .. }
            | Effect::ClipboardSet { .. }
            | Effect::PickFolder { .. }
            | Effect::Notify { .. }
            | Effect::WindowOp { .. }
            | Effect::LoginItem { .. }
            | Effect::LoginItemQuery { .. }
            | Effect::OpenUrl { .. }
            | Effect::Dialog { .. }
            | Effect::Share { .. }
            | Effect::PickFile { .. }
            | Effect::SaveFile { .. }
    )
}

pub fn run_with<A: App + 'static>(
    view: impl Into<ViewDriver<A>>,
    opts: Options,
) -> Result<(), Box<dyn std::error::Error>>
where
    A::Msg: Send + 'static,
{
    run_inner::<A>(view.into(), opts, None, None)
}

/// `run_with` plus §18.2 model rehydration: on exit — window close or
/// SIGTERM/SIGINT — the model snapshots to `<app-data>/model.json`. The
/// next launch restores it field by field (new or unreadable fields
/// become `Default` and are named; removed keys are ignored and named),
/// then still runs `init`'s cmds. A snapshot that is not a JSON object
/// is discarded. Requires `app_id` (for the snapshot location).
pub fn run_rehydrating<A>(
    view: impl Into<ViewDriver<A>>,
    opts: Options,
) -> Result<(), Box<dyn std::error::Error>>
where
    A: App + serde::Serialize + Rehydrate + 'static,
    A::Msg: Send + 'static,
{
    let view = view.into();
    let app_id = opts
        .app_id
        .clone()
        .ok_or("run_rehydrating requires Options::app_id")?;
    let path = snapshot_path(&app_id).ok_or("no home directory for model snapshot")?;

    let rehydrated: Option<A> = match std::fs::read_to_string(&path) {
        Ok(json) => {
            let (model, report) = A::rehydrate(&json);
            explain_rehydrate(&report);
            if report.invalid {
                None
            } else {
                Some(model)
            }
        }
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

    run_inner::<A>(view, opts, rehydrated, Some(save_hook))
}

fn snapshot_path(app_id: &str) -> Option<std::path::PathBuf> {
    directories::ProjectDirs::from("", "", app_id).map(|d| d.data_dir().join("model.json"))
}

fn explain_rehydrate(report: &RehydrateReport) {
    if report.invalid {
        eprintln!("[libra] model snapshot was not a JSON object; starting fresh");
        return;
    }
    if report.defaulted.is_empty() && report.removed.is_empty() {
        eprintln!("[libra] model rehydrated from previous session");
        return;
    }
    if !report.defaulted.is_empty() {
        eprintln!(
            "[libra] rehydrated; defaulted fields: {}",
            report.defaulted.join(", ")
        );
    }
    if !report.removed.is_empty() {
        eprintln!(
            "[libra] rehydrated; ignored removed fields: {}",
            report.removed.join(", ")
        );
    }
}

/// Write end of the pipe the signal handler uses. `-1` until armed.
static PERSIST_WRITE_FD: AtomicI32 = AtomicI32::new(-1);
static PERSIST_REQUESTED: AtomicBool = AtomicBool::new(false);

#[cfg(unix)]
extern "C" fn on_persist_signal(_: libc::c_int) {
    PERSIST_REQUESTED.store(true, Ordering::SeqCst);
    let fd = PERSIST_WRITE_FD.load(Ordering::SeqCst);
    if fd >= 0 {
        let byte = [1u8];
        unsafe {
            libc::write(fd, byte.as_ptr() as *const libc::c_void, 1);
        }
    }
}

/// Wake the shell on SIGTERM/SIGINT so it can snapshot before exit (§18.2).
/// The handler only writes a byte; a thread turns that into a user event.
/// Installed only for `run_rehydrating` — `run_with` must still die on
/// the default signal.
fn arm_persist_on_signal<M: Send + 'static>(proxy: EventLoopProxy<menu::ShellEvent<M>>) {
    #[cfg(unix)]
    {
        let mut fds = [0i32; 2];
        if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
            return;
        }
        PERSIST_WRITE_FD.store(fds[1], Ordering::SeqCst);
        unsafe {
            libc::signal(libc::SIGTERM, on_persist_signal as libc::sighandler_t);
            libc::signal(libc::SIGINT, on_persist_signal as libc::sighandler_t);
        }
        let read_fd = fds[0];
        std::thread::spawn(move || {
            let mut buf = [0u8; 8];
            loop {
                let n = unsafe {
                    libc::read(read_fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len())
                };
                if n <= 0 {
                    break;
                }
                let _ = proxy.send_event(menu::ShellEvent::PersistAndExit);
            }
        });
    }
    #[cfg(not(unix))]
    {
        let _ = proxy;
    }
}

fn persist_requested() -> bool {
    PERSIST_REQUESTED.swap(false, Ordering::SeqCst)
}

type SaveHook<A> = Box<dyn Fn(&A)>;

fn run_inner<A: App + 'static>(
    view: ViewDriver<A>,
    opts: Options,
    rehydrated: Option<A>,
    save_hook: Option<SaveHook<A>>,
) -> Result<(), Box<dyn std::error::Error>>
where
    A::Msg: Send + 'static,
{
    let mut opts = opts;
    if opts.icon.is_none() {
        if let Ok(path) = std::env::var(ICON_PATH_ENV) {
            if !path.is_empty() {
                opts.icon = Some(PathBuf::from(path));
            }
        }
    }

    #[cfg(not(debug_assertions))]
    let use_codegen = view.build_fn().is_some();
    #[cfg(debug_assertions)]
    let use_codegen = false;

    let (ast, registry, view_build) = if use_codegen {
        (
            None,
            libra_markup::ComponentRegistry::default(),
            view.build_fn(),
        )
    } else {
        let ast =
            Some(libra_markup::parse(view.lx()).map_err(|e| format!("markup parse error: {e}"))?);
        let registry = opts
            .watch
            .as_ref()
            .and_then(|p| p.parent())
            .map(|dir| libra_markup::ComponentRegistry::load_from_dir(dir).unwrap_or_default())
            .unwrap_or_default();
        (ast, registry, None)
    };
    let (app, init_cmds) = A::init();
    let app = rehydrated.unwrap_or(app);

    let mut builder = EventLoop::<menu::ShellEvent<A::Msg>>::with_user_event();
    #[cfg(target_os = "macos")]
    if opts.window_policy == WindowPolicy::CloseToTray {
        use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
        builder.with_activation_policy(ActivationPolicy::Accessory);
    }
    let event_loop = builder.build()?;
    let proxy = event_loop.create_proxy();
    let update_proxy = proxy.clone();
    if opts.single_instance {
        let raise = update_proxy.clone();
        match instance::claim(opts.app_id.as_deref(), move || {
            let _ = raise.send_event(menu::ShellEvent::RaiseWindow);
        }) {
            Ok(instance::Role::Secondary) => return Ok(()),
            Ok(instance::Role::Primary) => {}
            Err(e) => eprintln!("[libra] single instance unavailable ({e}); continuing"),
        }
    }
    if save_hook.is_some() {
        arm_persist_on_signal(proxy.clone());
    }
    let menu_proxy = proxy.clone();
    muda::MenuEvent::set_event_handler(Some(move |event: muda::MenuEvent| {
        let _ = menu_proxy.send_event(menu::ShellEvent::Menu(event.id));
    }));
    let tray_proxy = update_proxy.clone();
    tray_icon::menu::MenuEvent::set_event_handler(Some(
        move |event: tray_icon::menu::MenuEvent| {
            let _ = tray_proxy.send_event(menu::ShellEvent::TrayClick(event.id.0));
        },
    ));
    let sink: libra_effects::Sink<A::Msg> = Arc::new(move |msg| {
        // The window may be closing; a dead proxy just drops the msg.
        let _ = proxy.send_event(menu::ShellEvent::App(msg));
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
    let wake_proxy = event_loop.create_proxy();
    effects.set_stream_wake(Arc::new(move || {
        let _ = wake_proxy.send_event(menu::ShellEvent::Streams);
    }));
    let automation_socket = automate::start(update_proxy.clone());
    let (boot_effects, worker_cmds): (Vec<_>, Vec<_>) =
        init_cmds.into_iter().partition(is_shell_effect);
    effects.execute_all(worker_cmds);

    let watch_mtime = opts.watch.as_deref().and_then(watch_signature);
    let theme_mtime = opts
        .theme_file
        .as_deref()
        .and_then(|p| std::fs::metadata(p).ok()?.modified().ok());
    let theme = match &opts.theme_file {
        Some(path) => match std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|s| Theme::from_toml_str(&s))
        {
            Ok(t) => {
                eprintln!("[libra] theme '{}' loaded from {}", t.name, path.display());
                t
            }
            Err(e) => {
                eprintln!(
                    "[libra] theme file {}: {e}; using libra-dark",
                    path.display()
                );
                Theme::dark()
            }
        },
        None => Theme::dark(),
    };
    // §19.2: validate the app's shortcut declarations up front — a bad
    // chord or action name is a startup error, not a silent dead key.
    let shortcut_table = libra_core::build_shortcut_table::<A::Msg>(A::shortcuts())
        .unwrap_or_else(|e| panic!("[libra] invalid shortcuts: {e}"));
    let window_chrome = WindowChrome::at(opts.title.clone(), opts.logical_size.0, opts.logical_size.1);
    let insets = insets::desktop_insets(opts.frameless, false);
    let mut shell = Shell {
        opts,
        app,
        ast,
        registry,
        view_build,
        effects,
        renderer: Renderer::new(),
        laid: None,
        overlay: None,
        cursor: (0.0, 0.0),
        cursor_icon: Cursor::Default,
        scroll_state: libra_components::ScrollState::new(),
        shortcut_table,
        drag: None,
        text_click: None,
        menu_anchor: (0.0, 0.0),
        native_menus: menu::NativeMenus::new(),
        tray: tray::TrayHost::new(),
        window_hidden: false,
        window_chrome,
        insets,
        boot_effects,
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
        pumping_streams: false,
        focus: None,
        sticky_x: None,
        preserve_sticky: false,
        touch: None,
        touch_handles: false,
        osk: None,
        summon_osk: false,
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
        update_proxy,
        update_started: false,
        automation_socket,
        action_log: inspector::ActionLog::default(),
        inspector: None,
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
    Text,
}

struct TextClick {
    id: String,
    at: Instant,
    count: u8,
    x: f32,
    y: f32,
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

struct TouchGesture {
    id: u64,
    x: f64,
    y: f64,
    origin_x: f64,
    origin_y: f64,
    started: Instant,
    fired: bool,
    dragged: bool,
}

struct Shell<A: App>
where
    A::Msg: 'static,
{
    opts: Options,
    app: A,
    ast: Option<Node>,
    /// §7.2 composed components discovered alongside the root view file.
    registry: libra_markup::ComponentRegistry,
    /// Release codegen entry (PLAN-PACKAGE P0). `None` ⇒ interpret `ast`.
    view_build: Option<fn(&A) -> libra_components::ViewNode>,
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
    /// Pointer drag target (M5 Split divider / Slider track / text
    /// selection), keyed by the control's test_id while the left
    /// button is down.
    drag: Option<DragTarget>,
    /// Last text-field click, for double/triple-click word/line select.
    text_click: Option<TextClick>,
    /// M7 `<ContextMenu>` anchor — the last right-click point (logical
    /// px), a deterministic layout input like scroll offsets.
    menu_anchor: (f32, f32),
    /// §19.3 native menubar / context ids.
    native_menus: menu::NativeMenus,
    /// PLAN-RESIDENCY R1 status item.
    tray: tray::TrayHost,
    /// Close-to-tray hides the window and stops painting. Sources stay mounted.
    window_hidden: bool,
    /// N6 chrome. Applied before winit so a refused title or size never
    /// reaches the window.
    window_chrome: WindowChrome,
    /// N5. Recomputed when fullscreen changes. Passed into every layout.
    insets: libra_components::Insets,
    /// Shell-handled effects from `init`, applied once the window exists
    /// so a login-item query reads the real host instead of the headless ack.
    boot_effects: Vec<Effect<A::Msg>>,
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
    /// Re-entry guard: delivering a stream line relayouts, which pumps again.
    pumping_streams: bool,
    /// §10.1 focus: focused Input id + editing state + onChange name.
    focus: Option<(String, EditState, String)>,
    /// Pixel column remembered across textarea Up/Down (§10.1).
    sticky_x: Option<f32>,
    preserve_sticky: bool,
    /// Finger currently down on a text field, if any.
    touch: Option<TouchGesture>,
    /// Selection grips after a long-press.
    touch_handles: bool,
    /// Touch on-screen keyboard (§19.1). Pointer clicks leave this empty.
    osk: Option<Keyboard>,
    /// The click in progress came from a touch, so text focus summons `osk`.
    summon_osk: bool,
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
    /// PLAN-RELEASE: wakes the loop when a signed feed has a newer build.
    update_proxy: winit::event_loop::EventLoopProxy<menu::ShellEvent<A::Msg>>,
    update_started: bool,
    /// Socket path to unlink when the window loop ends (§16).
    automation_socket: Option<PathBuf>,
    /// Recent actions for the inspector and the `log` verb (§18.4).
    action_log: inspector::ActionLog,
    /// Dev-only second window. `None` when `LIBRA_INSPECTOR` is unset.
    inspector: Option<InspectorGfx>,
}

struct InspectorGfx {
    window: Arc<Window>,
    _context: softbuffer::Context<Arc<Window>>,
    surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
    renderer: Renderer,
}

impl<A: App> Drop for Shell<A>
where
    A::Msg: 'static,
{
    fn drop(&mut self) {
        if let Some(path) = self.automation_socket.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

impl<A: App + 'static> automate::Drive for Shell<A>
where
    A::Msg: Send + 'static,
{
    fn nodes(&self) -> Vec<automate::NodeSnap> {
        let Some(laid) = &self.laid else {
            return Vec::new();
        };
        laid.items
            .iter()
            .map(|item| automate::NodeSnap {
                test_id: item.semantics.test_id.clone(),
                role: format!("{:?}", item.semantics.role),
                text: item.semantics.text.clone(),
                x: item.rect.x,
                y: item.rect.y,
                w: item.rect.w,
                h: item.rect.h,
            })
            .collect()
    }

    fn dispatch_action(&mut self, action: &str, arg: Option<String>) -> Result<(), String> {
        let msg = match arg {
            Some(value) => A::Msg::action_with_str(action, value),
            None => A::Msg::action_by_name(action),
        };
        match msg {
            Some(msg) => {
                self.dispatch(msg);
                Ok(())
            }
            None => Err(format!("action '{action}' did not resolve")),
        }
    }

    fn fields(&self) -> Vec<(String, String)> {
        automate::visible_fields(A::field_names(), A::redacted_fields(), |name| {
            self.app.field(name)
        })
    }

    fn schema(&self) -> automate::SchemaSnap {
        automate::schema_snap(
            A::Msg::action_names(),
            A::field_names(),
            A::redacted_fields(),
        )
    }

    fn log(&self) -> Vec<String> {
        self.action_log.lines()
    }
}

impl<A: App + 'static> Shell<A>
where
    A::Msg: Send + 'static,
{
    fn scale(&self) -> f32 {
        self.gfx
            .as_ref()
            .map_or(1.0, |g| g.window.scale_factor() as f32)
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
        let has_stream_event_action = |m: &str| {
            A::Msg::action_with_stream_event(m, libra_core::StreamEvent::Dropped(0)).is_some()
        };
        let scope = Scope {
            field: &field,
            has_action: &has_action,
            has_str_action: &has_str_action,
            collection_rows: &collection_rows,
            has_frame_info_action: &has_frame_info_action,
            has_instant_action: &has_instant_action,
            has_stream_event_action: &has_stream_event_action,
            binding_ctx: "State",
        };
        let view_result = if let Some(build) = self.view_build {
            Ok(build(&self.app))
        } else {
            libra_interp::resolve(
                self.ast.as_ref().expect("ast or codegen build fn"),
                &scope,
                &self.registry,
            )
        };
        match view_result {
            Ok(view) => match libra_components::layout_scrolled(
                &view,
                (logical.width, logical.height),
                &self.renderer,
                &self.theme,
                &self.scroll_state,
                &self.insets,
            ) {
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
                            ItemKind::Source(SourceDecl::Frames { on_tick }) => {
                                Some(on_tick.clone())
                            }
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
                    if let Some(keyboard) = self.osk {
                        attach_osk(&mut laid, keyboard, &self.theme);
                    }
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
                    self.sync_ime();
                    self.pump_streams();
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
        #[cfg(debug_assertions)]
        if inspector::enabled() || self.automation_socket.is_some() {
            self.action_log.push(&inspector::brief_debug(&msg));
            if let Some(gfx) = &self.inspector {
                gfx.window.request_redraw();
            }
        }
        let effects = self.app.update(msg);
        self.handle_effects(effects);
    }

    fn handle_effects(&mut self, effects: Vec<Effect<A::Msg>>) {
        // SetTheme is presentation state (§11): the shell applies it;
        // ClipboardSet is a UI-thread OS call (arboard) acked inline;
        // PickFolder is the same for rfd's folder sheet;
        // Notify is the same for the OS banner (PLAN-NOTIFY NT1);
        // OpenUrl, Dialog, and Share are the same (PLAN-NATIVE-INTEGRATIONS
        // N1, N2, N4);
        // everything else goes to the effects runtime.
        let mut rest = Vec::with_capacity(effects.len());
        let mut acks: Vec<A::Msg> = Vec::new();
        for effect in effects {
            if let Effect::SetTheme { name } = &effect {
                match Theme::by_name(name) {
                    Some(t) => self.theme = t,
                    None => eprintln!(
                        "[libra] unknown theme '{name}' (built in: libra-dark, libra-light)"
                    ),
                }
                continue;
            }
            if let Effect::ClipboardSet { text, on_done } = &effect {
                let result =
                    match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(text.clone())) {
                        Ok(()) => libra_core::ClipboardResult::Ok,
                        Err(e) => libra_core::ClipboardResult::Err(e.to_string()),
                    };
                if let Some(ctor) = on_done {
                    acks.push(ctor(result));
                }
                continue;
            }
            if let Effect::PickFolder { on_done } = &effect {
                let result = match rfd::FileDialog::new().set_title("Open vault").pick_folder() {
                    Some(path) => {
                        libra_core::FolderPick::Picked(path.to_string_lossy().into_owned())
                    }
                    None => libra_core::FolderPick::Cancelled,
                };
                acks.push(on_done(result));
                continue;
            }
            if let Effect::Notify {
                note,
                on_activate,
                on_done,
            } = &effect
            {
                let click = on_activate.clone().map(|action| {
                    let proxy = self.update_proxy.clone();
                    Box::new(move || {
                        let _ = proxy.send_event(menu::ShellEvent::NotifyActivate(action));
                    }) as Box<dyn FnOnce() + Send>
                });
                let aumid = update::bundled_app_user_model_id();
                let result = notify::show(note, &self.opts.title, aumid.as_deref(), click);
                if let Some(ctor) = on_done {
                    acks.push(ctor(result));
                }
                continue;
            }
            if let Effect::WindowOp { op } = &effect {
                self.apply_window(op.clone());
                continue;
            }
            if let Effect::LoginItem { enabled, on_done } = &effect {
                let result =
                    login_item::set(self.opts.app_id.as_deref(), &self.opts.title, *enabled);
                acks.push(on_done(result));
                continue;
            }
            if let Effect::LoginItemQuery { on_done } = &effect {
                acks.push(on_done(login_item::query(self.opts.app_id.as_deref())));
                continue;
            }
            if let Effect::OpenUrl { url, on_done } = &effect {
                let result = open_url::launch(url);
                if let Some(ctor) = on_done {
                    acks.push(ctor(result));
                }
                continue;
            }
            if let Effect::Dialog { spec, on_done } = &effect {
                acks.push(on_done(dialog::show(spec)));
                continue;
            }
            if let Effect::Share { text, url, on_done } = &effect {
                let result = match libra_core::share_payload(text, url.as_deref()) {
                    Ok(payload) => share::copy(&payload),
                    Err(result) => result,
                };
                if let Some(ctor) = on_done {
                    acks.push(ctor(result));
                }
                continue;
            }
            if let Effect::PickFile { title, on_done } = &effect {
                let title = file_dialog_title(title, "Open");
                acks.push(on_done(files::pick(&title)));
                continue;
            }
            if let Effect::SaveFile {
                title,
                name,
                on_done,
            } = &effect
            {
                let title = file_dialog_title(title, "Save");
                let name = match name.as_deref() {
                    Some(name) => match file_name(name) {
                        Ok(name) => Some(name),
                        Err(reason) => {
                            acks.push(on_done(FilePick::Failed(reason)));
                            continue;
                        }
                    },
                    None => None,
                };
                acks.push(on_done(files::save(&title, name.as_deref())));
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
        self.sync_native_menu();
        self.sync_tray();
        if !self.window_hidden {
            if let Some(gfx) = &self.gfx {
                gfx.window.request_redraw();
            }
        }
    }

    fn sync_tray(&mut self) {
        self.tray.sync::<A::Msg>(self.app.tray());
    }

    fn apply_window(&mut self, op: WindowOp) {
        match apply_window_chrome(&mut self.window_chrome, &op) {
            ChromeChange::Refused(reason) => {
                eprintln!("[libra] window op refused: {reason}");
                return;
            }
            ChromeChange::Applied => {}
        }
        match &op {
            WindowOp::Quit => {
                let _ = self
                    .update_proxy
                    .send_event(menu::ShellEvent::PersistAndExit);
            }
            WindowOp::Hide => {
                self.window_hidden = true;
                if let Some(gfx) = &self.gfx {
                    gfx.window.set_visible(false);
                }
            }
            WindowOp::Show | WindowOp::Raise => {
                self.window_hidden = false;
                if let Some(gfx) = &self.gfx {
                    gfx.window.set_visible(true);
                    gfx.window.focus_window();
                    gfx.window.request_redraw();
                }
            }
            WindowOp::Title(_) => {
                self.opts.title = self.window_chrome.title.clone();
                if let Some(gfx) = &self.gfx {
                    gfx.window.set_title(&self.window_chrome.title);
                }
            }
            WindowOp::Fullscreen(on) => {
                self.insets = insets::desktop_insets(self.opts.frameless, *on);
                if let Some(gfx) = &self.gfx {
                    gfx.window
                        .set_fullscreen((*on).then_some(Fullscreen::Borderless(None)));
                }
            }
            WindowOp::Resize { .. } => {
                self.opts.logical_size = (self.window_chrome.width, self.window_chrome.height);
                if let Some(gfx) = &self.gfx {
                    let _ = gfx.window.request_inner_size(LogicalSize::new(
                        self.window_chrome.width as f64,
                        self.window_chrome.height as f64,
                    ));
                }
            }
        }
    }

    fn sync_native_menu(&mut self) {
        let decl = self.app.menu();
        let window = self.gfx.as_ref().map(|g| g.window.as_ref());
        menu::sync_menubar::<A::Msg>(&mut self.native_menus, decl, window);
    }

    fn dispatch_menu(&mut self, id: muda::MenuId) {
        let Some((action, arg)) = self.native_menus.lookup(&id) else {
            return;
        };
        let msg = match arg {
            Some(a) => A::Msg::action_with_str(&action, a),
            None => A::Msg::action_by_name(&action),
        };
        if let Some(msg) = msg {
            self.dispatch(msg);
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
        let want = self
            .app
            .subscriptions()
            .iter()
            .any(|s| matches!(s, Sub::Frames { .. }))
            || !self.markup_frame_handlers.is_empty()
            || self.effects.store().has_live_ingest();
        if want && !self.frames_active {
            self.next_frame_at = Instant::now();
        }
        self.frames_active = want;
    }

    /// Drain stream buffers for mounted `<Stream>` ids and dispatch each
    /// event as an action (§12.5, PLAN-STREAM-DELIVERY). Unmounted ids
    /// are killed by the runtime. A line's own relayout re-enters this
    /// method; the guard keeps that from nesting.
    fn pump_streams(&mut self) {
        if self.pumping_streams || self.gfx.is_none() {
            return;
        }
        self.pumping_streams = true;
        for _ in 0..8 {
            let handlers = self.mounted_stream_handlers();
            let mounted: Vec<String> = handlers.iter().map(|(id, _)| id.clone()).collect();
            let batch = self.effects.sync_streams(&mounted);
            if batch.is_empty() {
                break;
            }
            for (id, event) in batch {
                let handler = handlers
                    .iter()
                    .find(|(mounted_id, _)| mounted_id == &id)
                    .map(|(_, name)| name.clone());
                let Some(handler) = handler else { continue };
                let Some(msg) = A::Msg::action_with_stream_event(&handler, event) else {
                    continue;
                };
                self.dispatch(msg);
            }
        }
        self.pumping_streams = false;
    }

    /// Markup `<Stream>` handlers, then ambient `Sub::Stream` ids markup
    /// did not declare (PLAN-STREAM-DELIVERY D5). A hidden window still
    /// has `gfx`, so close-to-tray keeps delivering.
    fn mounted_stream_handlers(&self) -> Vec<(String, String)> {
        let ambient: Vec<(String, String)> = self
            .app
            .subscriptions()
            .into_iter()
            .filter_map(|sub| match sub {
                Sub::Stream { id, on_event, .. } => Some((id, on_event)),
                Sub::Frames { .. } => None,
            })
            .collect();
        libra_core::union_stream_handlers(&self.markup_streams, &ambient)
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
            if self
                .running_intervals
                .iter()
                .any(|(ms, n, _)| *ms == every_ms && *n == name)
            {
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
        let mhz = self
            .gfx
            .as_ref()
            .and_then(|g| g.window.current_monitor())
            .and_then(|m| m.refresh_rate_millihertz());
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
            Sub::Stream { .. } => None,
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
                .find(|i| {
                    let scrolls = match &i.kind {
                        ItemKind::Scroll => true,
                        ItemKind::VirtualViewport { follow: false } => true,
                        _ => false,
                    };
                    scrolls && i.rect.contains(lx, ly)
                })
                .and_then(|i| i.semantics.test_id.clone())
        });
        let Some(id) = target else { return };
        let pos = self.scroll_state.entry(id).or_insert(0.0);
        *pos = (*pos + dy).max(0.0);
        self.relayout();
        self.request_redraw();
    }

    fn click(&mut self) {
        self.touch_handles = false;
        let scale = self.scale();
        let (lx, ly) = (self.cursor.0 as f32 / scale, self.cursor.1 as f32 / scale);
        let Some(laid) = &self.laid else { return };
        // M5: pressing a `<Split>` divider gutter starts a drag instead
        // of a click (the gutter is not a pressable item).
        let target = laid.items.iter().rev().find_map(|i| match &i.kind {
            ItemKind::SplitDivider { split, .. } if i.rect.contains(lx, ly) => {
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
            // Empty toolbar chrome in a frameless window is the title-bar
            // drag region. AppKit `movableByWindowBackground` cannot be
            // used here: the whole window is one softbuffer surface, so
            // it would steal Split and text drags.
            let chrome = self.opts.frameless && laid.chrome_drag_at(lx, ly);
            let had_focus = self.focus.is_some();
            if had_focus || self.osk.is_some() {
                self.osk = None;
                if had_focus {
                    self.dispatch_focused_submit();
                    self.focus = None;
                } else {
                    self.relayout();
                }
                self.request_redraw();
            }
            if chrome {
                if let Some(gfx) = &self.gfx {
                    let _ = gfx.window.drag_window();
                }
            }
            return;
        };
        let osk_label = match &hit.kind {
            ItemKind::OskKey { label } => Some(label.clone()),
            _ => None,
        };
        if let Some(label) = osk_label {
            if let Some(op) = osk_edit(&label) {
                self.apply_edit(op);
            }
            return;
        }
        let keyboard = match &hit.kind {
            ItemKind::Input { keyboard, .. }
            | ItemKind::Search { keyboard, .. }
            | ItemKind::Textarea { keyboard, .. } => *keyboard,
            _ => Keyboard::None,
        };
        let name = hit.on_press.clone().expect("hit item has a handler");
        if hit.semantics.role == Role::TextInput {
            let id = hit
                .semantics
                .test_id
                .clone()
                .expect("Input has a testId (load-enforced)");
            let text = hit.semantics.text.clone();
            let focus = self.focus_paint();
            let byte = caret_byte_at(hit, lx, ly, &self.renderer, &self.theme, focus.as_ref())
                .unwrap_or(text.len());
            let already = self
                .focus
                .as_ref()
                .is_some_and(|(fid, e, _)| fid == &id && e.text() == text);
            let switching = self.focus.as_ref().is_some_and(|(fid, _, _)| fid != &id);
            if switching {
                self.dispatch_focused_submit();
            }
            let shift = self.modifiers.shift_key();
            if !already {
                self.focus = Some((id.clone(), EditState::new(text), name));
            }
            self.osk = if self.summon_osk && keyboard != Keyboard::None {
                Some(keyboard)
            } else {
                None
            };
            let count = self.bump_text_click(&id, lx, ly);
            let op = match count {
                2 => EditOp::SelectWord,
                3 => EditOp::SelectLine,
                _ => EditOp::SetCaret {
                    byte,
                    select: shift && already,
                },
            };
            self.apply_edit(op);
            if self.summon_osk {
                self.relayout();
            }
            self.drag = Some(DragTarget::Text);
            return;
        }
        let on_press_arg = hit.on_press_arg.clone();
        self.dispatch_focused_submit();
        self.focus = None;
        self.osk = None;
        // Item-scoped handler (`onPress={toggle(item)}`, §6.2): dispatch the
        // String-payload action with the resolved loop-binding value.
        let msg = match on_press_arg {
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
            Some(DragTarget::Text) => Cursor::Text,
            None => {
                let scale = self.scale();
                let (lx, ly) = (self.cursor.0 as f32 / scale, self.cursor.1 as f32 / scale);
                self.laid
                    .as_ref()
                    .map_or(Cursor::Default, |laid| laid.cursor_at(lx, ly))
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
            ItemKind::ListItem {
                on_menu: Some((name, arg)),
                ..
            } if i.rect.contains(lx, ly) && i.clip.map_or(true, |c| c.contains(lx, ly)) => {
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
        if let Some(decl) = self.app.context_menu() {
            if let Some(gfx) = &self.gfx {
                menu::show_context::<A::Msg>(
                    &mut self.native_menus,
                    &decl,
                    &gfx.window,
                    self.cursor,
                );
            }
        }
    }

    /// Active pointer drag (M5 Split / Slider): re-derive the fraction
    /// from the pointer within the control's rect and dispatch its
    /// String-payload handler (`{:.4}` — identical formatting to the
    /// harness's `resize_split`/`set_slider`, so both paths agree).
    fn bump_text_click(&mut self, id: &str, x: f32, y: f32) -> u8 {
        const WINDOW: Duration = Duration::from_millis(400);
        const SLOP: f32 = 6.0;
        let now = Instant::now();
        let next = match &self.text_click {
            Some(prev)
                if prev.id == id
                    && now.duration_since(prev.at) <= WINDOW
                    && (prev.x - x).abs() <= SLOP
                    && (prev.y - y).abs() <= SLOP =>
            {
                (prev.count % 3) + 1
            }
            _ => 1,
        };
        self.text_click = Some(TextClick {
            id: id.to_string(),
            at: now,
            count: next,
            x,
            y,
        });
        next
    }

    fn text_drag_move(&mut self) {
        let scale = self.scale();
        let (lx, ly) = (self.cursor.0 as f32 / scale, self.cursor.1 as f32 / scale);
        let focus = self.focus_paint();
        let Some(id) = self.focus.as_ref().map(|(id, _, _)| id.clone()) else {
            return;
        };
        let Some(laid) = &self.laid else { return };
        let Some(item) = laid
            .items
            .iter()
            .find(|i| i.semantics.test_id.as_deref() == Some(id.as_str()))
        else {
            return;
        };
        let Some(byte) = caret_byte_at(item, lx, ly, &self.renderer, &self.theme, focus.as_ref())
        else {
            return;
        };
        self.apply_edit(EditOp::SetCaret { byte, select: true });
    }

    fn drag_move(&mut self) {
        let Some(target) = self.drag.clone() else {
            return;
        };
        if matches!(target, DragTarget::Text) {
            self.text_drag_move();
            return;
        }
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
            DragTarget::Text => None,
        };
        let Some((rect, handler, lo, hi)) = found else {
            return;
        };
        if rect.w <= 0.0 {
            return;
        }
        let fraction = ((lx - rect.x) / rect.w).clamp(lo, hi);
        let msg =
            A::Msg::action_with_str(&handler, format!("{fraction:.4}")).unwrap_or_else(|| {
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
            // `<Search onSubmit>` / `<Input onSubmit>` (§7.1): Enter submits
            // the unit action. The value is already in State via onChange.
            Key::Named(NamedKey::Enter) => {
                if let Some(name) = self.focused_text_submit() {
                    let msg = A::Msg::action_by_name(&name)
                        .unwrap_or_else(|| panic!("'{name}' is not a unit Action variant"));
                    self.dispatch(msg);
                    return;
                }
                None
            }
            Key::Named(NamedKey::ArrowUp) if self.focused_is_textarea() => {
                self.visual_arrow(-1, shift);
                return;
            }
            Key::Named(NamedKey::ArrowDown) if self.focused_is_textarea() => {
                self.visual_arrow(1, shift);
                return;
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
        if !self.preserve_sticky {
            self.sticky_x = None;
        }
        let Some((id, mut edit, on_change)) = self.focus.take() else {
            return;
        };
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
        self.sync_ime();
        self.request_redraw();
    }

    fn focus_paint(&self) -> Option<InputFocus> {
        self.focus.as_ref().map(|(id, e, _)| {
            let preedit = e.preedit().cloned();
            let caret = e.display_caret();
            let selection = if preedit.is_some() { caret..caret } else { e.selection() };
            InputFocus {
                test_id: id.clone(),
                caret,
                selection,
                preedit,
                handles: self.touch_handles,
                show_caret: e.show_caret(),
            }
        })
    }

    /// Arrow Up/Down in a textarea follows soft-wrapped rows. The pure
    /// core's `EditOp::Up` stays hard-line; it has no font metrics.
    fn visual_arrow(&mut self, dir: i32, select: bool) {
        if self.focus.as_ref().is_some_and(|(_, e, _)| e.preedit().is_some()) {
            let text = self
                .focus
                .as_ref()
                .and_then(|(_, e, _)| e.preedit().map(|p| p.text.clone()))
                .unwrap_or_default();
            if text.is_empty() {
                self.apply_edit(EditOp::Preedit { text: String::new(), cursor: None });
            } else {
                self.apply_edit(EditOp::Commit(text));
            }
        }
        let computed = {
            let (id, caret) = match &self.focus {
                Some((id, edit, _)) => (id.clone(), edit.caret()),
                None => return,
            };
            let Some(laid) = &self.laid else { return };
            let Some(item) = laid
                .items
                .iter()
                .find(|i| i.semantics.test_id.as_deref() == Some(id.as_str()))
            else {
                return;
            };
            textarea_vertical_caret(item, caret, dir, self.sticky_x, &self.renderer, &self.theme)
        };
        let Some((byte, x)) = computed else { return };
        self.sticky_x = x;
        self.preserve_sticky = true;
        self.apply_edit(EditOp::SetCaret { byte, select });
        self.preserve_sticky = false;
    }

    fn sync_ime(&self) {
        let Some(gfx) = &self.gfx else { return };
        let allowed = self.focus.is_some();
        gfx.window.set_ime_allowed(allowed);
        if !allowed {
            return;
        }
        let Some((id, _, _)) = &self.focus else { return };
        let Some(laid) = &self.laid else { return };
        let Some(item) = laid
            .items
            .iter()
            .find(|i| i.semantics.test_id.as_deref() == Some(id.as_str()))
        else {
            return;
        };
        let rect = item.rect;
        gfx.window.set_ime_cursor_area(
            winit::dpi::LogicalPosition::new(rect.x as f64, rect.y as f64),
            winit::dpi::LogicalSize::new(rect.w.max(1.0) as f64, rect.h.max(1.0) as f64),
        );
    }

    fn ime(&mut self, ime: Ime) {
        if self.focus.is_none() {
            return;
        }
        match ime {
            Ime::Enabled => self.sync_ime(),
            Ime::Disabled => {
                self.apply_edit(EditOp::Preedit { text: String::new(), cursor: None });
            }
            Ime::Preedit(text, cursor) => {
                self.apply_edit(EditOp::Preedit { text, cursor });
            }
            Ime::Commit(text) => self.apply_edit(EditOp::Commit(text)),
        }
    }

    fn on_touch(&mut self, touch: Touch) {
        let scale = self.scale() as f64;
        let x = touch.location.x / scale;
        let y = touch.location.y / scale;
        match touch.phase {
            TouchPhase::Started => {
                self.cursor = (touch.location.x, touch.location.y);
                self.touch_handles = false;
                self.summon_osk = true;
                self.click();
                self.summon_osk = false;
                if self.focus.is_some() {
                    self.touch = Some(TouchGesture {
                        id: touch.id,
                        x,
                        y,
                        origin_x: x,
                        origin_y: y,
                        started: Instant::now(),
                        fired: false,
                        dragged: false,
                    });
                }
            }
            TouchPhase::Moved => {
                let Some(g) = &mut self.touch else { return };
                if g.id != touch.id {
                    return;
                }
                g.x = x;
                g.y = y;
                let dx = x - g.origin_x;
                let dy = y - g.origin_y;
                if dx * dx + dy * dy > TOUCH_SLOP * TOUCH_SLOP {
                    g.dragged = true;
                }
                if g.dragged {
                    self.cursor = (touch.location.x, touch.location.y);
                    self.extend_selection_to_cursor();
                }
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                if self.touch.as_ref().is_some_and(|g| g.id == touch.id) {
                    self.touch = None;
                    self.drag = None;
                }
            }
        }
    }

    fn extend_selection_to_cursor(&mut self) {
        let scale = self.scale();
        let (lx, ly) = (self.cursor.0 as f32 / scale, self.cursor.1 as f32 / scale);
        let byte = {
            let Some((id, _, _)) = &self.focus else { return };
            let id = id.clone();
            let Some(laid) = &self.laid else { return };
            let Some(item) = laid
                .items
                .iter()
                .find(|i| i.semantics.test_id.as_deref() == Some(id.as_str()))
            else {
                return;
            };
            let focus = self.focus_paint();
            caret_byte_at(item, lx, ly, &self.renderer, &self.theme, focus.as_ref()).unwrap_or(0)
        };
        self.apply_edit(EditOp::SetCaret { byte, select: true });
    }

    fn poll_long_press(&mut self) {
        let ready = self.touch.as_ref().is_some_and(|g| {
            !g.fired && !g.dragged && g.started.elapsed() >= LONG_PRESS
        });
        if !ready {
            return;
        }
        if let Some(g) = &mut self.touch {
            g.fired = true;
        }
        self.touch_handles = true;
        self.apply_edit(EditOp::SelectWord);
    }

    /// Is the focused control a `Textarea`? Decides the multi-line key
    /// bindings (Enter/Up/Down) and paste newline handling (§10.1).
    fn focused_is_textarea(&self) -> bool {
        let Some((id, ..)) = &self.focus else {
            return false;
        };
        let Some(laid) = &self.laid else { return false };
        laid.items.iter().any(|i| {
            i.semantics.test_id.as_deref() == Some(id.as_str())
                && matches!(i.kind, libra_components::ItemKind::Textarea { .. })
        })
    }

    fn dispatch_focused_submit(&mut self) {
        let Some(name) = self.focused_text_submit() else {
            return;
        };
        let msg = A::Msg::action_by_name(&name)
            .unwrap_or_else(|| panic!("'{name}' is not a unit Action variant"));
        self.dispatch(msg);
    }

    /// Unit-action name for Enter on a focused `<Search onSubmit>` or
    /// `<Input onSubmit>` (§7.1). `None` if focus is not a Search/Input,
    /// or the control omitted it.
    fn focused_text_submit(&self) -> Option<String> {
        let (id, ..) = self.focus.as_ref()?;
        let laid = self.laid.as_ref()?;
        laid.items.iter().find_map(|i| {
            if i.semantics.test_id.as_deref() != Some(id.as_str()) {
                return None;
            }
            match &i.kind {
                libra_components::ItemKind::Search { on_submit, .. } => on_submit.clone(),
                libra_components::ItemKind::Input { on_submit, .. } => on_submit.clone(),
                _ => None,
            }
        })
    }

    #[allow(dead_code)]
    fn focused_search_submit(&self) -> Option<String> {
        self.focused_text_submit()
    }

    /// Dev hot reload: swap the AST if any `.lx` under the view
    /// directory changed (PLAN-SDK S3 — the watch path is the root
    /// view file; its parent directory is polled). On a parse error
    /// keep the last good view and overlay the diagnostic.
    fn check_watch(&mut self) {
        let Some(path) = self.opts.watch.clone() else {
            return;
        };
        let Some(mtime) = watch_signature(&path) else {
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
                self.ast = Some(ast);
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
        let Some(path) = self.opts.theme_file.clone() else {
            return;
        };
        let Some(mtime) = std::fs::metadata(&path)
            .ok()
            .and_then(|m| m.modified().ok())
        else {
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

    fn inspector_fields(&self) -> Vec<(String, String)> {
        let mut rows = self.fields();
        for name in A::redacted_fields() {
            rows.push(((*name).to_string(), "(redacted)".into()));
        }
        rows
    }

    fn open_inspector(&mut self, event_loop: &ActiveEventLoop) {
        if self.inspector.is_some() || !inspector::enabled() {
            return;
        }
        let mut attrs = Window::default_attributes()
            .with_title("Libra Inspector")
            .with_inner_size(LogicalSize::new(320.0, 520.0));
        if let Some(main) = &self.gfx {
            if let Ok(pos) = main.window.outer_position() {
                let size = main.window.outer_size();
                attrs = attrs.with_position(PhysicalPosition::new(
                    pos.x + size.width as i32 + 12,
                    pos.y,
                ));
            }
        }
        let window = Arc::new(event_loop.create_window(attrs).expect("create inspector"));
        let context = softbuffer::Context::new(window.clone()).expect("inspector context");
        let surface =
            softbuffer::Surface::new(&context, window.clone()).expect("inspector surface");
        window.request_redraw();
        self.inspector = Some(InspectorGfx {
            window,
            _context: context,
            surface,
            renderer: Renderer::new(),
        });
    }

    fn present_inspector(&mut self) {
        if self.inspector.is_none() {
            return;
        }
        let fields = self.inspector_fields();
        let actions = self.action_log.lines();
        let theme = self.theme.clone();
        let store = Arc::clone(self.effects.store());
        let Some(gfx) = &mut self.inspector else { return };
        let size = gfx.window.inner_size();
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else {
            return;
        };
        let scale = gfx.window.scale_factor() as f32;
        if scale <= 0.0 {
            return;
        }
        let view = inspector::view(&fields, &actions);
        let logical = (size.width as f32 / scale, size.height as f32 / scale);
        let Ok(laid) = libra_components::layout(&view, logical, &gfx.renderer, &theme) else {
            eprintln!("[libra] inspector layout failed");
            return;
        };
        let mut cmds = laid.paint(&gfx.renderer, store.as_ref(), None, &theme);
        scale_cmds_in_place(&mut cmds, scale);
        let pixmap = gfx.renderer.render(&cmds, size.width, size.height);
        gfx.surface.resize(w, h).expect("inspector resize");
        let mut buffer = gfx.surface.buffer_mut().expect("inspector buffer");
        blit_rows(
            &mut buffer,
            &pixmap,
            0,
            w.get() as usize,
            0,
            h.get() as usize,
        );
        gfx.window.pre_present_notify();
        buffer.present().expect("inspector present");
    }

    fn present(&mut self) {
        if self.window_hidden {
            return;
        }
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
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else {
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
            let mut cmds =
                laid.paint_full(renderer, store, focus_paint.as_ref(), hover, &theme, scale);
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
                blit_rows(
                    &mut buffer,
                    &pixmap,
                    0,
                    w.get() as usize,
                    0,
                    h.get() as usize,
                );
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
        let Some(cache) = self.frame_cache.as_ref() else {
            return;
        };
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
                blit_rows(
                    &mut buffer,
                    pixmap,
                    x0 as usize,
                    x1 as usize,
                    y0 as usize,
                    y1 as usize,
                );
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

    /// PLAN-RELEASE. Only a packaged `.app` with an update endpoint in
    /// its identity resource checks. Decline leaves the app running.
    fn start_update_check(&mut self) {
        if self.update_started || !update::check_enabled() {
            return;
        }
        self.update_started = true;
        #[cfg(target_os = "macos")]
        {
            let Some(path) = update::bundled_identity_path() else {
                return;
            };
            let cfg = match update::load_config(&path) {
                Ok(Some(cfg)) => cfg,
                Ok(None) => return,
                Err(e) => {
                    eprintln!("[libra] update check: {e}");
                    return;
                }
            };
            let proxy = self.update_proxy.clone();
            let target = update::host_target().to_string();
            std::thread::spawn(move || match update::fetch_text(&cfg.endpoint) {
                Ok(body) => match update::offer_from_feed(&cfg, &target, &body) {
                    Ok(Some(offer)) => {
                        let _ = proxy.send_event(menu::ShellEvent::UpdateOffer(offer));
                    }
                    Ok(None) => {}
                    Err(e) => eprintln!("[libra] update check: {e}"),
                },
                Err(e) => eprintln!("[libra] update check: {e}"),
            });
        }
    }

    fn confirm_update(&mut self, offer: update::UpdateOffer) {
        let notes: String = offer.notes.chars().take(400).collect();
        let description = if notes.is_empty() {
            format!(
                "{} {} is available (you have {}).",
                offer.product_name, offer.version, offer.current
            )
        } else {
            format!(
                "{} {} is available (you have {}).\n\n{notes}",
                offer.product_name, offer.version, offer.current
            )
        };
        let answer = rfd::MessageDialog::new()
            .set_title("Update available")
            .set_description(&description)
            .set_buttons(rfd::MessageButtons::YesNo)
            .show();
        if answer != rfd::MessageDialogResult::Yes {
            return;
        }
        let pubkey = match update::bundled_identity_path().and_then(|path| {
            update::load_config(&path)
                .ok()
                .flatten()
                .map(|cfg| cfg.pubkey)
        }) {
            Some(pubkey) => pubkey,
            None => {
                eprintln!("[libra] update: identity resource disappeared");
                return;
            }
        };
        let proxy = self.update_proxy.clone();
        std::thread::spawn(move || match update::stage(&pubkey, &offer) {
            Ok(staged) => {
                let _ = proxy.send_event(menu::ShellEvent::ApplyUpdate(staged));
            }
            Err(e) => eprintln!("[libra] update: {e}"),
        });
    }

    fn apply_update(&mut self, event_loop: &ActiveEventLoop, staged: PathBuf) {
        let exe = match std::env::current_exe() {
            Ok(path) => path,
            Err(e) => {
                eprintln!("[libra] update: {e}");
                return;
            }
        };
        let Some(dest) = update::app_bundle(&exe) else {
            eprintln!("[libra] update: the running binary is not inside a .app");
            return;
        };
        let script = update::relaunch_script(std::process::id(), &dest, &staged);
        let path = std::env::temp_dir().join(format!("libra-update-{}.sh", std::process::id()));
        if let Err(e) = std::fs::write(&path, &script) {
            eprintln!("[libra] update: {e}");
            return;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755));
        }
        let mut cmd = std::process::Command::new("/bin/sh");
        cmd.arg(&path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            unsafe {
                cmd.pre_exec(|| {
                    let _ = libc::setsid();
                    Ok(())
                });
            }
        }
        if let Err(e) = cmd.spawn() {
            eprintln!("[libra] update: {e}");
            return;
        }
        eprintln!(
            "[libra] update: replacing {} after this process exits",
            dest.display()
        );
        self.persist_and_exit(event_loop);
    }

    fn persist_and_exit(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(save) = &self.save_hook {
            save(&self.app);
        }
        event_loop.exit();
    }
}

impl<A: App + 'static> ApplicationHandler<menu::ShellEvent<A::Msg>> for Shell<A>
where
    A::Msg: Send + 'static,
{
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return;
        }
        let mut attrs = Window::default_attributes()
            .with_title(self.opts.title.clone())
            // Created hidden: the AccessKit adapter must
            // attach before first show.
            .with_visible(false)
            .with_inner_size(LogicalSize::new(
                self.opts.logical_size.0 as f64,
                self.opts.logical_size.1 as f64,
            ));
        if let Some(path) = self.opts.icon.as_deref() {
            if let Some(icon) = icon::load_png(path) {
                attrs = attrs.with_window_icon(Some(icon));
            } else {
                eprintln!(
                    "[libra] window icon not loaded from {} (expected a square PNG)",
                    path.display()
                );
            }
        }
        #[cfg(target_os = "macos")]
        if self.opts.frameless {
            use winit::platform::macos::WindowAttributesExtMacOS;
            attrs = attrs
                .with_titlebar_transparent(true)
                .with_fullsize_content_view(true)
                .with_title_hidden(true);
        }
        #[cfg(not(target_os = "macos"))]
        if self.opts.frameless {
            attrs = attrs.with_decorations(false);
        }
        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        let context = softbuffer::Context::new(window.clone()).expect("softbuffer context");
        let surface =
            softbuffer::Surface::new(&context, window.clone()).expect("softbuffer surface");
        let a11y = accesskit_winit::Adapter::with_direct_handlers(
            event_loop,
            &window,
            Activation {
                snapshot: Arc::clone(&self.a11y_snapshot),
            },
            Actions::<A> {
                snapshot: Arc::clone(&self.a11y_snapshot),
                sink: Arc::clone(&self.a11y_sink),
            },
            Deactivation,
        );
        window.set_visible(true);
        window.request_redraw();
        self.gfx = Some(Gfx {
            window,
            _context: context,
            surface,
            a11y,
        });
        self.refresh_frame_period();
        // relayout collects mounted `<Frames>` then sync_subscriptions
        // arms the display loop — do not sync on empty handlers first.
        self.relayout();
        self.sync_native_menu();
        self.sync_tray();
        if !self.boot_effects.is_empty() {
            let boot = std::mem::take(&mut self.boot_effects);
            self.handle_effects(boot);
        }
        self.start_update_check();
        self.open_inspector(event_loop);
    }

    /// Effect results, a11y actions, and `<Interval>` ticks arrive here
    /// (via the EventLoopProxy) and dispatch on the main thread — same
    /// causal path as §4.2's loop. Frame ticks run from `about_to_wait`
    /// while the display loop is active.
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: menu::ShellEvent<A::Msg>) {
        match event {
            menu::ShellEvent::App(msg) => self.dispatch(msg),
            menu::ShellEvent::Menu(id) => self.dispatch_menu(id),
            menu::ShellEvent::PersistAndExit => {
                PERSIST_REQUESTED.store(false, Ordering::SeqCst);
                self.persist_and_exit(event_loop);
            }
            menu::ShellEvent::Streams => self.pump_streams(),
            menu::ShellEvent::UpdateOffer(offer) => self.confirm_update(offer),
            menu::ShellEvent::ApplyUpdate(staged) => self.apply_update(event_loop, staged),
            menu::ShellEvent::NotifyActivate(msg) => self.dispatch(msg),
            menu::ShellEvent::RaiseWindow => self.apply_window(WindowOp::Raise),
            menu::ShellEvent::TrayClick(action) => {
                if let Some(msg) = A::Msg::action_by_name(&action) {
                    self.dispatch(msg);
                }
            }
            menu::ShellEvent::Automate { line, reply } => {
                let response = automate::handle_line(self, &line);
                let _ = reply.send(response);
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if self.inspector.as_ref().is_some_and(|gfx| gfx.window.id() == id) {
            match event {
                WindowEvent::CloseRequested => self.inspector = None,
                WindowEvent::RedrawRequested | WindowEvent::Resized(_) => self.present_inspector(),
                _ => {}
            }
            return;
        }
        if let Some(gfx) = &mut self.gfx {
            let window = Arc::clone(&gfx.window);
            gfx.a11y.process_event(&window, &event);
        }
        match event {
            WindowEvent::CloseRequested => {
                if self.opts.window_policy == WindowPolicy::CloseToTray {
                    self.apply_window(WindowOp::Hide);
                } else {
                    self.persist_and_exit(event_loop);
                }
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
            WindowEvent::Ime(ime) => self.ime(ime),
            WindowEvent::Touch(touch) => self.on_touch(touch),
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
        self.poll_long_press();
        if persist_requested() {
            self.persist_and_exit(event_loop);
            return;
        }
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

        let mut wake = if self.opts.watch.is_some() || self.opts.theme_file.is_some() {
            Some(self.next_watch_check)
        } else {
            None
        };
        if let Some(g) = &self.touch {
            if !g.fired && !g.dragged {
                let at = g.started + LONG_PRESS;
                wake = Some(wake.map_or(at, |w| w.min(at)));
            }
        }
        match wake {
            Some(at) => event_loop.set_control_flow(ControlFlow::WaitUntil(at)),
            None => event_loop.set_control_flow(ControlFlow::Wait),
        }
    }
}

/// Serves the current tree when AT first connects (§17).
struct Activation {
    snapshot: Arc<std::sync::Mutex<a11y::A11ySnapshot>>,
}

impl accesskit::ActivationHandler for Activation {
    fn request_initial_tree(&mut self) -> Option<accesskit::TreeUpdate> {
        Some(a11y::tree_update(
            &self.snapshot.lock().expect("a11y snapshot"),
        ))
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
        color: Color {
            r: 0x8B,
            g: 0x1A,
            b: 0x1A,
            a: 255,
        },
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
            DrawCmd::StrokeRRect {
                rect,
                radius,
                width,
                ..
            } => {
                *rect = scale_rect(rect, s);
                *radius *= s;
                *width *= s;
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
            DrawCmd::Strips {
                x, col_w, strips, ..
            } => {
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
            DrawCmd::RRect {
                rect: Rect::new(1.0, 2.0, 3.0, 4.0),
                radius: 6.0,
                color: theme.accent,
            },
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
            DrawCmd::Text {
                x, y, px, color, ..
            } => {
                assert_eq!((*x, *y, *px), (20.0, 40.0, 28.0));
                assert_eq!(*color, Theme::dark().text);
            }
            other => panic!("unexpected cmd: {other:?}"),
        }
    }

    #[test]
    fn overlay_draws_bar_then_text_over_the_frame() {
        let mut cmds = vec![DrawCmd::Clear(Theme::dark().bg)];
        overlay_cmds(
            &mut cmds,
            "views/x.lx: 3:7: no field 'cuont'",
            400.0,
            &Theme::dark(),
        );
        assert_eq!(cmds.len(), 3);
        assert!(matches!(&cmds[1], DrawCmd::Rect { rect, .. } if rect.w == 400.0));
        assert!(matches!(&cmds[2], DrawCmd::Text { content, .. } if content.contains("cuont")));
    }

    #[test]
    fn watch_signature_sees_a_sibling_lx() {
        let dir = std::env::temp_dir().join(format!("libra-watch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("components")).unwrap();
        let root = dir.join("app.lx");
        std::fs::write(&root, "<Column/>").unwrap();
        std::fs::write(dir.join("components/card.lx"), "<props/>").unwrap();
        assert!(watch_signature(&root).is_some());
        assert!(max_lx_mtime(&dir).is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
