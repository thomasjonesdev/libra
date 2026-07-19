//! The workbench (PLAN-GALLERY): sidebar select → isolated component canvas +
//! controls. Snapshots are now **per-component** (isolation), so a rendering
//! change fails only the affected component's frame.

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
    let mut h = Harness::<Gallery>::with_viewport(VIEW, 1200, 760).theme(theme);
    let _ = h.expect_effect(fx::any()); // the IngestOpen — tests feed directly
    h
}

/// Default landing: Button selected, its variants + playground + label control.
#[test]
fn snapshot_workbench_button_dark() {
    let h = harness("libra-dark");
    h.query(by::test_id("canvas-title")).assert_text("Button");
    h.assert_snapshot("workbench_button_dark");
}

#[test]
fn snapshot_workbench_button_light() {
    let h = harness("libra-light");
    h.assert_snapshot("workbench_button_light");
}

/// Selecting a sidebar entry swaps the canvas — through real hit testing on a
/// `<for>`-generated `ListItem` (queried by its title text).
#[test]
fn selecting_a_component_swaps_the_canvas() {
    let mut h = harness("libra-dark");
    h.query(by::test_id("canvas-title")).assert_text("Button");
    h.click(by::text("Badge"));
    h.query(by::test_id("canvas-title")).assert_text("Badge");
    assert_eq!(h.state().selected, "badge");
}

#[test]
fn snapshot_workbench_badge_dark() {
    let mut h = harness("libra-dark");
    h.dispatch(Action::Select("badge".into()));
    h.assert_snapshot("workbench_badge_dark");
}

#[test]
fn snapshot_workbench_heading_dark() {
    let mut h = harness("libra-dark");
    h.dispatch(Action::Select("heading".into()));
    h.query(by::test_id("canvas-title")).assert_text("Heading");
    h.assert_snapshot("workbench_heading_dark");
}

#[test]
fn snapshot_workbench_progress_dark() {
    let mut h = harness("libra-dark");
    h.dispatch(Action::Select("progress".into()));
    h.assert_snapshot("workbench_progress_dark");
}

/// Numeric binding end-to-end: the +/− control moves the `ctl_progress` f32,
/// and the `<Progress value={ctl_progress}>` bar repaints to match (§26 #8).
#[test]
fn progress_control_moves_the_bound_bar() {
    let mut h = harness("libra-dark");
    h.dispatch(Action::Select("progress".into()));
    let at_half = h.frame_png();
    h.click(by::test_id("ctl-progress-up"));
    assert!((h.state().ctl_progress - 0.6).abs() < 1e-6, "up nudges +0.1");
    assert_ne!(h.frame_png(), at_half, "the bound Progress bar repaints wider");
}

/// The label control feeds the playground instance (a bindable text prop).
#[test]
fn label_control_drives_the_playground() {
    let mut h = harness("libra-dark");
    h.dispatch(Action::LabelChanged("Hello".into()));
    h.query(by::test_id("pg-button")).assert_text("Hello");
}

/// The Chart canvas renders the live wave (data plane drives the loop; no
/// `<Frames>`). Feed then select so the rebuilt canvas paints the ring.
#[test]
fn snapshot_workbench_chart_dark() {
    let mut h = harness("libra-dark");
    feed_wave(&h);
    h.dispatch(Action::Select("chart".into()));
    h.assert_snapshot("workbench_chart_dark");
}

/// PLAN-SCROLL: at a short viewport the sidebar overflows; scrolling clips +
/// offsets it (the frame changes) and is deterministic + reversible.
#[test]
fn sidebar_scrolls_and_round_trips() {
    let mut h = Harness::<Gallery>::with_viewport(VIEW, 900, 300).theme("libra-dark");
    let _ = h.expect_effect(fx::any()); // IngestOpen
    let top = h.frame_png();
    h.scroll("sidebar", 120.0);
    assert_ne!(h.frame_png(), top, "scrolling the sidebar repaints it");
    h.scroll("sidebar", -999.0); // clamps back to 0
    assert_eq!(h.frame_png(), top, "scrolling back to the top restores identical bytes");
}

#[test]
fn snapshot_sidebar_scrolled() {
    let mut h = Harness::<Gallery>::with_viewport(VIEW, 900, 300).theme("libra-dark");
    let _ = h.expect_effect(fx::any());
    h.scroll("sidebar", 150.0);
    h.assert_snapshot("workbench_sidebar_scrolled");
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

/// M6 `<Dialog>`: the workbench cell opens the modal; the snapshot
/// covers scrim + panel byte-exactly, and dismissing restores the
/// resting frame.
#[test]
fn snapshot_workbench_dialog_dark() {
    let mut h = harness("libra-dark");
    h.dispatch(Action::Select("dialog".into()));
    let resting = h.frame_png();
    h.click(by::test_id("pg-open-dialog"));
    h.assert_snapshot("workbench_dialog_dark");
    h.click(by::test_id("pg-dialog-close"));
    assert_eq!(h.frame_png(), resting, "closing restores identical bytes");
}

/// M5 `<Split>`: the harness resize verb drives the same controlled
/// path as the shell's divider drag; resizing repaints, and resizing
/// back restores identical bytes.
#[test]
fn snapshot_workbench_split_dark_and_resize_round_trips() {
    let mut h = harness("libra-dark");
    h.dispatch(Action::Select("split".into()));
    h.assert_snapshot("workbench_split_dark");
    let at_half = h.frame_png();
    h.resize_split("pg-split", 0.25);
    assert!((h.state().ctl_split - 0.25).abs() < 1e-4, "fraction echoed into the model");
    assert_ne!(h.frame_png(), at_half, "panes repaint at the new fraction");
    h.resize_split("pg-split", 0.5);
    assert_eq!(h.frame_png(), at_half, "resizing back restores identical bytes");
}
