//! The interp≡codegen conformance suite (ARCHITECTURE.md §6.4): golden
//! fixtures run through both engines; the trees must match node for
//! node. `TestModel`/`TestMsg` exercise every binding & handler kind
//! the grammar knows.

use libra_core::{LibraReflect, SeriesHandle, SeriesId};

#[derive(Debug, Clone, PartialEq, LibraReflect)]
pub struct TestModel {
    pub count: i64,
    pub name: String,
    pub status: String,
    pub s0: SeriesHandle,
}

impl Default for TestModel {
    fn default() -> Self {
        TestModel {
            count: 0,
            name: String::new(),
            status: "ok".into(),
            s0: SeriesHandle::new(SeriesId(0), 3000.0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, LibraReflect)]
#[allow(dead_code)]
pub enum TestMsg {
    Increment,
    Reset,
    Refresh,
    NameChanged(String),
}

/// The codegen engine's output, compiled into this crate. Any type
/// mismatch between markup and Model/Msg is a rustc error here —
/// §6.5's compile-time promise, mechanically kept.
pub mod generated {
    use super::*;
    include!(concat!(env!("OUT_DIR"), "/kitchen_sink.rs"));
}
