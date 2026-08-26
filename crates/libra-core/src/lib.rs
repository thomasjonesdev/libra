//! libra-core — the application contract: `State` / `Action` / `reduce` /
//! `Effect`, the `App` trait (`init` + `update`), the `ReflectState` /
//! `ReflectAction` reflection supertraits, and `Remote<T, E>`.
//!
//! Architecture: ARCHITECTURE.md §5 (State & Actions), §5.3 (purity rules
//! for `update`), §12.1 (the Effect vocabulary).

pub use libra_reflect::LibraReflect;

/// An action constructor: how an effect result becomes an action. Enum
/// tuple variants coerce to this directly (`Action::HttpDone` is a
/// `fn(HttpResult) -> Action`). Note: fn-pointer *equality* is
/// unreliable across crates (codegen duplicates the shim), so
/// `Effect`'s `PartialEq` deliberately compares the effect's data and
/// ignores ctor identity — effects are data; the ctor is routing.
pub type ActionCtor<T, M> = fn(T) -> M;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimerId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProcessId(pub u64);

/// Identifies a data-plane series (§14.2). State never holds
/// samples — it holds handles; samples live in ring buffers read at
/// paint time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct SeriesId(pub u32);

/// The State-side view of a series (§14.2): id + view window + pause
/// flag. Control decisions are pure actions that mutate this struct;
/// samples never enter `update`.
///
/// Phase 2 reflection shortcut: markup binds a handle through its
/// `Display` form (`id|window_ms|paused`) since bindings are strings —
/// typed bindings arrive with the grammar's growth.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SeriesHandle {
    pub id: SeriesId,
    /// How much history the component shows, in ms of sample time.
    pub window_ms: f64,
    pub paused: bool,
}

impl SeriesHandle {
    pub fn new(id: SeriesId, window_ms: f64) -> Self {
        SeriesHandle { id, window_ms, paused: false }
    }
}

impl std::fmt::Display for SeriesHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}|{}|{}", self.id.0, self.window_ms, self.paused)
    }
}

impl std::str::FromStr for SeriesHandle {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        let mut it = s.split('|');
        let (Some(id), Some(win), Some(paused)) = (it.next(), it.next(), it.next()) else {
            return Err(format!("malformed series handle: {s:?}"));
        };
        Ok(SeriesHandle {
            id: SeriesId(id.parse().map_err(|e| format!("series id: {e}"))?),
            window_ms: win.parse().map_err(|e| format!("window: {e}"))?,
            paused: paused.parse().map_err(|e| format!("paused: {e}"))?,
        })
    }
}

/// Where ingested samples come from (§14.2). Closed, like Cmd.
#[derive(Debug, Clone, PartialEq)]
pub enum SourceSpec {
    /// Replay a recorded `.ltape` file at `speed`× real time
    /// (`f64::INFINITY` = as fast as possible).
    Tape { path: std::path::PathBuf, speed: f64 },
    /// Deterministic synthetic sine + noise (benchmarks, demos):
    /// `rate_hz` samples/sec of `sin(2π·freq_hz·t) · amp` + seeded noise.
    Sine { rate_hz: f64, freq_hz: f64, amp: f64, seed: u64 },
}

/// What one vsync-coalesced frame tick tells `update` (§14.2): all the
/// time the app is allowed to know. Tests construct these directly —
/// time is injected, never read.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameInfo {
    /// Session-relative time, ms.
    pub t_ms: f64,
    pub index: u64,
}

impl FrameInfo {
    pub fn at(t_ms: f64) -> Self {
        FrameInfo { t_ms, index: (t_ms / 16.0) as u64 }
    }
}

/// What one `<Interval every={ms} onTick={name}>` tick tells `update`
/// (§12.5): just the session-relative time the timer fired — no frame
/// index, since intervals aren't vsync-coalesced (each identity ticks
/// on its own cadence). Tests construct these directly — time is
/// injected, never read.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Instant {
    pub t_ms: f64,
}

impl Instant {
    pub fn at(t_ms: f64) -> Self {
        Instant { t_ms }
    }
}

/// Ongoing event sources, declared by `App::subscriptions` and diffed
/// by the runtime (§12.2). Phase 2 subset: frame ticks. Streams join
/// with WebSocket/process effects.
#[derive(Clone)]
pub enum Sub<M> {
    /// One action per vsync-coalesced tick while subscribed.
    Frames { on_frame: ActionCtor<FrameInfo, M> },
}

impl<M> Sub<M> {
    pub fn frames(on_frame: ActionCtor<FrameInfo, M>) -> Self {
        Sub::Frames { on_frame }
    }
}

impl<M> std::fmt::Debug for Sub<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Sub::Frames { .. } => write!(f, "Sub::Frames"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: &'static str,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
}

impl HttpRequest {
    pub fn get(url: impl Into<String>) -> Self {
        HttpRequest { method: "GET", url: url.into(), headers: vec![], body: None }
    }
    pub fn post(url: impl Into<String>, body: Vec<u8>) -> Self {
        HttpRequest { method: "POST", url: url.into(), headers: vec![], body: Some(body) }
    }
    pub fn header(mut self, k: impl Into<String>, v: impl Into<String>) -> Self {
        self.headers.push((k.into(), v.into()));
        self
    }
}

/// What comes back from `Effect::Http`. Any HTTP status is `Response`
/// (a 500 is an answer, not a transport failure); `Failed` is
/// connect/DNS/timeout territory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpResult {
    Response { status: u16, body: Vec<u8> },
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KvResult {
    Ok,
    Err(String),
}

/// A program to run — data, not a `std::process::Command` (§12.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub cwd: Option<std::path::PathBuf>,
    pub timeout_ms: Option<u64>,
}

impl CommandSpec {
    pub fn new(program: impl Into<String>) -> Self {
        CommandSpec {
            program: program.into(),
            args: vec![],
            env: vec![],
            cwd: None,
            timeout_ms: None,
        }
    }

    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    pub fn cwd(mut self, path: impl Into<std::path::PathBuf>) -> Self {
        self.cwd = Some(path.into());
        self
    }

    pub fn timeout_ms(mut self, ms: u64) -> Self {
        self.timeout_ms = Some(ms);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessStatus {
    Exited(i32),
    Signalled(i32),
    TimedOut,
    SpawnFailed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessResult {
    pub status: ProcessStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub finished_at_unix_ms: u64,
}

/// Effect descriptions (ARCHITECTURE.md §12.1, Phase-1 subset).
/// Actions return these; the runtime executes them; tests intercept
/// them and hand-feed results back. Closed and versioned — grows in
/// later phases (Ws*, File*, Spawn, Interval, ingest…).
#[derive(Debug, Clone)]
pub enum Effect<M> {
    // Network
    Http { req: HttpRequest, on_done: ActionCtor<HttpResult, M> },

    // Persistence (§13, executed by libra-kv via the effects runtime)
    KvGet { key: String, on_done: ActionCtor<Option<Vec<u8>>, M> },
    KvSet { key: String, value: Vec<u8>, on_done: Option<ActionCtor<KvResult, M>> },
    KvDelete { key: String },
    KvScan { prefix: String, on_done: ActionCtor<Vec<(String, Vec<u8>)>, M> },

    // Time & entropy (keeping actions pure)
    Timer { id: Option<TimerId>, after_ms: u64, action: M },
    CancelTimer { id: TimerId },
    Random { bytes: usize, on_done: ActionCtor<Vec<u8>, M> },

    // Data plane (§14): ingest threads owned by the runtime; samples
    // land in ring buffers, never in actions.
    IngestOpen { source: SourceSpec, series: SeriesId },
    IngestClose { series: SeriesId },

    // Platform (§12.1): handled by the shell (and intercepted by the
    // harness), not the worker pool — the renderer re-resolves live.
    SetTheme { name: String },
    /// Put text on the OS clipboard (PLAN-NATIVE-INTEGRATIONS): shell-
    /// handled (arboard), acked with a [`ClipboardResult`] when a ctor
    /// is given — "copy note" flows can report a typed outcome.
    ClipboardSet { text: String, on_done: Option<ActionCtor<ClipboardResult, M>> },
    /// Native folder sheet (PLAN-NATIVE-INTEGRATIONS / §19 `rfd`).
    /// Shell-handled on the UI thread; harness intercepts.
    PickFolder { on_done: ActionCtor<FolderPick, M> },

    // Processes (§12.6): worker-pool subprocess, run-to-completion.
    Process { id: Option<ProcessId>, command: CommandSpec, on_done: ActionCtor<ProcessResult, M> },
    CancelProcess { id: ProcessId },
}

/// Outcome of [`Effect::ClipboardSet`] (PLAN-NATIVE-INTEGRATIONS).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardResult {
    Ok,
    Err(String),
}

/// Outcome of [`Effect::PickFolder`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FolderPick {
    Picked(String),
    Cancelled,
}

/// Equality over the effect's *data*. Ctor fields compare by presence
/// only (`Some` vs `None`), never by fn-pointer identity — that
/// identity is not stable across crate boundaries, and the description
/// of an effect doesn't include where its result is routed.
impl<M: PartialEq> PartialEq for Effect<M> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Effect::Http { req: a, .. }, Effect::Http { req: b, .. }) => a == b,
            (Effect::KvGet { key: a, .. }, Effect::KvGet { key: b, .. }) => a == b,
            (
                Effect::KvSet { key: ka, value: va, on_done: da },
                Effect::KvSet { key: kb, value: vb, on_done: db },
            ) => ka == kb && va == vb && da.is_some() == db.is_some(),
            (Effect::KvDelete { key: a }, Effect::KvDelete { key: b }) => a == b,
            (Effect::KvScan { prefix: a, .. }, Effect::KvScan { prefix: b, .. }) => a == b,
            (
                Effect::Timer { id: ia, after_ms: aa, action: ma },
                Effect::Timer { id: ib, after_ms: ab, action: mb },
            ) => ia == ib && aa == ab && ma == mb,
            (Effect::CancelTimer { id: a }, Effect::CancelTimer { id: b }) => a == b,
            (Effect::Random { bytes: a, .. }, Effect::Random { bytes: b, .. }) => a == b,
            (
                Effect::IngestOpen { source: sa, series: ia },
                Effect::IngestOpen { source: sb, series: ib },
            ) => sa == sb && ia == ib,
            (Effect::IngestClose { series: a }, Effect::IngestClose { series: b }) => a == b,
            (Effect::SetTheme { name: a }, Effect::SetTheme { name: b }) => a == b,
            (
                Effect::ClipboardSet { text: ta, on_done: da },
                Effect::ClipboardSet { text: tb, on_done: db },
            ) => ta == tb && da.is_some() == db.is_some(),
            (Effect::PickFolder { .. }, Effect::PickFolder { .. }) => true,
            (
                Effect::Process { id: ia, command: ca, .. },
                Effect::Process { id: ib, command: cb, .. },
            ) => ia == ib && ca == cb,
            (Effect::CancelProcess { id: a }, Effect::CancelProcess { id: b }) => a == b,
            _ => false,
        }
    }
}

impl<M> Effect<M> {
    /// Convenience constructors mirroring §5.1's usage style.
    pub fn http(req: HttpRequest, on_done: ActionCtor<HttpResult, M>) -> Self {
        Effect::Http { req, on_done }
    }
    pub fn kv_get(key: impl Into<String>, on_done: ActionCtor<Option<Vec<u8>>, M>) -> Self {
        Effect::KvGet { key: key.into(), on_done }
    }
    pub fn kv_set(key: impl Into<String>, value: Vec<u8>) -> Self {
        Effect::KvSet { key: key.into(), value, on_done: None }
    }
    pub fn kv_set_acked(
        key: impl Into<String>,
        value: Vec<u8>,
        on_done: ActionCtor<KvResult, M>,
    ) -> Self {
        Effect::KvSet { key: key.into(), value, on_done: Some(on_done) }
    }
    pub fn timer(after_ms: u64, action: M) -> Self {
        Effect::Timer { id: None, after_ms, action }
    }
    pub fn timer_id(id: TimerId, after_ms: u64, action: M) -> Self {
        Effect::Timer { id: Some(id), after_ms, action }
    }
    pub fn clipboard_set(text: impl Into<String>) -> Self {
        Effect::ClipboardSet { text: text.into(), on_done: None }
    }
    pub fn clipboard_set_acked(
        text: impl Into<String>,
        on_done: ActionCtor<ClipboardResult, M>,
    ) -> Self {
        Effect::ClipboardSet { text: text.into(), on_done: Some(on_done) }
    }
    pub fn pick_folder(on_done: ActionCtor<FolderPick, M>) -> Self {
        Effect::PickFolder { on_done }
    }
    pub fn process(command: CommandSpec, on_done: ActionCtor<ProcessResult, M>) -> Self {
        Effect::Process { id: None, command, on_done }
    }
    pub fn process_id(
        id: ProcessId,
        command: CommandSpec,
        on_done: ActionCtor<ProcessResult, M>,
    ) -> Self {
        Effect::Process { id: Some(id), command, on_done }
    }
}

/// What an `#[libra::actions]` method may return (ARCHITECTURE.md
/// §5.1): `()`, a single `Effect`, an optional one, or several. The
/// generated `reduce` calls `.into_effects()` uniformly — a plain
/// local trait rather than `std::convert::Into`, because `Vec<Effect<M>>`
/// is a foreign type and orphan rules block `impl From<()> for
/// Vec<Effect<M>>` directly.
pub trait IntoEffects<M> {
    fn into_effects(self) -> Vec<Effect<M>>;
}

impl<M> IntoEffects<M> for () {
    fn into_effects(self) -> Vec<Effect<M>> {
        vec![]
    }
}

impl<M> IntoEffects<M> for Effect<M> {
    fn into_effects(self) -> Vec<Effect<M>> {
        vec![self]
    }
}

impl<M> IntoEffects<M> for Option<Effect<M>> {
    fn into_effects(self) -> Vec<Effect<M>> {
        self.into_iter().collect()
    }
}

impl<M> IntoEffects<M> for Vec<Effect<M>> {
    fn into_effects(self) -> Vec<Effect<M>> {
        self
    }
}

/// Async-state type (Appendix C, via Elm's RemoteData): makes
/// loading/error impossible-states unrepresentable on State.
/// Serde-derived because Models are serializable (§5.1) — snapshots,
/// rehydration, session export.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Remote<T, E> {
    NotAsked,
    Loading,
    Success(T),
    Failure(E),
}

impl<T, E> Remote<T, E> {
    pub fn is_loading(&self) -> bool {
        matches!(self, Remote::Loading)
    }
    pub fn success(&self) -> Option<&T> {
        match self {
            Remote::Success(t) => Some(t),
            _ => None,
        }
    }
}

impl<T, E> Default for Remote<T, E> {
    fn default() -> Self {
        Remote::NotAsked
    }
}

/// One element of a `<for each={…}>` collection (§6.2 / PLAN-ITEM-FIELDS).
///
/// Opaque element types (`String`, `SeriesHandle`, …) carry only
/// [`whole`](CollectionRow::whole) — the item's `Display` text — with
/// an empty [`fields`](CollectionRow::fields) map. Structured element
/// types (`T: ReflectState + Display`, typically `#[derive(LibraReflect)]`)
/// also expose named Display fields for `{loopVar.field}` /
/// `key={loopVar.field}` / `onPress={action(loopVar.field)}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionRow {
    /// Whole-item Display — used by `key={t}`, `onPress={f(t)}`, and
    /// Chart/`series={s}` round-trips.
    pub whole: String,
    /// Named fields on a structured item; empty for opaque elements.
    pub fields: Vec<(String, String)>,
}

impl CollectionRow {
    /// Opaque row: Display only, no field map.
    pub fn opaque(whole: impl Into<String>) -> Self {
        CollectionRow { whole: whole.into(), fields: Vec::new() }
    }

    /// Resolve a loop-scoped binding against this row.
    ///
    /// - `loop_var` → `whole`
    /// - `loop_var.field` → that field's Display (one segment only)
    /// - anything else → `None` (caller may fall through to outer state)
    pub fn get(&self, path: &str, loop_var: &str) -> Option<String> {
        if path == loop_var {
            return Some(self.whole.clone());
        }
        let prefix = format!("{loop_var}.");
        let Some(field) = path.strip_prefix(&prefix) else {
            return None;
        };
        if field.is_empty() || field.contains('.') {
            return None;
        }
        self.fields
            .iter()
            .find(|(k, _)| k == field)
            .map(|(_, v)| v.clone())
    }
}

/// Build a [`CollectionRow`] from a structured item (`ReflectState` +
/// `Display`). Used by the state derive for `Vec<T>` element types that
/// aren't on the opaque allowlist (`String`, `SeriesHandle`).
pub fn collection_row_from_state<T: ReflectState + std::fmt::Display>(item: &T) -> CollectionRow {
    CollectionRow {
        whole: item.to_string(),
        fields: T::field_names()
            .iter()
            .filter_map(|n| item.field(n).map(|v| ((*n).to_string(), v)))
            .collect(),
    }
}

/// Opaque row from a Display-only element (`String`, `SeriesHandle`, …).
pub fn collection_row_opaque(item: &impl std::fmt::Display) -> CollectionRow {
    CollectionRow::opaque(item.to_string())
}

/// State reflection: how markup bindings resolve against the state.
/// Implemented by `#[derive(LibraReflect)]` on the State struct
/// (Display fields become bindable; `#[libra(skip)]` excludes one), or
/// by hand for custom projections. Drives the interpreter's load-time
/// validation and, later, the automation-server schema (§16).
pub trait ReflectState {
    /// Resolve a binding path to display text.
    ///
    /// Paths include whole-value fields (`count`), nested ReflectState
    /// fields (`user.name` — PLAN-ACCESSORS), and the accessor whitelist
    /// on strings/collections (`items.len`, `name.isEmpty`, with optional
    /// trailing `()`). `None` makes the binding a load-time interp error.
    fn field(&self, path: &str) -> Option<String>;

    /// Every bindable field name (diagnostics, automation schema).
    fn field_names() -> &'static [&'static str];

    /// Resolve an iterable binding path (`<for each={path}>`, §6.2) to
    /// each element's display text, in order. `None` means the path
    /// doesn't name a collection field — same load-time-error contract
    /// as `field`. Default `None` keeps every existing `ReflectState`
    /// implementor (hand-written or `#[derive(LibraReflect)]`-only)
    /// source-compatible; `#[libra::state]`/the derive override this
    /// for `Vec<T>`-typed fields.
    fn collection(&self, path: &str) -> Option<Vec<String>> {
        let _ = path;
        None
    }

    /// Resolve an iterable binding path to structured rows (§6.2 item
    /// fields). Default wraps [`collection`](ReflectState::collection)
    /// as opaque rows so hand-written implementors stay source-compatible;
    /// the derive overrides this with [`collection_row_of`] per element.
    fn collection_rows(&self, path: &str) -> Option<Vec<CollectionRow>> {
        self.collection(path)
            .map(|items| items.into_iter().map(CollectionRow::opaque).collect())
    }
}

/// Action reflection: how markup handler names resolve to actions.
/// Implemented by `#[derive(LibraReflect)]` on the Action enum: unit
/// variants are constructible by name; payload variants are effect
/// results, not UI handlers, and resolve to `None`. Public names are
/// camelCase per §3.3; the derive maps them from the Rust identifier.
pub trait ReflectAction: Sized {
    fn action_by_name(name: &str) -> Option<Self>;

    /// Resolve a handler name to an action carrying a control-provided
    /// String payload (§6.2's `onChange={filterChanged}` — the control
    /// supplies the value). Derived for single-`String` tuple variants;
    /// anything else resolves to `None`.
    fn action_with_str(name: &str, value: String) -> Option<Self> {
        let _ = (name, value);
        None
    }

    /// Resolve a handler name to an action carrying a `FrameInfo`
    /// (§12.5's `<Frames onTick={frame}>` source element). Derived for
    /// single-`FrameInfo` tuple variants; anything else resolves to
    /// `None`. Mirrors `action_with_str`'s shape for a different
    /// runtime-supplied payload type.
    fn action_with_frame_info(name: &str, info: FrameInfo) -> Option<Self> {
        let _ = (name, info);
        None
    }

    /// Resolve a handler name to an action carrying an `Instant`
    /// (§12.5's `<Interval every={ms} onTick={name}>` source element).
    /// Derived for single-`Instant` tuple variants; anything else
    /// resolves to `None`. Mirrors `action_with_frame_info`'s shape for
    /// a different runtime-supplied payload type.
    fn action_with_instant(name: &str, instant: Instant) -> Option<Self> {
        let _ = (name, instant);
        None
    }

    /// Every variant name (diagnostics, automation schema).
    fn action_names() -> &'static [&'static str];
}

/// Form-state helper (Appendix C, via React Hook Form): per-field
/// dirty/touched/error with a pure validator — plain state machinery,
/// unit-testable, no framework runtime involved. Fields live on
/// State; `set` runs in an action.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Field<T> {
    value: T,
    initial: T,
    touched: bool,
    error: Option<String>,
    #[serde(skip)]
    validator: Option<fn(&T) -> Option<String>>,
}

impl<T: Clone + PartialEq> Field<T> {
    pub fn new(initial: T) -> Self {
        Field { value: initial.clone(), initial, touched: false, error: None, validator: None }
    }

    /// A pure validator: `None` = valid, `Some(msg)` = the error to
    /// display (ordinary model state, per §10.1's validation posture).
    pub fn with_validator(mut self, v: fn(&T) -> Option<String>) -> Self {
        self.error = v(&self.value);
        self.validator = Some(v);
        self
    }

    /// The `update`-side setter: touches, stores, revalidates.
    pub fn set(&mut self, value: T) {
        self.value = value;
        self.touched = true;
        self.error = self.validator.and_then(|v| v(&self.value));
    }

    pub fn value(&self) -> &T {
        &self.value
    }
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
    pub fn is_valid(&self) -> bool {
        self.error.is_none()
    }
    /// Changed from its initial value.
    pub fn is_dirty(&self) -> bool {
        self.value != self.initial
    }
    /// Ever set by the user (errors usually display only when touched).
    pub fn is_touched(&self) -> bool {
        self.touched
    }
    /// Submit-gating: show this field's error only after interaction.
    pub fn visible_error(&self) -> Option<&str> {
        if self.touched {
            self.error()
        } else {
            None
        }
    }
    /// Mark clean at the current value (after a successful submit).
    pub fn commit(&mut self) {
        self.initial = self.value.clone();
        self.touched = false;
    }
}

impl<T: std::fmt::Display> std::fmt::Display for Field<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.value.fmt(f)
    }
}

/// Screen-stack helper (Appendix C, via React Router): a navigation
/// stack over an app-defined `Screen` enum, driven by push/pop/replace
/// actions. Plain state machinery — the root screen is never popped, so
/// `current()` is total.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NavStack<S> {
    stack: Vec<S>,
}

impl<S> NavStack<S> {
    pub fn new(root: S) -> Self {
        NavStack { stack: vec![root] }
    }
    pub fn current(&self) -> &S {
        self.stack.last().expect("nav stack keeps its root")
    }
    pub fn push(&mut self, screen: S) {
        self.stack.push(screen);
    }
    /// Pops unless at the root. Returns whether a pop happened.
    pub fn pop(&mut self) -> bool {
        if self.stack.len() > 1 {
            self.stack.pop();
            true
        } else {
            false
        }
    }
    pub fn replace(&mut self, screen: S) {
        *self.stack.last_mut().expect("nav stack keeps its root") = screen;
    }
    pub fn depth(&self) -> usize {
        self.stack.len()
    }
    pub fn can_pop(&self) -> bool {
        self.stack.len() > 1
    }
}

/// The application contract — `init` + `update`, nothing else (§5.1).
/// This is runtime plumbing: `libra::app!` (§5.8) and the
/// `#[libra::actions]` macro (§5.2) generate an impl of this trait from
/// state + actions, so apps using the ergonomic layer never write one
/// by hand — none of the four ported examples reference `App::Msg`
/// directly. `Msg`/`update` keep their names here permanently rather
/// than pending, as originally noted: renaming the associated type
/// would be a breaking change with no deprecated-alias escape hatch
/// (Rust can't dual-accept `type Msg` and `type Action` for one
/// required associated type), and it would force every hand-rolled
/// `impl App` — including examples/gallery, deliberately not on the
/// actions-style port list (MIGRATION-NAMING.md Stage 6) — to update
/// immediately for a name the ergonomic surface never exposes. The
/// user-facing vocabulary is state/action/reduce (§3.1); this trait is
/// the generated-code layer underneath it, not part of that surface.
///
/// Purity rules for `update` (ARCHITECTURE.md §5.3): no I/O, no clocks,
/// no randomness, no threads, no blocking. Time and entropy arrive
/// *inside* actions. These rules are what make unit tests total.
pub trait App: Sized + ReflectState {
    type Msg: Clone + std::fmt::Debug + ReflectAction;

    /// Initial state + startup effects.
    fn init() -> (Self, Vec<Effect<Self::Msg>>);

    /// The only place state changes. Pure.
    fn update(&mut self, msg: Self::Msg) -> Vec<Effect<Self::Msg>>;

    /// Ambient event sources, derived from state (§12.2). The runtime
    /// diffs the result after every update: new subs start, removed
    /// subs stop. Pure — so "tick while this screen is live" is a
    /// testable function.
    fn subscriptions(&self) -> Vec<Sub<Self::Msg>> {
        vec![]
    }

    /// App-level keyboard shortcuts (§19.2): a static chord → action
    /// table, validated at startup (`build_shortcut_table`). Static —
    /// shortcuts are fixed per app; state-dependent enablement is the
    /// action's job, not the table's.
    fn shortcuts() -> Vec<Shortcut> {
        vec![]
    }
}

// ---------------------------------------------------------------------------
// Keyboard shortcuts (§19.2) — declarations, not handlers.
// ---------------------------------------------------------------------------

/// One declared shortcut: a chord string (`"cmd+n"`, §19.2 grammar), the
/// camelCase action it dispatches, and an optional **static** string
/// payload (`"cmd+1"` → `jumpFolder("1")`). No computed payloads — an
/// action needing context reads it from State, same rule as markup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shortcut {
    pub chord: String,
    pub action: String,
    pub arg: Option<String>,
}

impl Shortcut {
    pub fn action(chord: &str, action: &str) -> Self {
        Shortcut { chord: chord.into(), action: action.into(), arg: None }
    }
    pub fn with_arg(chord: &str, action: &str, arg: &str) -> Self {
        Shortcut { chord: chord.into(), action: action.into(), arg: Some(arg.into()) }
    }
}

/// A parsed chord — pure data, no platform types, so the shell and the
/// harness share one parser and match against one representation.
/// `cmd` is the platform-primary modifier (Super on macOS, Ctrl
/// elsewhere — the shell decides which physical modifier maps to it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chord {
    pub cmd: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub key: ChordKey,
}

/// The closed key set of the §19.2 chord grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChordKey {
    /// A single character (letters, digits) — lowercase.
    Char(char),
    Escape,
    Backspace,
    Delete,
    Enter,
    Space,
    Tab,
    Up,
    Down,
    Left,
    Right,
}

/// Parse a §19.2 chord string: lowercase, `+`-joined, `modifier* + key`.
pub fn parse_chord(s: &str) -> Result<Chord, String> {
    let mut chord =
        Chord { cmd: false, ctrl: false, alt: false, shift: false, key: ChordKey::Space };
    let mut key: Option<ChordKey> = None;
    for part in s.split('+') {
        match part {
            "cmd" => chord.cmd = true,
            "ctrl" => chord.ctrl = true,
            "alt" => chord.alt = true,
            "shift" => chord.shift = true,
            _ => {
                if key.is_some() {
                    return Err(format!("chord '{s}': more than one key ('{part}')"));
                }
                key = Some(match part {
                    "escape" => ChordKey::Escape,
                    "backspace" => ChordKey::Backspace,
                    "delete" => ChordKey::Delete,
                    "enter" => ChordKey::Enter,
                    "space" => ChordKey::Space,
                    "tab" => ChordKey::Tab,
                    "up" => ChordKey::Up,
                    "down" => ChordKey::Down,
                    "left" => ChordKey::Left,
                    "right" => ChordKey::Right,
                    p => {
                        let mut chars = p.chars();
                        match (chars.next(), chars.next()) {
                            (Some(c), None) if c.is_ascii_lowercase() || c.is_ascii_digit() => {
                                ChordKey::Char(c)
                            }
                            _ => {
                                return Err(format!(
                                    "chord '{s}': unknown key '{p}' — a single lowercase \
                                     char/digit or one of escape/backspace/delete/enter/space/tab/up/down/left/right"
                                ))
                            }
                        }
                    }
                });
            }
        }
    }
    match key {
        Some(k) => {
            chord.key = k;
            Ok(chord)
        }
        None => Err(format!("chord '{s}': no key (only modifiers)")),
    }
}

/// §19.2 precedence, shared by the shell and the harness: while a text
/// control is focused, only `cmd` chords the editing keymap doesn't
/// claim may fire. Reserved while focused: everything without `cmd`
/// (plain typing/navigation), and `cmd`+`c/x/v/z/a` (clipboard,
/// undo/redo, select-all) plus `cmd`+`left/right` (Home/End).
pub fn shortcut_fires_while_focused(chord: &Chord) -> bool {
    if !chord.cmd {
        return false;
    }
    !matches!(
        chord.key,
        ChordKey::Char('c' | 'x' | 'v' | 'z' | 'a') | ChordKey::Left | ChordKey::Right
    )
}

/// Validate an app's shortcut declarations against its reflected
/// actions (§19.2 load-time validation): every chord must parse, every
/// action must resolve — unit actions via `action_by_name`, arg'd ones
/// via `action_with_str`. Errors name the offending entry.
pub fn build_shortcut_table<M: ReflectAction>(
    shortcuts: Vec<Shortcut>,
) -> Result<Vec<(Chord, Shortcut)>, String> {
    let mut table = Vec::with_capacity(shortcuts.len());
    for s in shortcuts {
        let chord = parse_chord(&s.chord)?;
        let resolves = match &s.arg {
            Some(arg) => M::action_with_str(&s.action, arg.clone()).is_some(),
            None => M::action_by_name(&s.action).is_some(),
        };
        if !resolves {
            let kind = if s.arg.is_some() { "String-payload" } else { "unit" };
            return Err(format!(
                "shortcut '{}': '{}' is not a {kind} action — check the actions block (§19.2)",
                s.chord, s.action
            ));
        }
        if table.iter().any(|(c, _)| *c == chord) {
            return Err(format!("shortcut '{}': chord declared twice", s.chord));
        }
        table.push((chord, s));
    }
    Ok(table)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Counter {
        count: i64,
        quote: Remote<String, String>,
    }

    #[derive(Clone, Debug, PartialEq)]
    enum Msg {
        Inc,
        Fetch,
        Got(HttpResult),
    }

    // Hand-implemented reflection (the derive is the normal path; this
    // proves custom projections stay possible).
    impl ReflectState for Counter {
        fn field(&self, path: &str) -> Option<String> {
            match path {
                "count" => Some(self.count.to_string()),
                _ => None,
            }
        }
        fn field_names() -> &'static [&'static str] {
            &["count"]
        }
    }

    impl ReflectAction for Msg {
        fn action_by_name(name: &str) -> Option<Msg> {
            match name {
                "inc" => Some(Msg::Inc),
                _ => None,
            }
        }
        fn action_names() -> &'static [&'static str] {
            &["inc", "fetch", "got"]
        }
    }

    impl App for Counter {
        type Msg = Msg;
        fn init() -> (Self, Vec<Effect<Msg>>) {
            (Counter { count: 0, quote: Remote::NotAsked }, vec![])
        }
        fn update(&mut self, msg: Msg) -> Vec<Effect<Msg>> {
            match msg {
                Msg::Inc => {
                    self.count += 1;
                    vec![Effect::kv_set("count", self.count.to_le_bytes().to_vec())]
                }
                Msg::Fetch => {
                    self.quote = Remote::Loading;
                    vec![Effect::http(HttpRequest::get("https://api.github.com/zen"), Msg::Got)]
                }
                Msg::Got(HttpResult::Response { status: 200, body }) => {
                    self.quote = Remote::Success(String::from_utf8_lossy(&body).into_owned());
                    vec![]
                }
                Msg::Got(r) => {
                    self.quote = Remote::Failure(format!("{r:?}"));
                    vec![]
                }
            }
        }
    }

    #[test]
    fn the_loop_shape_holds() {
        let (mut m, cmds) = Counter::init();
        assert!(cmds.is_empty());
        let cmds = m.update(Msg::Inc);
        assert_eq!(cmds, vec![Effect::kv_set("count", 1i64.to_le_bytes().to_vec())]);
        assert_eq!(m.field("count").as_deref(), Some("1"));
    }

    #[test]
    fn effects_are_data_you_assert_on_and_feed_back() {
        let (mut m, _) = Counter::init();
        let cmds = m.update(Msg::Fetch);
        assert!(m.quote.is_loading());
        // The effect is data: pull the ctor out and hand-feed a result —
        // this is the whole integration-test mechanism in miniature.
        let Effect::Http { req, on_done } = &cmds[0] else { panic!("expected Http") };
        assert_eq!(req.url, "https://api.github.com/zen");
        let msg = on_done(HttpResult::Response { status: 200, body: b"Mind the gap.".to_vec() });
        m.update(msg);
        assert_eq!(m.quote.success().map(String::as_str), Some("Mind the gap."));
    }

    #[test]
    fn into_effects_covers_the_four_action_return_shapes() {
        let e: Effect<Msg> = Effect::http(HttpRequest::get("x"), Msg::Got);
        let empty: Vec<Effect<Msg>> = ().into_effects();
        assert_eq!(empty, Vec::<Effect<Msg>>::new());
        assert_eq!(e.clone().into_effects(), vec![e.clone()]);
        assert_eq!(Some(e.clone()).into_effects(), vec![e.clone()]);
        let none: Vec<Effect<Msg>> = None::<Effect<Msg>>.into_effects();
        assert_eq!(none, Vec::<Effect<Msg>>::new());
        assert_eq!(vec![e.clone(), e.clone()].into_effects(), vec![e.clone(), e]);
    }

    #[test]
    fn nav_stack_pushes_pops_and_protects_the_root() {
        #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
        enum Screen {
            Home,
            Detail(u32),
            Settings,
        }
        let mut nav = NavStack::new(Screen::Home);
        assert!(!nav.can_pop() && !nav.pop());
        nav.push(Screen::Detail(7));
        nav.push(Screen::Settings);
        assert_eq!((nav.depth(), nav.current()), (3, &Screen::Settings));
        assert!(nav.pop());
        assert_eq!(nav.current(), &Screen::Detail(7));
        nav.replace(Screen::Detail(9));
        assert_eq!(nav.current(), &Screen::Detail(9));
        assert!(nav.pop());
        assert!(!nav.pop(), "root is never popped");
        assert_eq!(nav.current(), &Screen::Home);
    }

    #[test]
    fn form_field_tracks_dirty_touched_error_and_submit_gating() {
        fn not_empty(s: &String) -> Option<String> {
            s.is_empty().then(|| "required".to_string())
        }
        let mut f = Field::new(String::new()).with_validator(not_empty);
        // Invalid from the start, but the error is gated until touched:
        assert!(!f.is_valid());
        assert_eq!(f.visible_error(), None);
        assert!(!f.is_dirty() && !f.is_touched());

        f.set("callsign".into());
        assert!(f.is_valid() && f.is_dirty() && f.is_touched());

        f.set(String::new());
        assert_eq!(f.visible_error(), Some("required"));

        f.set("AX-4".into());
        f.commit(); // submitted: clean at the new value
        assert!(!f.is_dirty() && !f.is_touched() && f.is_valid());
        assert_eq!(f.to_string(), "AX-4"); // Display → bindable via LibraReflect
    }

    #[test]
    fn effect_equality_is_over_data_not_ctor_identity() {
        fn other_ctor(r: HttpResult) -> Msg {
            Msg::Got(r)
        }
        // Different ctors, same effect description → equal.
        let a: Effect<Msg> = Effect::http(HttpRequest::get("x"), Msg::Got);
        let b: Effect<Msg> = Effect::http(HttpRequest::get("x"), other_ctor);
        assert_eq!(a, b);
        // Different data → unequal.
        assert_ne!(a, Effect::http(HttpRequest::get("y"), Msg::Got));
        // Acked vs fire-and-forget KvSet → unequal (presence matters).
        let acked: Effect<Msg> =
            Effect::kv_set_acked("k", vec![1], |_| Msg::Got(HttpResult::Failed(String::new())));
        assert_ne!(acked, Effect::kv_set("k", vec![1]));
    }

    // ---- §19.2 keyboard shortcuts ----

    #[test]
    fn chords_parse_modifiers_and_the_closed_key_set() {
        let c = parse_chord("cmd+shift+n").unwrap();
        assert!(c.cmd && c.shift && !c.alt);
        assert_eq!(c.key, ChordKey::Char('n'));
        assert_eq!(parse_chord("escape").unwrap().key, ChordKey::Escape);
        assert_eq!(parse_chord("cmd+1").unwrap().key, ChordKey::Char('1'));
        assert_eq!(parse_chord("cmd+alt+down").unwrap().key, ChordKey::Down);

        assert!(parse_chord("cmd+shift").unwrap_err().contains("no key"));
        assert!(parse_chord("cmd+n+m").unwrap_err().contains("more than one key"));
        assert!(parse_chord("cmd+F5").unwrap_err().contains("unknown key"));
    }

    #[test]
    fn shortcut_table_validates_action_names_at_build_time() {
        // Msg (this module's test enum): Inc is a unit variant.
        let ok = build_shortcut_table::<Msg>(vec![Shortcut::action("cmd+i", "inc")]);
        assert_eq!(ok.unwrap().len(), 1);

        let bad = build_shortcut_table::<Msg>(vec![Shortcut::action("cmd+q", "quench")]);
        assert!(bad.unwrap_err().contains("'quench' is not a unit action"));

        // A unit action declared with an arg fails the String-payload check.
        let bad = build_shortcut_table::<Msg>(vec![Shortcut::with_arg("cmd+i", "inc", "3")]);
        assert!(bad.unwrap_err().contains("String-payload"));

        let dup = build_shortcut_table::<Msg>(vec![
            Shortcut::action("cmd+i", "inc"),
            Shortcut::action("cmd+i", "inc"),
        ]);
        assert!(dup.unwrap_err().contains("declared twice"));
    }

    #[test]
    fn focused_precedence_reserves_the_editing_chords() {
        let fires = |s: &str| shortcut_fires_while_focused(&parse_chord(s).unwrap());
        assert!(!fires("n"), "un-modified keys belong to typing");
        assert!(!fires("escape"), "escape blurs; it never dispatches while focused");
        assert!(!fires("cmd+c"), "clipboard is the editor's");
        assert!(!fires("cmd+z"));
        assert!(!fires("cmd+left"), "cmd+arrows are Home/End");
        assert!(fires("cmd+n"), "unclaimed cmd chords fire while typing");
        assert!(fires("cmd+backspace"));
        assert!(fires("cmd+alt+down"));
    }
}
