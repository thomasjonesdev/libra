//! The gallery's snapshot matrix: every component × both themes, plus a
//! focused-input state. A component rendering change fails here first.

use gallery::{Action, Gallery, VIEW, WAVE};
use libra_data::Sample;
use libra_test::{by, fx, Harness};

/// Deterministic wave, fed synchronously — no generator thread in tests.
fn feed_wave(h: &Harness<Gallery>) {
    let ring = h.store().ring(WAVE);
    for k in 0..4000 {
        let t_ms = k as f64 * 0.5; // 2 kHz over 2 s
        ring.push(Sample { t_ms, v: (t_ms * 0.009).sin() * (1.0 + (k % 5) as f64 * 0.02) });
    }
}

fn harness(theme: &str) -> Harness<Gallery> {
    let mut h = Harness::<Gallery>::with_viewport(VIEW, 640, 360).theme(theme);
    let _ = h.expect_effect(fx::any()); // the IngestOpen — tests feed directly
    feed_wave(&h);
    h.dispatch(Action::Noop); // rebuild so paint sees the wave
    h
}

#[test]
fn snapshot_gallery_dark() {
    let h = harness("libra-dark");
    h.assert_snapshot("gallery_dark");
}

#[test]
fn snapshot_gallery_light() {
    let h = harness("libra-light");
    h.assert_snapshot("gallery_light");
}

#[test]
fn snapshot_gallery_focused_input_dark() {
    let mut h = harness("libra-dark");
    h.type_text(by::test_id("empty"), "typed");
    h.assert_snapshot("gallery_focus_dark");
}

/// Hover is an explicit paint input (§7.1 ListItem wash) — pinned to
/// an item's center, the frame is byte-exact like any other.
#[test]
fn snapshot_gallery_hovered_row_dark() {
    let mut h = harness("libra-dark");
    h.hover(by::test_id("row-plain"));
    h.assert_snapshot("gallery_hover_dark");
}

/// The hover wash renders and clears — hovered ≠ resting bytes, and
/// clearing restores the exact resting frame.
#[test]
fn hover_wash_appears_and_clears() {
    let mut h = harness("libra-dark");
    let resting = h.frame_png();
    h.hover(by::test_id("row-plain"));
    assert_ne!(h.frame_png(), resting, "hover wash changes the frame");
    h.blur_hover();
    assert_eq!(h.frame_png(), resting, "clearing hover restores identical bytes");
}

#[test]
fn theme_toggle_round_trips() {
    let mut h = harness("libra-dark");
    let dark = h.frame_png();
    h.click(by::test_id("theme"));
    let _ = h.expect_effect(fx::set_theme());
    assert_ne!(h.frame_png(), dark);
    h.click(by::test_id("theme"));
    let _ = h.expect_effect(fx::set_theme());
    assert_eq!(h.frame_png(), dark, "toggling back restores identical bytes");
}
