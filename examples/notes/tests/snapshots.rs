//! Snapshot tests (§15.5): byte-exact PNG comparison across the three
//! panes, empty/populated states, overlays, and both shipped themes
//! (PLAN-NOTES-APP M8 light matrix).

use libra_core::KvResult;
use libra_test::{by, fx, Harness};
use notes::{Action, Notes, STORE_KEY, VIEW};

fn harness() -> Harness<Notes> {
    let mut h = Harness::<Notes>::with_viewport(VIEW, 900, 600);
    let load = h.expect_effect(fx::kv_get(STORE_KEY));
    h.resolve_kv_get(load, None);
    h
}

fn harness_light() -> Harness<Notes> {
    harness().theme("libra-light")
}

fn settle_save(h: &mut Harness<Notes>) {
    while h.pending_effects().any(|e| matches!(e, libra_core::Effect::Timer { .. })) {
        let t = h.expect_effect(fx::timer());
        h.fire_timer(t);
    }
    while h.pending_effects().any(|e| matches!(e, libra_core::Effect::KvSet { .. })) {
        let kv = h.expect_effect(fx::kv_set(STORE_KEY));
        h.resolve_kv_set(kv, KvResult::Ok);
    }
}

fn seed_note_with_body(h: &mut Harness<Notes>, body: &str) {
    h.click(by::test_id("new-note"));
    settle_save(h);
    h.type_text(by::test_id("editor-body"), body);
    settle_save(h);
}

#[test]
fn snapshot_empty_state_dark() {
    let h = harness();
    h.assert_snapshot("notes_empty_dark");
}

#[test]
fn snapshot_empty_state_light() {
    let h = harness_light();
    h.assert_snapshot("notes_empty_light");
}

#[test]
fn snapshot_one_note_with_body_dark() {
    let mut h = harness();
    seed_note_with_body(&mut h, "Grocery list\nmilk, eggs, bread");
    h.assert_snapshot("notes_with_body");
}

#[test]
fn snapshot_one_note_with_body_light() {
    let mut h = harness_light();
    seed_note_with_body(&mut h, "Grocery list\nmilk, eggs, bread");
    h.assert_snapshot("notes_with_body_light");
}

#[test]
fn snapshot_recently_deleted_visible_after_a_delete_dark() {
    let mut h = harness();
    h.click(by::test_id("new-note"));
    settle_save(&mut h);
    h.click(by::test_id("delete-note"));
    settle_save(&mut h);
    h.assert_snapshot("notes_recently_deleted");
}

#[test]
fn snapshot_recently_deleted_visible_after_a_delete_light() {
    let mut h = harness_light();
    h.click(by::test_id("new-note"));
    settle_save(&mut h);
    h.click(by::test_id("delete-note"));
    settle_save(&mut h);
    h.assert_snapshot("notes_recently_deleted_light");
}

/// M6: the folder dialog renders over the app — scrim, centered panel,
/// inline error line — byte-exactly.
#[test]
fn snapshot_folder_dialog_with_validation_error_dark() {
    let mut h = harness();
    h.click(by::test_id("new-note"));
    settle_save(&mut h);
    h.click(by::test_id("new-folder"));
    h.click(by::test_id("dialog-confirm")); // blank name → inline error
    h.assert_snapshot("notes_folder_dialog");
}

#[test]
fn snapshot_folder_dialog_with_validation_error_light() {
    let mut h = harness_light();
    h.click(by::test_id("new-note"));
    settle_save(&mut h);
    h.click(by::test_id("new-folder"));
    h.click(by::test_id("dialog-confirm"));
    h.assert_snapshot("notes_folder_dialog_light");
}

/// The Settings drawer: Card-grouped Switch + Radio group, anchored to
/// the right edge over the scrim — byte-exactly.
#[test]
fn snapshot_settings_drawer_open_dark() {
    let mut h = harness();
    h.click(by::test_id("settings"));
    h.assert_snapshot("notes_settings_drawer");
}

#[test]
fn snapshot_settings_drawer_open_light() {
    let mut h = harness_light();
    h.click(by::test_id("settings"));
    h.assert_snapshot("notes_settings_drawer_light");
}

#[test]
fn light_and_dark_render_different_bytes() {
    let dark = Harness::<Notes>::with_viewport(VIEW, 900, 600).frame_png();
    let light = Harness::<Notes>::with_viewport(VIEW, 900, 600).theme("libra-light").frame_png();
    assert_ne!(dark, light, "theme must be part of the frame's identity");
}

/// M7: the context menu floats at the right-clicked row — anchored by
/// a deterministic input, so the open-menu frame is byte-exact.
fn open_note_context_menu(h: &mut Harness<Notes>) {
    seed_note_with_body(h, "Menu target");
    // A second, later, selected note: the selected note's title also
    // appears in the header Breadcrumb, so `by::text` needs a
    // non-selected title to right-click unambiguously.
    h.dispatch(Action::Tick(libra_core::Instant::at(60_000.0)));
    h.click(by::test_id("new-note"));
    settle_save(h);
    h.type_text(by::test_id("editor-body"), "Other note");
    settle_save(h);
    h.blur();
    h.right_click(by::text("Menu target"));
}

#[test]
fn snapshot_note_context_menu_open_dark() {
    let mut h = harness();
    open_note_context_menu(&mut h);
    h.assert_snapshot("notes_context_menu");
}

#[test]
fn snapshot_note_context_menu_open_light() {
    let mut h = harness_light();
    open_note_context_menu(&mut h);
    h.assert_snapshot("notes_context_menu_light");
}
