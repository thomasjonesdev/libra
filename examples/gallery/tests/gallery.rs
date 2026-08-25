//! The workbench (PLAN-GALLERY / PLAN-NOTES-APP M8): sidebar select →
//! isolated component canvas + controls. Snapshots are **per-component**
//! under both built-in themes, so a rendering change fails only the
//! affected component × theme frame.

use gallery::{Action, Gallery, VIEW, WAVE};
use libra_data::Sample;
use libra_test::{by, fx, Harness};
use std::path::PathBuf;

fn view_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("views")
}

/// Deterministic wave, fed synchronously — no generator thread in tests.
fn feed_wave(h: &Harness<Gallery>) {
    let ring = h.store().ring(WAVE);
    for k in 0..4000 {
        let t_ms = k as f64 * 0.5; // 2 kHz over 2 s
        ring.push(Sample { t_ms, v: (t_ms * 0.009).sin() * (1.0 + (k % 5) as f64 * 0.02) });
    }
}

fn harness(theme: &str) -> Harness<Gallery> {
    let mut h = Harness::<Gallery>::with_view_dir(VIEW, &view_dir(), 1200, 760).theme(theme);
    let _ = h.expect_effect(fx::any()); // the IngestOpen — tests feed directly
    h
}

fn select(theme: &str, id: &str) -> Harness<Gallery> {
    let mut h = harness(theme);
    h.dispatch(Action::Select(id.into()));
    h
}

/// Default landing: Button selected, its variants + playground + label control.
#[test]
#[libra::verifies("REQ-UI-BUTTON-001")]
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
    select("libra-dark", "badge").assert_snapshot("workbench_badge_dark");
}

#[test]
fn snapshot_workbench_badge_light() {
    select("libra-light", "badge").assert_snapshot("workbench_badge_light");
}

#[test]
fn snapshot_workbench_heading_dark() {
    let h = select("libra-dark", "heading");
    h.query(by::test_id("canvas-title")).assert_text("Heading");
    h.assert_snapshot("workbench_heading_dark");
}

#[test]
fn snapshot_workbench_heading_light() {
    select("libra-light", "heading").assert_snapshot("workbench_heading_light");
}

#[test]
fn snapshot_workbench_progress_dark() {
    select("libra-dark", "progress").assert_snapshot("workbench_progress_dark");
}

#[test]
fn snapshot_workbench_progress_light() {
    select("libra-light", "progress").assert_snapshot("workbench_progress_light");
}

/// Numeric binding end-to-end: the +/− control moves the `ctl_progress` f32,
/// and the `<Progress value={ctl_progress}>` bar repaints to match (§26 #8).
#[test]
fn progress_control_moves_the_bound_bar() {
    let mut h = select("libra-dark", "progress");
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

#[test]
fn snapshot_workbench_chart_light() {
    let mut h = harness("libra-light");
    feed_wave(&h);
    h.dispatch(Action::Select("chart".into()));
    h.assert_snapshot("workbench_chart_light");
}

/// PLAN-SCROLL: at a short viewport the sidebar overflows; scrolling clips +
/// offsets it (the frame changes) and is deterministic + reversible.
#[test]
fn sidebar_scrolls_and_round_trips() {
    let mut h = Harness::<Gallery>::with_view_dir(VIEW, &view_dir(), 900, 300).theme("libra-dark");
    let _ = h.expect_effect(fx::any()); // IngestOpen
    let top = h.frame_png();
    h.scroll("sidebar", 120.0);
    assert_ne!(h.frame_png(), top, "scrolling the sidebar repaints it");
    h.scroll("sidebar", -999.0); // clamps back to 0
    assert_eq!(h.frame_png(), top, "scrolling back to the top restores identical bytes");
}

#[test]
fn snapshot_sidebar_scrolled() {
    let mut h = Harness::<Gallery>::with_view_dir(VIEW, &view_dir(), 900, 300).theme("libra-dark");
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
    let mut h = select("libra-dark", "dialog");
    let resting = h.frame_png();
    h.click(by::test_id("pg-open-dialog"));
    h.assert_snapshot("workbench_dialog_dark");
    h.click(by::test_id("pg-dialog-close"));
    assert_eq!(h.frame_png(), resting, "closing restores identical bytes");
}

#[test]
fn snapshot_workbench_dialog_light() {
    let mut h = select("libra-light", "dialog");
    h.click(by::test_id("pg-open-dialog"));
    h.assert_snapshot("workbench_dialog_light");
}

/// M5 `<Split>`: the harness resize verb drives the same controlled
/// path as the shell's divider drag; resizing repaints, and resizing
/// back restores identical bytes.
#[test]
fn snapshot_workbench_split_dark_and_resize_round_trips() {
    let mut h = select("libra-dark", "split");
    h.assert_snapshot("workbench_split_dark");
    let at_half = h.frame_png();
    h.resize_split("pg-split", 0.25);
    assert!((h.state().ctl_split - 0.25).abs() < 1e-4, "fraction echoed into the model");
    assert_ne!(h.frame_png(), at_half, "panes repaint at the new fraction");
    h.resize_split("pg-split", 0.5);
    assert_eq!(h.frame_png(), at_half, "resizing back restores identical bytes");
}

#[test]
fn snapshot_workbench_split_light() {
    select("libra-light", "split").assert_snapshot("workbench_split_light");
}

/// M7 `<ContextMenu>`: right-click opens the anchored menu (byte-exact
/// snapshot); a menu item closes it back to the identical resting frame.
#[test]
fn snapshot_workbench_menu_and_round_trip() {
    let mut h = select("libra-dark", "list_item");
    let resting = h.frame_png();
    h.right_click(by::test_id("pg-row-menu"));
    h.assert_snapshot("workbench_menu_dark");
    h.click(by::test_id("pg-menu-a"));
    assert_eq!(h.frame_png(), resting, "closing restores identical bytes");
}

#[test]
fn snapshot_workbench_menu_light() {
    let mut h = select("libra-light", "list_item");
    h.right_click(by::test_id("pg-row-menu"));
    h.assert_snapshot("workbench_menu_light");
}

/// Resting ListItem / Textarea canvases (notes-critical; M8).
#[test]
fn snapshot_workbench_list_item_dark() {
    select("libra-dark", "list_item").assert_snapshot("workbench_list_item_dark");
}

#[test]
fn snapshot_workbench_list_item_light() {
    select("libra-light", "list_item").assert_snapshot("workbench_list_item_light");
}

#[test]
fn snapshot_workbench_chip_dark() {
    select("libra-dark", "chip").assert_snapshot("workbench_chip_dark");
}

#[test]
fn snapshot_workbench_chip_light() {
    select("libra-light", "chip").assert_snapshot("workbench_chip_light");
}

#[test]
fn snapshot_workbench_tabs_dark() {
    select("libra-dark", "tabs").assert_snapshot("workbench_tabs_dark");
}

#[test]
fn snapshot_workbench_tabs_light() {
    select("libra-light", "tabs").assert_snapshot("workbench_tabs_light");
}

/// Select switches the model; the trailing × fires onClose.
#[test]
fn tabs_select_and_close() {
    let mut h = select("libra-dark", "tabs");
    assert_eq!(h.cursor_over(by::test_id("pg-tab-editor")), libra_components::Cursor::Pointer);
    assert!(h.state().tab_editor);
    h.click(by::test_id("pg-tab-preview"));
    assert!(h.state().tab_preview, "press dispatched; the app flipped selected");
    assert_eq!(h.state().tab_body, "Previewing Atlas.md");
    assert!(!h.state().tab_dismissed);
    h.click(by::test_id("pg-tab-scratch-close"));
    assert!(h.state().tab_dismissed, "× dispatched onClose");
}

/// Select toggles the model; the trailing × fires onRemove.
#[test]
fn chip_selects_and_dismisses() {
    let mut h = select("libra-dark", "chip");
    assert_eq!(h.cursor_over(by::test_id("pg-chip")), libra_components::Cursor::Pointer);
    assert!(h.state().chip_on);
    h.click(by::test_id("pg-chip"));
    assert!(!h.state().chip_on, "press dispatched; the app flipped selected");
    h.click(by::test_id("pg-chip"));
    assert!(h.state().chip_on);
    assert!(!h.state().chip_dismissed);
    h.click(by::test_id("pg-chip-tag-remove"));
    assert!(h.state().chip_dismissed, "× dispatched onRemove");
}

#[test]
fn snapshot_workbench_search_dark() {
    select("libra-dark", "search").assert_snapshot("workbench_search_dark");
}

#[test]
fn snapshot_workbench_search_light() {
    select("libra-light", "search").assert_snapshot("workbench_search_light");
}

/// Typing updates the bound value; Enter fires `onSubmit`.
#[test]
fn search_types_and_submits_on_enter() {
    let mut h = select("libra-dark", "search");
    assert_eq!(h.cursor_over(by::test_id("pg-search")), libra_components::Cursor::Text);
    h.type_text(by::test_id("pg-search"), " more");
    assert_eq!(h.state().ctl_input, "editable more");
    assert!(!h.state().search_submitted);
    h.press_enter();
    assert!(h.state().search_submitted, "Enter dispatched onSubmit");
}

#[test]
fn snapshot_workbench_textarea_dark() {
    select("libra-dark", "textarea").assert_snapshot("workbench_textarea_dark");
}

#[test]
fn snapshot_workbench_textarea_light() {
    select("libra-light", "textarea").assert_snapshot("workbench_textarea_light");
}

/// Catalog parity wave: per-component snapshots + the model-owned
/// toggle round trips.
#[test]
fn snapshot_workbench_alert_dark() {
    select("libra-dark", "alert").assert_snapshot("workbench_alert_dark");
}

#[test]
fn snapshot_workbench_alert_light() {
    select("libra-light", "alert").assert_snapshot("workbench_alert_light");
}

#[test]
fn snapshot_workbench_card_and_panel_dark() {
    let mut h = select("libra-dark", "card");
    h.assert_snapshot("workbench_card_dark");
    h.dispatch(Action::Select("panel".into()));
    h.assert_snapshot("workbench_panel_dark");
}

#[test]
fn snapshot_workbench_card_and_panel_light() {
    let mut h = select("libra-light", "card");
    h.assert_snapshot("workbench_card_light");
    h.dispatch(Action::Select("panel".into()));
    h.assert_snapshot("workbench_panel_light");
}

#[test]
fn snapshot_workbench_skeleton_and_avatar_dark() {
    let mut h = select("libra-dark", "skeleton");
    h.assert_snapshot("workbench_skeleton_dark");
    h.dispatch(Action::Select("avatar".into()));
    h.assert_snapshot("workbench_avatar_dark");
}

#[test]
fn snapshot_workbench_skeleton_and_avatar_light() {
    let mut h = select("libra-light", "skeleton");
    h.assert_snapshot("workbench_skeleton_light");
    h.dispatch(Action::Select("avatar".into()));
    h.assert_snapshot("workbench_avatar_light");
}

#[test]
fn switch_and_checkbox_toggle_through_the_model() {
    let mut h = select("libra-dark", "switch");
    h.assert_snapshot("workbench_switch_dark");
    let on = h.frame_png();
    h.click(by::test_id("pg-switch"));
    assert!(!h.state().switch_on, "press dispatched; the app flipped the model");
    assert_ne!(h.frame_png(), on, "knob repaints off");
    h.click(by::test_id("pg-switch"));
    assert_eq!(h.frame_png(), on, "toggling back restores identical bytes");

    h.dispatch(Action::Select("checkbox".into()));
    h.assert_snapshot("workbench_checkbox_dark");
    h.click(by::test_id("pg-checkbox"));
    assert!(!h.state().check_on);
}

#[test]
fn snapshot_workbench_switch_and_checkbox_light() {
    let mut h = select("libra-light", "switch");
    h.assert_snapshot("workbench_switch_light");
    h.dispatch(Action::Select("checkbox".into()));
    h.assert_snapshot("workbench_checkbox_light");
}

#[test]
fn radio_group_picks_one() {
    let mut h = select("libra-dark", "radio");
    h.assert_snapshot("workbench_radio_dark");
    h.click(by::test_id("pg-radio-b"));
    assert_eq!(h.state().radio_pick, "b");
    assert!(h.state().radio_b && !h.state().radio_a, "projection follows the pick");
}

#[test]
fn snapshot_workbench_radio_light() {
    select("libra-light", "radio").assert_snapshot("workbench_radio_light");
}

#[test]
fn snapshot_workbench_drawer_and_round_trip() {
    let mut h = select("libra-dark", "drawer");
    let resting = h.frame_png();
    h.click(by::test_id("pg-open-drawer"));
    h.assert_snapshot("workbench_drawer_dark");
    h.click(by::test_id("pg-drawer-close"));
    assert_eq!(h.frame_png(), resting, "closing restores identical bytes");
}

#[test]
fn snapshot_workbench_drawer_light() {
    let mut h = select("libra-light", "drawer");
    h.click(by::test_id("pg-open-drawer"));
    h.assert_snapshot("workbench_drawer_light");
}

/// Slider: the harness verb drives the same controlled path as the
/// shell's track drag, and it visibly moves the bound Progress too.
#[test]
fn snapshot_workbench_slider_and_set_round_trips() {
    let mut h = select("libra-dark", "slider");
    h.assert_snapshot("workbench_slider_dark");
    let at_half = h.frame_png();
    h.set_slider("pg-slider", 0.8);
    assert!((h.state().ctl_slider - 0.8).abs() < 1e-4);
    assert_ne!(h.frame_png(), at_half, "knob, fill, and the bound Progress repaint");
    h.set_slider("pg-slider", 0.5);
    assert_eq!(h.frame_png(), at_half, "setting back restores identical bytes");
}

#[test]
fn snapshot_workbench_slider_light() {
    select("libra-light", "slider").assert_snapshot("workbench_slider_light");
}

/// Tooltip: hidden at rest, floats over the hovered target, and clears
/// back to identical resting bytes.
#[test]
fn snapshot_workbench_tooltip_hover() {
    let mut h = select("libra-dark", "tooltip");
    let resting = h.frame_png();
    h.hover(by::test_id("pg-tip-btn"));
    assert_ne!(h.frame_png(), resting, "hover paints the label");
    h.assert_snapshot("workbench_tooltip_hover_dark");
    h.blur_hover();
    assert_eq!(h.frame_png(), resting, "unhover restores identical bytes");
}

#[test]
fn snapshot_workbench_tooltip_hover_light() {
    let mut h = select("libra-light", "tooltip");
    h.hover(by::test_id("pg-tip-btn"));
    h.assert_snapshot("workbench_tooltip_hover_light");
}

/// Nav wave: per-component snapshots + the shared page-index
/// playground driving Pagination and Stepper together.
#[test]
fn snapshot_workbench_breadcrumb_dark() {
    let mut h = select("libra-dark", "breadcrumb");
    h.assert_snapshot("workbench_breadcrumb_dark");
    // The current crumb has no handler at all (proven at the
    // conformance layer); a middle crumb is a real link.
    h.click(by::test_id("pg-crumb-mid"));
}

#[test]
fn snapshot_workbench_breadcrumb_light() {
    select("libra-light", "breadcrumb").assert_snapshot("workbench_breadcrumb_light");
}

#[test]
fn pagination_and_stepper_share_the_page_playground() {
    let mut h = select("libra-dark", "pagination");
    h.assert_snapshot("workbench_pagination_dark");
    h.query(by::test_id("pg-page-label")).assert_text("page 1");
    h.click(by::text("3"));
    assert_eq!(h.state().ctl_page, 2, "page button '3' is 1-based display for index 2");
    h.query(by::test_id("pg-page-label")).assert_text("page 2");

    h.dispatch(Action::Select("stepper".into()));
    h.assert_snapshot("workbench_stepper_dark");
}

#[test]
fn snapshot_workbench_pagination_and_stepper_light() {
    let mut h = select("libra-light", "pagination");
    h.assert_snapshot("workbench_pagination_light");
    h.dispatch(Action::Select("stepper".into()));
    h.assert_snapshot("workbench_stepper_light");
}

#[test]
fn snapshot_workbench_timeline_dark() {
    select("libra-dark", "timeline").assert_snapshot("workbench_timeline_dark");
}

#[test]
fn snapshot_workbench_timeline_light() {
    select("libra-light", "timeline").assert_snapshot("workbench_timeline_light");
}
