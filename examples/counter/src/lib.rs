//! The Phase 0 counter — the reference app for the determinism gate.
//! §5.8's "counter in ~10 lines": state + actions + one `app!`
//! declaration replaces the old hand-written `App` impl.

use libra::{actions, app, state};

#[derive(Default)]
#[state]
pub struct Counter {
    pub count: i64,
}

#[actions]
impl Counter {
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
    state: Counter,
    view: "../views/counter.lx",
    window: (400.0, 240.0),
    title: "Libra Counter",
}

#[cfg(test)]
mod tests {
    use super::*;
    use libra_core::App;

    #[test]
    fn increment_and_decrement() {
        let (mut c, effects) = Counter::init();
        assert!(effects.is_empty(), "counter has no startup effects");
        assert!(reduce(&mut c, Action::Increment).is_empty());
        assert!(reduce(&mut c, Action::Increment).is_empty());
        assert_eq!(c.count, 2);
        reduce(&mut c, Action::Decrement);
        assert_eq!(c.count, 1);
    }

    #[test]
    fn reset_from_anywhere() {
        let (mut c, _) = Counter::init();
        for _ in 0..17 {
            reduce(&mut c, Action::Increment);
        }
        reduce(&mut c, Action::Reset);
        assert_eq!(c.count, 0);
    }

    #[test]
    fn decrement_goes_negative_by_design() {
        let (mut c, _) = Counter::init();
        reduce(&mut c, Action::Decrement);
        assert_eq!(c.count, -1);
    }
}
