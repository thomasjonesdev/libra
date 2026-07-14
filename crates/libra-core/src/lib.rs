//! libra-core — the application contract: State/Action/reduce/Effect.
//!
//! Phase 1: the `App` trait is now just `init` + `update` (§5.1's
//! shape). Reflection — binding paths and handler names — lives in the
//! `ReflectState`/`ReflectAction` traits, normally implemented by
//! `#[derive(LibraReflect)]` (re-exported here from libra-reflect) and
//! hand-implementable when an app needs a custom projection. Plus the
//! Phase-1 subset of the §12.1 Effect vocabulary (Http, Kv*, Timer,
//! Random) and `Remote<T, E>` (Appendix C).
//!
//! Naming migration (ARCHITECTURE.md §3.2, MIGRATION-NAMING.md): the
//! founding names `Cmd`/`ReflectModel`/`ReflectMsg`/`MsgCtor` survive as
//! `#[deprecated]` aliases until Stage 8 removes them.

pub use libra_reflect::LibraReflect;

/// An action constructor: how an effect result becomes an action. Enum
/// tuple variants coerce to this directly (`Action::HttpDone` is a
/// `fn(HttpResult) -> Action`). Note: fn-pointer *equality* is
/// unreliable across crates (codegen duplicates the shim), so
/// `Effect`'s `PartialEq` deliberately compares the effect's data and
/// ignores ctor identity — effects are data; the ctor is routing.
pub type ActionCtor<T, M> = fn(T) -> M;

#[deprecated(note = "renamed to ActionCtor")]
pub type MsgCtor<T, M> = ActionCtor<T, M>;

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
}

/// Deprecated alias for `Effect`. A type alias can't apply
/// `#[deprecated]` to construction sites the way a struct/enum can, but
/// this keeps existing `Cmd<M>` signatures compiling.
#[deprecated(note = "renamed to Effect")]
pub type Cmd<M> = Effect<M>;

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

/// State reflection: how markup bindings resolve against the state.
/// Implemented by `#[derive(LibraReflect)]` on the State struct
/// (Display fields become bindable; `#[libra(skip)]` excludes one), or
/// by hand for custom projections. Drives the interpreter's load-time
/// validation and, later, the automation-server schema (§16).
pub trait ReflectState {
    /// Resolve a binding path (`count`) to display text.
    /// `None` makes the binding a load-time interp error.
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
}

/// Deprecated alias for `ReflectState`, kept as a standalone trait
/// (not a blanket impl over `ReflectState`) so a type never ends up
/// implementing both with the same method names in scope at once —
/// that would make `.field(...)` ambiguous for its callers. Old code
/// that hand-implements `ReflectModel` still compiles; it just doesn't
/// automatically satisfy a `ReflectState` bound, and vice versa.
#[deprecated(note = "renamed to ReflectState")]
pub trait ReflectModel {
    fn field(&self, path: &str) -> Option<String>;
    fn field_names() -> &'static [&'static str];
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

    /// Every variant name (diagnostics, automation schema).
    fn action_names() -> &'static [&'static str];
}

/// Deprecated alias for `ReflectAction` — standalone for the same
/// ambiguity-avoidance reason as `ReflectModel` above.
#[deprecated(note = "renamed to ReflectAction; msg_by_name/msg_with_str/msg_names \
    renamed to action_by_name/action_with_str/action_names")]
pub trait ReflectMsg: Sized {
    fn msg_by_name(name: &str) -> Option<Self>;
    fn msg_with_str(name: &str, value: String) -> Option<Self> {
        let _ = (name, value);
        None
    }
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
/// This is runtime plumbing: `libra::app!` (§5.8) and the
/// `#[libra::actions]` macro (§5.2) generate an impl of this trait from
/// state + actions, so most apps never write one by hand. `Msg`/
/// `update` keep their names here pending the Stage 4 macro work; the
/// user-facing vocabulary is state/action/reduce (§3.1).
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
    #[allow(deprecated)]
    fn deprecated_aliases_still_compile_and_behave_identically() {
        // Cmd/ReflectModel/ReflectMsg/MsgCtor survive as aliases until
        // MIGRATION-NAMING.md Stage 8. `Cmd` is a true type alias for
        // `Effect`, so it's interchangeable at the value level. The
        // deprecated reflect traits are kept as standalone (not
        // blanket-derived) traits — see the doc comment on
        // `ReflectModel` for why — so old code that hand-implements
        // them still compiles; this covers exactly that path.
        let a: Cmd<Msg> = Cmd::http(HttpRequest::get("x"), Msg::Got);
        let b: Effect<Msg> = Effect::http(HttpRequest::get("x"), Msg::Got);
        assert_eq!(a, b);

        struct OldStyleCounter {
            count: i64,
        }

        #[allow(deprecated)]
        impl ReflectModel for OldStyleCounter {
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

        #[allow(deprecated)]
        fn field_via_old_trait<T: ReflectModel>(t: &T, path: &str) -> Option<String> {
            t.field(path)
        }
        let c = OldStyleCounter { count: 42 };
        assert_eq!(field_via_old_trait(&c, "count").as_deref(), Some("42"));

        #[derive(Debug, Clone, PartialEq)]
        enum OldStyleMsg {
            Inc,
        }

        #[allow(deprecated)]
        impl ReflectMsg for OldStyleMsg {
            fn msg_by_name(name: &str) -> Option<Self> {
                match name {
                    "Inc" => Some(OldStyleMsg::Inc),
                    _ => None,
                }
            }
            fn msg_names() -> &'static [&'static str] {
                &["Inc"]
            }
        }

        #[allow(deprecated)]
        fn by_name_via_old_trait<T: ReflectMsg>(name: &str) -> Option<T> {
            T::msg_by_name(name)
        }
        assert_eq!(by_name_via_old_trait::<OldStyleMsg>("Inc"), Some(OldStyleMsg::Inc));
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
}
