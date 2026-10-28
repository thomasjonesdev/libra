//! Snapshot tests (§15.5). First run records; commit the PNG.

use libra_test::{by, Harness};
use starter::{Starter, VIEW};

#[test]
fn snapshot_initial() {
    let h = Harness::<Starter>::render(VIEW);
    h.assert_snapshot("starter_initial");
}

#[test]
fn rendering_is_deterministic_within_process() {
    let h = Harness::<Starter>::render(VIEW);
    let a = h.frame_png();
    let b = h.frame_png();
    assert_eq!(a, b, "same state + same view must produce identical bytes");
}

#[test]
fn different_state_produces_different_bytes() {
    let mut h = Harness::<Starter>::render(VIEW);
    let before = h.frame_png();
    h.click(by::test_id("inc"));
    let after = h.frame_png();
    assert_ne!(before, after, "a state change must change the pixels");
}
