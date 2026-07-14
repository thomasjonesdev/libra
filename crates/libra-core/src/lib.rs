//! libra-core — the application contract: Model/Msg/update/Cmd.
//!
//! Phase 1: the `App` trait is now just `init` + `update` (§5.1's
//! shape). Reflection — binding paths and handler names — lives in the
//! `ReflectModel`/`ReflectMsg` traits, normally implemented by
//! `#[derive(LibraReflect)]` (re-exported here from libra-reflect) and
//! hand-implementable when an app needs a custom projection. Plus the
//! Phase-1 subset of the §12.1 Cmd vocabulary (Http, Kv*, Timer,
//! Random) and `Remote<T, E>` (Appendix C).

pub use libra_reflect::LibraReflect;

/// A msg constructor: how an effect result becomes a msg. Enum tuple
/// variants coerce to this directly (`Msg::HttpDone` is a
/// `fn(HttpResult) -> Msg`). Note: fn-pointer *equality* is unreliable
/// across crates (codegen duplicates the shim), so `Cmd`'s `PartialEq`
/// deliberately compares the effect's data and ignores ctor identity —
/// effects are data; the ctor is routing.
pub type MsgCtor<T, M> = fn(T) -> M;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimerId(pub u64);

/// Identifies a data-plane series (§14.2). The Model never holds
/// samples — it holds handles; samples live in ring buffers read at
/// paint time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct SeriesId(pub u32);

/// The Model-side view of a series (§14.2): id + view window + pause
/// flag. Control decisions are pure msgs that mutate this struct;
/// samples never enter `update`.
///
/// Phase 2 reflection shortcut: markup binds a handle through its
/// `Display` form (`id|window_ms|paused`) since bindings are strings —
/// typed bindings arrive with the grammar's growth.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SeriesHandle {
    pub id: SeriesId,
    /// How much history the widget shows, in ms of sample time.
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

/// Ongoing event sources, declared by `App::subscriptions` and diffed
/// by the runtime (§12.2). Phase 2 subset: frame ticks. Streams join
/// with WebSocket/process cmds.
#[derive(Clone)]
pub enum Sub<M> {
    /// One msg per vsync-coalesced tick while subscribed.
    Frames { on_frame: MsgCtor<FrameInfo, M> },
}

impl<M> Sub<M> {
    pub fn frames(on_frame: MsgCtor<FrameInfo, M>) -> Self {
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

/// What comes back from `Cmd::Http`. Any HTTP status is `Response`
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

/// Effect descriptions (ARCHITECTURE.md §12.1, Phase-1 subset).
/// `update` returns these; the runtime executes them; tests intercept
/// them and hand-feed results back. Closed and versioned — grows in
/// later phases (Ws*, File*, Spawn, Interval, ingest…).
#[derive(Debug, Clone)]
pub enum Cmd<M> {
    // Network
    Http { req: HttpRequest, on_done: MsgCtor<HttpResult, M> },

    // Persistence (§13, executed by libra-kv via the effects runtime)
    KvGet { key: String, on_done: MsgCtor<Option<Vec<u8>>, M> },
    KvSet { key: String, value: Vec<u8>, on_done: Option<MsgCtor<KvResult, M>> },
    KvDelete { key: String },
    KvScan { prefix: String, on_done: MsgCtor<Vec<(String, Vec<u8>)>, M> },

    // Time & entropy (keeping update pure)
    Timer { id: Option<TimerId>, after_ms: u64, msg: M },
    CancelTimer { id: TimerId },
    Random { bytes: usize, on_done: MsgCtor<Vec<u8>, M> },

    // Data plane (§14): ingest threads owned by the runtime; samples
    // land in ring buffers, never in msgs.
    IngestOpen { source: SourceSpec, series: SeriesId },
    IngestClose { series: SeriesId },

    // Platform (§12.1): handled by the shell (and intercepted by the
    // harness), not the worker pool — the renderer re-resolves live.
    SetTheme { name: String },
}

/// Equality over the effect's *data*. Ctor fields compare by presence
/// only (`Some` vs `None`), never by fn-pointer identity — that
/// identity is not stable across crate boundaries, and the description
/// of an effect doesn't include where its result is routed.
impl<M: PartialEq> PartialEq for Cmd<M> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Cmd::Http { req: a, .. }, Cmd::Http { req: b, .. }) => a == b,
            (Cmd::KvGet { key: a, .. }, Cmd::KvGet { key: b, .. }) => a == b,
            (
                Cmd::KvSet { key: ka, value: va, on_done: da },
                Cmd::KvSet { key: kb, value: vb, on_done: db },
            ) => ka == kb && va == vb && da.is_some() == db.is_some(),
            (Cmd::KvDelete { key: a }, Cmd::KvDelete { key: b }) => a == b,
            (Cmd::KvScan { prefix: a, .. }, Cmd::KvScan { prefix: b, .. }) => a == b,
            (
                Cmd::Timer { id: ia, after_ms: aa, msg: ma },
                Cmd::Timer { id: ib, after_ms: ab, msg: mb },
            ) => ia == ib && aa == ab && ma == mb,
            (Cmd::CancelTimer { id: a }, Cmd::CancelTimer { id: b }) => a == b,
            (Cmd::Random { bytes: a, .. }, Cmd::Random { bytes: b, .. }) => a == b,
            (
                Cmd::IngestOpen { source: sa, series: ia },
                Cmd::IngestOpen { source: sb, series: ib },
            ) => sa == sb && ia == ib,
            (Cmd::IngestClose { series: a }, Cmd::IngestClose { series: b }) => a == b,
            (Cmd::SetTheme { name: a }, Cmd::SetTheme { name: b }) => a == b,
            _ => false,
        }
    }
}

impl<M> Cmd<M> {
    /// Convenience constructors mirroring §5.1's usage style.
    pub fn http(req: HttpRequest, on_done: MsgCtor<HttpResult, M>) -> Self {
        Cmd::Http { req, on_done }
    }
    pub fn kv_get(key: impl Into<String>, on_done: MsgCtor<Option<Vec<u8>>, M>) -> Self {
        Cmd::KvGet { key: key.into(), on_done }
    }
    pub fn kv_set(key: impl Into<String>, value: Vec<u8>) -> Self {
        Cmd::KvSet { key: key.into(), value, on_done: None }
    }
    pub fn kv_set_acked(
        key: impl Into<String>,
        value: Vec<u8>,
        on_done: MsgCtor<KvResult, M>,
    ) -> Self {
        Cmd::KvSet { key: key.into(), value, on_done: Some(on_done) }
    }
    pub fn timer(after_ms: u64, msg: M) -> Self {
        Cmd::Timer { id: None, after_ms, msg }
    }
}

/// Async-state type (Appendix C, via Elm's RemoteData): makes
/// loading/error impossible-states unrepresentable on the Model.
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

/// Model reflection: how markup bindings resolve against the model.
/// Implemented by `#[derive(LibraReflect)]` on the Model struct
/// (Display fields become bindable; `#[libra(skip)]` excludes one), or
/// by hand for custom projections. Drives the interpreter's load-time
/// validation and, later, the automation-server schema (§16).
pub trait ReflectModel {
    /// Resolve a binding path (`count`) to display text.
    /// `None` makes the binding a load-time interp error.
    fn field(&self, path: &str) -> Option<String>;

    /// Every bindable field name (diagnostics, automation schema).
    fn field_names() -> &'static [&'static str];
}

/// Msg reflection: how markup handler names resolve to msgs.
/// Implemented by `#[derive(LibraReflect)]` on the Msg enum: unit
/// variants are constructible by name; payload variants are effect
/// results, not UI handlers, and resolve to `None`.
pub trait ReflectMsg: Sized {
    fn msg_by_name(name: &str) -> Option<Self>;

    /// Resolve a handler name to a msg carrying a control-provided
    /// String payload (§6.2's `onChange={FilterChanged}` — the control
    /// supplies the value). Derived for single-`String` tuple variants;
    /// anything else resolves to `None`.
    fn msg_with_str(name: &str, value: String) -> Option<Self> {
        let _ = (name, value);
        None
    }

    /// Every variant name (diagnostics, automation schema).
    fn msg_names() -> &'static [&'static str];
}

/// Form-state helper (Appendix C, via React Hook Form): per-field
/// dirty/touched/error with a pure validator — plain model machinery,
/// Tier-1 testable, no framework runtime involved. Fields live on the
/// Model; `set` runs in `update`.
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
/// msgs. Plain model machinery — the root screen is never popped, so
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
///
/// Purity rules for `update` (ARCHITECTURE.md §5.2): no I/O, no clocks,
/// no randomness, no threads, no blocking. Time and entropy arrive
/// *inside* msgs. These rules are what make Tier-1 tests total.
pub trait App: Sized + ReflectModel {
    type Msg: Clone + std::fmt::Debug + ReflectMsg;

    /// Initial model + startup effects.
    fn init() -> (Self, Vec<Cmd<Self::Msg>>);

    /// The only place state changes. Pure.
    fn update(&mut self, msg: Self::Msg) -> Vec<Cmd<Self::Msg>>;

    /// Ambient event sources, derived from the model (§12.2). The
    /// runtime diffs the result after every update: new subs start,
    /// removed subs stop. Pure — so "tick while this screen is live"
    /// is a testable function.
    fn subscriptions(&self) -> Vec<Sub<Self::Msg>> {
        vec![]
    }
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
    impl ReflectModel for Counter {
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

    impl ReflectMsg for Msg {
        fn msg_by_name(name: &str) -> Option<Msg> {
            match name {
                "Inc" => Some(Msg::Inc),
                _ => None,
            }
        }
        fn msg_names() -> &'static [&'static str] {
            &["Inc", "Fetch", "Got"]
        }
    }

    impl App for Counter {
        type Msg = Msg;
        fn init() -> (Self, Vec<Cmd<Msg>>) {
            (Counter { count: 0, quote: Remote::NotAsked }, vec![])
        }
        fn update(&mut self, msg: Msg) -> Vec<Cmd<Msg>> {
            match msg {
                Msg::Inc => {
                    self.count += 1;
                    vec![Cmd::kv_set("count", self.count.to_le_bytes().to_vec())]
                }
                Msg::Fetch => {
                    self.quote = Remote::Loading;
                    vec![Cmd::http(HttpRequest::get("https://api.github.com/zen"), Msg::Got)]
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
        assert_eq!(cmds, vec![Cmd::kv_set("count", 1i64.to_le_bytes().to_vec())]);
        assert_eq!(m.field("count").as_deref(), Some("1"));
    }

    #[test]
    fn effects_are_data_you_assert_on_and_feed_back() {
        let (mut m, _) = Counter::init();
        let cmds = m.update(Msg::Fetch);
        assert!(m.quote.is_loading());
        // The cmd is data: pull the ctor out and hand-feed a result —
        // this is the whole Tier-3 mechanism in miniature.
        let Cmd::Http { req, on_done } = &cmds[0] else { panic!("expected Http") };
        assert_eq!(req.url, "https://api.github.com/zen");
        let msg = on_done(HttpResult::Response { status: 200, body: b"Mind the gap.".to_vec() });
        m.update(msg);
        assert_eq!(m.quote.success().map(String::as_str), Some("Mind the gap."));
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
    fn cmd_equality_is_over_data_not_ctor_identity() {
        fn other_ctor(r: HttpResult) -> Msg {
            Msg::Got(r)
        }
        // Different ctors, same effect description → equal.
        let a: Cmd<Msg> = Cmd::http(HttpRequest::get("x"), Msg::Got);
        let b: Cmd<Msg> = Cmd::http(HttpRequest::get("x"), other_ctor);
        assert_eq!(a, b);
        // Different data → unequal.
        assert_ne!(a, Cmd::http(HttpRequest::get("y"), Msg::Got));
        // Acked vs fire-and-forget KvSet → unequal (presence matters).
        let acked: Cmd<Msg> = Cmd::kv_set_acked("k", vec![1], |_| Msg::Got(HttpResult::Failed(String::new())));
        assert_ne!(acked, Cmd::kv_set("k", vec![1]));
    }
}
