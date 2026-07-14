//! Covers `libra::app!`'s `init:` field — the path §5.5's startup-effects
//! example needs (not exercised by tests/app.rs, which uses the
//! no-`init:` Default fallback).

use libra::{actions, app, state};

#[state]
struct Session {
    status: String,
}

#[actions]
impl Session {
    fn ready(&mut self) {
        self.status = "ready".into();
    }
}

impl Session {
    fn start() -> (Session, Vec<libra_core::Effect<Action>>) {
        (Session { status: "starting…".into() }, vec![])
    }
}

app! {
    state: Session,
    view: "fixtures/session.lx",
    init: Session::start,
    title: "Test Session",
}

#[test]
fn init_uses_the_explicit_path_instead_of_default() {
    let (s, effects) = <Session as libra_core::App>::init();
    assert_eq!(s.status, "starting…");
    assert!(effects.is_empty());
}

#[test]
fn window_field_is_optional_and_falls_back() {
    // No `window:` field above — proves the default doesn't block
    // compilation. Can't easily assert the fallback value without
    // opening a window, so this test is really just "it compiled".
    let _f: fn() -> Result<(), Box<dyn std::error::Error>> = run;
}

#[test]
fn actions_dispatch_through_the_explicit_init_state() {
    let mut h = libra_test::Harness::<Session>::render(VIEW);
    h.query(libra_test::by::test_id("status")).assert_text("starting…");
    h.dispatch(Action::Ready);
    h.query(libra_test::by::test_id("status")).assert_text("ready");
}
