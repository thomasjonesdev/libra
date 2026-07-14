//! Component tests (§15.3): real markup, real interpreter, real
//! layout, real hit testing, real dispatch. No display server anywhere.

use counter::{Action, Counter, VIEW};
use libra_components::Role;
use libra_test::{by, Harness};

#[test]
fn initial_render_shows_zero() {
    let h = Harness::<Counter>::render(VIEW);
    h.query(by::test_id("count")).assert_text("0");
    h.query(by::test_id("status")).assert_text("count is 0");
}

#[test]
fn click_increments_through_real_hit_testing() {
    let mut h = Harness::<Counter>::render(VIEW);
    h.click(by::test_id("inc"));
    h.click(by::test_id("inc"));
    h.click(by::test_id("inc"));
    h.query(by::test_id("count")).assert_text("3");
    assert_eq!(h.state().count, 3);
    h.click(by::test_id("dec"));
    h.query(by::test_id("count")).assert_text("2");
}

#[test]
fn click_by_visible_text() {
    let mut h = Harness::<Counter>::render(VIEW);
    h.click(by::text("+"));
    h.click(by::text("Reset"));
    h.query(by::test_id("count")).assert_text("0");
}

#[test]
fn roles_are_queryable() {
    let h = Harness::<Counter>::render(VIEW);
    // First button in paint order is Reset (header row).
    assert_eq!(h.query(by::role(Role::Button)).text(), "Reset");
}

#[test]
fn bound_text_tracks_state_after_direct_dispatch() {
    let mut h = Harness::<Counter>::render(VIEW);
    h.dispatch(Action::Increment);
    h.dispatch(Action::Increment);
    h.query(by::test_id("status")).assert_text("count is 2");
}
