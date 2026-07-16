//! Component tests (§15.3): headless view queries.

use dashboard::{Dashboard, VIEW};
use libra_components::Role;
use libra_test::{by, Harness};

#[test]
fn renders_the_chart_wall() {
    let h = Harness::<Dashboard>::with_viewport(VIEW, 900, 620);
    h.query(by::test_id("status")).assert_text("starting…");
    h.query(by::test_id("toggle")).assert_text("Pause");
    // §6.2's <for> doesn't bind a per-item testId (no dot-path access
    // into a loop-bound value yet, §26), so this checks presence via
    // role the way the pre-<for> version checked one hardcoded
    // testId — panics (via query) if no Chart rendered at all.
    h.query(by::role(Role::Chart));
}

#[test]
fn toggle_click_pauses_through_real_hit_testing() {
    let mut h = Harness::<Dashboard>::with_viewport(VIEW, 900, 620);
    assert!(h.has_source("frames"), "running → <Frames> mounted via <if>");
    h.click(by::test_id("toggle"));
    h.query(by::test_id("status")).assert_text("paused");
    assert!(!h.state().running);
    assert!(!h.has_source("frames"), "paused → <Frames> unmounted");
}
