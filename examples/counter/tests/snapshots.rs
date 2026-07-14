//! Snapshot tests (§15.5): byte-exact rendering through the canonical
//! CPU executor. Snapshot names are unchanged from before the test
//! reorg — PNG identity must not depend on how the suite is organized.

use counter::{Counter, VIEW};
use libra_test::{by, Harness};

#[test]
fn snapshot_initial() {
    let h = Harness::<Counter>::render(VIEW);
    h.assert_snapshot("counter_initial");
}

#[test]
fn snapshot_after_three_increments() {
    let mut h = Harness::<Counter>::render(VIEW);
    for _ in 0..3 {
        h.click(by::test_id("inc"));
    }
    h.assert_snapshot("counter_three");
}

#[test]
fn rendering_is_deterministic_within_process() {
    let h = Harness::<Counter>::render(VIEW);
    let a = h.frame_png();
    let b = h.frame_png();
    assert_eq!(a, b, "same state + same view must produce identical bytes");
}

#[test]
fn different_state_produces_different_bytes() {
    let mut h = Harness::<Counter>::render(VIEW);
    let before = h.frame_png();
    h.click(by::test_id("inc"));
    let after = h.frame_png();
    assert_ne!(before, after, "a state change must change the pixels");
}
