//! The Phase 0 counter — the reference app for the determinism gate.

use libra_core::{App, Cmd, LibraReflect};

pub const VIEW: &str = include_str!("../views/counter.lx");

#[derive(Debug, Clone, PartialEq, LibraReflect)]
pub struct Counter {
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, LibraReflect)]
pub enum Msg {
    Increment,
    Decrement,
    Reset,
}

impl App for Counter {
    type Msg = Msg;

    fn init() -> (Self, Vec<Cmd<Msg>>) {
        (Counter { count: 0 }, vec![])
    }

    fn update(&mut self, msg: Msg) -> Vec<Cmd<Msg>> {
        match msg {
            Msg::Increment => self.count += 1,
            Msg::Decrement => self.count -= 1,
            Msg::Reset => self.count = 0,
        }
        vec![]
    }
}
