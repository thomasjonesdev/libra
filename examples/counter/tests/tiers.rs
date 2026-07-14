//! The Phase 0 gate: all three implemented test tiers against the counter.
//! (Tier 3 arrives with the effects runtime in Phase 1; Tier 5 with replay
//! in Phase 4.)

use counter::{Counter, Msg, VIEW};
use libra_core::App;
use libra_test::{by, Harness};
use libra_components::Role;

// ---------------------------------------------------------------------------
// Tier 1 — pure logic. No harness, no rendering, no framework.
// ---------------------------------------------------------------------------

#[test]
fn tier1_increment_and_decrement() {
    let (mut m, cmds) = Counter::init();
    assert!(cmds.is_empty(), "counter has no startup effects");
    assert!(m.update(Msg::Increment).is_empty());
    assert!(m.update(Msg::Increment).is_empty());
    assert_eq!(m.count, 2);
    m.update(Msg::Decrement);
    assert_eq!(m.count, 1);
}

#[test]
fn tier1_reset_from_anywhere() {
    let (mut m, _) = Counter::init();
    for _ in 0..17 {
        m.update(Msg::Increment);
    }
    m.update(Msg::Reset);
    assert_eq!(m.count, 0);
}

#[test]
fn tier1_decrement_goes_negative_by_design() {
    let (mut m, _) = Counter::init();
    m.update(Msg::Decrement);
    assert_eq!(m.count, -1);
}

// ---------------------------------------------------------------------------
// Tier 2 — headless view: real markup, real interpreter, real layout,
// real hit testing, real update. No display server anywhere.
// ---------------------------------------------------------------------------

#[test]
fn tier2_initial_render_shows_zero() {
    let h = Harness::<Counter>::new(VIEW);
    h.query(by::test_id("count")).assert_text("0");
    h.query(by::test_id("status")).assert_text("count is 0");
}

#[test]
fn tier2_click_increments_through_real_hit_testing() {
    let mut h = Harness::<Counter>::new(VIEW);
    h.click(by::test_id("inc"));
    h.click(by::test_id("inc"));
    h.click(by::test_id("inc"));
    h.query(by::test_id("count")).assert_text("3");
    assert_eq!(h.model().count, 3);
    h.click(by::test_id("dec"));
    h.query(by::test_id("count")).assert_text("2");
}

#[test]
fn tier2_click_by_visible_text() {
    let mut h = Harness::<Counter>::new(VIEW);
    h.click(by::text("+"));
    h.click(by::text("Reset"));
    h.query(by::test_id("count")).assert_text("0");
}

#[test]
fn tier2_roles_are_queryable() {
    let h = Harness::<Counter>::new(VIEW);
    // First button in paint order is Reset (header row).
    assert_eq!(h.query(by::role(Role::Button)).text(), "Reset");
}

#[test]
fn tier2_bound_text_tracks_model_after_direct_dispatch() {
    let mut h = Harness::<Counter>::new(VIEW);
    h.dispatch(Msg::Increment);
    h.dispatch(Msg::Increment);
    h.query(by::test_id("status")).assert_text("count is 2");
}

// ---------------------------------------------------------------------------
// Tier 4 — byte-exact snapshots through the canonical CPU executor.
// ---------------------------------------------------------------------------

#[test]
fn tier4_snapshot_initial() {
    let h = Harness::<Counter>::new(VIEW);
    h.assert_snapshot("counter_initial");
}

#[test]
fn tier4_snapshot_after_three_increments() {
    let mut h = Harness::<Counter>::new(VIEW);
    for _ in 0..3 {
        h.click(by::test_id("inc"));
    }
    h.assert_snapshot("counter_three");
}

#[test]
fn tier4_rendering_is_deterministic_within_process() {
    let h = Harness::<Counter>::new(VIEW);
    let a = h.frame_png();
    let b = h.frame_png();
    assert_eq!(a, b, "same model + same view must produce identical bytes");
}

#[test]
fn tier4_different_state_produces_different_bytes() {
    let mut h = Harness::<Counter>::new(VIEW);
    let before = h.frame_png();
    h.click(by::test_id("inc"));
    let after = h.frame_png();
    assert_ne!(before, after, "a state change must change the pixels");
}
