//! Unit tests (§15.2): `reduce` only. No harness, no runtime.

use libra::App;
use starter::{reduce, Action, Starter};

#[test]
fn increment_and_decrement() {
    let (mut s, effects) = Starter::init();
    assert!(effects.is_empty(), "starter has no startup effects");
    assert!(reduce(&mut s, Action::Increment).is_empty());
    assert!(reduce(&mut s, Action::Increment).is_empty());
    assert_eq!(s.count, 2);
    reduce(&mut s, Action::Decrement);
    assert_eq!(s.count, 1);
}

#[test]
fn reset_from_anywhere() {
    let (mut s, _) = Starter::init();
    for _ in 0..17 {
        reduce(&mut s, Action::Increment);
    }
    reduce(&mut s, Action::Reset);
    assert_eq!(s.count, 0);
}
