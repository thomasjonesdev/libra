//! Standalone starter app (PLAN-SDK S1, ARCHITECTURE.md §5.8).
//! Copy this crate out of the Libra checkout to start a new app.

use libra::{actions, app, state};

#[derive(Default)]
#[state]
pub struct Starter {
    pub count: i64,
}

#[actions]
impl Starter {
    pub fn increment(&mut self) {
        self.count += 1;
    }
    pub fn decrement(&mut self) {
        self.count -= 1;
    }
    pub fn reset(&mut self) {
        self.count = 0;
    }
}

app! {
    state: Starter,
    // Relative to this file (`include_str!`). Debug watch concatenates
    // `CARGO_MANIFEST_DIR + "/src/" + this literal`, so the `../views/`
    // form is load-bearing until S3 unifies the three view-path bases.
    view: "../views/starter.lx",
    window: (400.0, 240.0),
    title: "Libra Starter",
}
