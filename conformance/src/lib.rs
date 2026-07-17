//! The interp≡codegen conformance suite (ARCHITECTURE.md §6.4): golden
//! fixtures run through both engines; the trees must match node for
//! node. `TestState`/`TestAction` exercise every binding & handler kind
//! the grammar knows.

use libra_core::{FrameInfo, Instant, LibraReflect, SeriesHandle, SeriesId};

#[derive(Debug, Clone, PartialEq, LibraReflect)]
pub struct Item {
    pub id: String,
    pub label: String,
    /// Bool row field — `selected={t.active}` on `<ListItem>` (both
    /// engines resolve it via the shared Display-"true" rule).
    pub active: bool,
}

impl std::fmt::Display for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Whole-value fallback for key={t} — id is unique.
        write!(f, "{}", self.id)
    }
}

/// Nested ReflectState for `{user.name}` / `test={user.active}`
/// (PLAN-ACCESSORS).
#[derive(Debug, Clone, PartialEq, LibraReflect)]
pub struct User {
    pub name: String,
    pub active: bool,
}

impl std::fmt::Display for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Debug, Clone, PartialEq, LibraReflect)]
pub struct TestState {
    pub count: i64,
    pub name: String,
    pub status: String,
    pub s0: SeriesHandle,
    /// §6.2's `<for each={series} as="s" key={s}>` — exercised by the
    /// for_loop fixture, not kitchen_sink (kept separate so this
    /// addition can't disturb kitchen_sink's existing tree shape).
    pub series: Vec<SeriesHandle>,
    /// §6.2 item fields (`{t.label}`, `key={t.id}`, `onPress={…(t.id)}`)
    /// — exercised by for_item_fields. Structured via LibraReflect on
    /// [`Item`]; `series` stays opaque whole-value.
    pub items: Vec<Item>,
    /// §6.2 `<if test={running}>` — exercised by if_else. Bool Display
    /// is `"true"`/`"false"`; codegen uses a typed `if state.running`.
    pub running: bool,
    /// §12.5 `<Stream id={feed}>` — string id for the mount/query v1.
    pub feed: String,
    /// PLAN-ACCESSORS — nested path + accessors fixture.
    pub user: User,
}

impl Default for TestState {
    fn default() -> Self {
        TestState {
            count: 0,
            name: String::new(),
            status: "ok".into(),
            s0: SeriesHandle::new(SeriesId(0), 3000.0),
            series: vec![
                SeriesHandle::new(SeriesId(1), 1000.0),
                SeriesHandle::new(SeriesId(2), 2000.0),
                SeriesHandle::new(SeriesId(3), 3000.0),
            ],
            items: vec![
                Item { id: "1".into(), label: "Alpha".into(), active: true },
                Item { id: "2".into(), label: "Beta".into(), active: false },
            ],
            running: true,
            feed: "telemetry".into(),
            user: User { name: "Ada".into(), active: true },
        }
    }
}

#[derive(Debug, Clone, PartialEq, LibraReflect)]
#[allow(dead_code)]
pub enum TestAction {
    Increment,
    Reset,
    Refresh,
    NameChanged(String),
    /// §12.5's `<Frames onTick={frame}>` — exercised by the frames
    /// fixture, not kitchen_sink/for_loop.
    Frame(FrameInfo),
    /// §12.5's `<Interval onTick={poll}>` — Instant-payload tick.
    Poll(Instant),
    /// §12.5's `<Stream onEvent={packet}>` — String body for v1.
    Packet(String),
}

/// The codegen engine's output, compiled into this crate. Any type
/// mismatch between markup and State/Action is a rustc error here —
/// §6.5's compile-time promise, mechanically kept.
pub mod generated {
    include!(concat!(env!("OUT_DIR"), "/kitchen_sink.rs"));
    include!(concat!(env!("OUT_DIR"), "/for_loop.rs"));
    include!(concat!(env!("OUT_DIR"), "/for_actions.rs"));
    include!(concat!(env!("OUT_DIR"), "/for_item_fields.rs"));
    include!(concat!(env!("OUT_DIR"), "/if_else.rs"));
    include!(concat!(env!("OUT_DIR"), "/frames.rs"));
    include!(concat!(env!("OUT_DIR"), "/interval_stream.rs"));
    include!(concat!(env!("OUT_DIR"), "/accessors.rs"));
    include!(concat!(env!("OUT_DIR"), "/chrome.rs"));
    include!(concat!(env!("OUT_DIR"), "/list_textarea.rs"));
}
