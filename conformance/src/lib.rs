//! The interp≡codegen conformance suite (ARCHITECTURE.md §6.4): golden
//! fixtures run through both engines; the trees must match node for
//! node. `TestState`/`TestAction` exercise every binding & handler kind
//! the grammar knows.

use libra_core::{FrameInfo, LibraReflect, SeriesHandle, SeriesId};

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
}

/// The codegen engine's output, compiled into this crate. Any type
/// mismatch between markup and State/Action is a rustc error here —
/// §6.5's compile-time promise, mechanically kept.
pub mod generated {
    use super::*;
    include!(concat!(env!("OUT_DIR"), "/kitchen_sink.rs"));
    include!(concat!(env!("OUT_DIR"), "/for_loop.rs"));
    include!(concat!(env!("OUT_DIR"), "/frames.rs"));
}
