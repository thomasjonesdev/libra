//! Component tests (§15.3): headless view queries.

use dashboard::{Dashboard, VIEW};
use libra_components::Role;
use libra_test::{by, Harness};

#[test]
fn renders_the_chart_wall() {
    let h = Harness::<Dashboard>::with_viewport(VIEW, 900, 620);
    h.query(by::test_id("status")).assert_text("live · 16 series @ 10 kHz");
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
    h.query(by::test_id("status")).assert_text("live · 16 series @ 10 kHz");
    h.click(by::test_id("toggle"));
    // Pause closes ingest — the shell's display loop idles (§12.5); no
    // `<Frames>` pump is involved.
    h.query(by::test_id("status")).assert_text("paused");
    assert!(!h.state().running);
}
