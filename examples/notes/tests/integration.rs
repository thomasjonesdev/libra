//! Integration tests (§15.4): effect flows through the real Harness —
//! boot restore, debounced vs. immediate autosave, single-flight save,
//! theme toggle. No mocks; effects are intercepted and resolved by hand.

use libra_core::{Effect, KvResult};
use libra_test::{by, fx, Harness};
use notes::{Action, Notes, STORE_KEY, VIEW};

fn fresh_harness() -> Harness<Notes> {
    let mut h = Harness::<Notes>::with_viewport(VIEW, 900, 600);
    let load = h.expect_effect(fx::kv_get(STORE_KEY));
    h.resolve_kv_get(load, None);
    h
}

#[test]
fn boot_restores_before_first_render_and_shows_the_default_folder() {
    let h = fresh_harness();
    h.query(by::test_id("folder-name")).assert_text("Notes");
    h.query(by::test_id("empty")).assert_text("No notes yet");
}

#[test]
fn structural_change_persists_on_a_near_immediate_timer() {
    let mut h = fresh_harness();
    h.click(by::test_id("new-note"));
    let save = h.expect_effect(fx::timer());
    let Effect::Timer { after_ms, .. } = &save else { unreachable!() };
    assert_eq!(*after_ms, 0, "structural changes don't wait for the debounce");
    h.fire_timer(save);
    let kv = h.expect_effect(fx::kv_set(STORE_KEY));
    h.resolve_kv_set(kv, KvResult::Ok);
    h.query(by::test_id("status")).assert_text("saved");
    h.expect_no_effects();
}

#[test]
fn body_edits_debounce_before_saving() {
    let mut h = fresh_harness();
    h.click(by::test_id("new-note"));
    let boot_timer = h.expect_effect(fx::timer()); // the NewNote structural save
    h.fire_timer(boot_timer);
    let boot_save = h.expect_effect(fx::kv_set(STORE_KEY));
    h.resolve_kv_set(boot_save, KvResult::Ok);

    h.type_text(by::test_id("editor-body"), "Grocery list");
    // Each keystroke schedules its own debounce timer. Firing the stale
    // (earlier) ones must be a silent no-op — only the last, matching
    // the current save generation, actually persists.
    let mut timers = Vec::new();
    while h.pending_effects().any(|e| matches!(e, Effect::Timer { .. })) {
        timers.push(h.expect_effect(fx::timer()));
    }
    assert!(timers.len() > 1, "each keystroke schedules its own debounce timer");
    let (last, earlier) = timers.split_last().unwrap();
    for stale in earlier {
        let Effect::Timer { after_ms, .. } = stale else { unreachable!() };
        assert!(*after_ms > 0, "body edits debounce, unlike structural changes");
        h.fire_timer(stale.clone());
    }
    h.expect_no_effects(); // superseded generations produce nothing
    h.fire_timer(last.clone());
    let kv = h.expect_effect(fx::kv_set(STORE_KEY));
    let Effect::KvSet { value, .. } = &kv else { unreachable!() };
    let text = String::from_utf8(value.clone()).unwrap();
    assert!(text.contains("Grocery list"));
    h.resolve_kv_set(kv, KvResult::Ok);
    h.expect_no_effects();
}

#[test]
fn a_save_completing_while_dirty_again_triggers_exactly_one_more_save() {
    let mut h = fresh_harness();
    h.click(by::test_id("new-note"));
    let t1 = h.expect_effect(fx::timer());
    h.fire_timer(t1);
    let first_save = h.expect_effect(fx::kv_set(STORE_KEY));

    // A second structural edit lands while the first save is in flight
    // (the M6 dialog flow, driven as actions).
    h.dispatch(Action::OpenNewFolderDialog);
    h.dispatch(Action::DialogNameChanged("Work".into()));
    h.dispatch(Action::DialogConfirm);
    let t2 = h.expect_effect(fx::timer());
    h.fire_timer(t2);
    h.expect_no_effects(); // save_in_flight — this one becomes pending, not a new KvSet

    h.resolve_kv_set(first_save, KvResult::Ok);
    // The pending edit re-persists on another zero-delay timer once the
    // first save lands.
    let t3 = h.expect_effect(fx::timer());
    h.fire_timer(t3);
    let second_save = h.expect_effect(fx::kv_set(STORE_KEY));
    h.resolve_kv_set(second_save, KvResult::Ok);
    h.query(by::test_id("status")).assert_text("saved");
    h.expect_no_effects();
}

#[test]
fn restart_restores_notes_from_the_persisted_store() {
    let mut h = fresh_harness();
    h.click(by::test_id("new-note"));
    let t = h.expect_effect(fx::timer());
    h.fire_timer(t);
    let save = h.expect_effect(fx::kv_set(STORE_KEY));
    let Effect::KvSet { value, .. } = &save else { unreachable!() };
    let persisted = value.clone();
    h.resolve_kv_set(save, KvResult::Ok);

    let mut h2 = Harness::<Notes>::with_viewport(VIEW, 900, 600);
    let load = h2.expect_effect(fx::kv_get(STORE_KEY));
    h2.resolve_kv_get(load, Some(persisted));
    // The restored note shows up (as "Untitled" — empty body) in the list.
    assert_eq!(h2.state().notes.len(), 1);
    assert_eq!(h2.state().note_rows[0].title, "Untitled");
}

/// `Effect::SetTheme` is data like any other effect: an integration test
/// asserts on it, and the harness applies it so subsequent frames
/// render restyled.
#[test]
fn theme_toggle_emits_set_theme_and_restyles() {
    let mut h = fresh_harness();
    let before = h.frame_png();
    h.click(by::test_id("theme"));
    let effect = h.expect_effect(fx::set_theme());
    assert_eq!(effect, Effect::SetTheme { name: "libra-light".into() });
    assert_ne!(h.frame_png(), before, "frame restyles under the new theme");
    h.query(by::test_id("theme")).assert_text("libra-light");
    h.expect_no_effects();
}

/// M7 (PLAN-NATIVE-INTEGRATIONS): copying a note is an intercepted
/// `Effect::ClipboardSet` carrying the body; the hand-fed ack lands as
/// a typed outcome in status. No OS clipboard in tests.
#[test]
fn copy_note_emits_a_clipboard_effect_with_a_typed_outcome() {
    let mut h = fresh_harness();
    h.click(by::test_id("new-note"));
    let t = h.expect_effect(fx::timer());
    h.fire_timer(t);
    let save = h.expect_effect(fx::kv_set(STORE_KEY));
    h.resolve_kv_set(save, KvResult::Ok);
    h.type_text(by::test_id("editor-body"), "the body to copy");
    while h.pending_effects().any(|e| matches!(e, Effect::Timer { .. })) {
        let t = h.expect_effect(fx::timer());
        h.fire_timer(t);
    }
    while h.pending_effects().any(|e| matches!(e, Effect::KvSet { .. })) {
        let kv = h.expect_effect(fx::kv_set(STORE_KEY));
        h.resolve_kv_set(kv, KvResult::Ok);
    }

    h.blur();
    assert!(h.press_shortcut("cmd+shift+c"));
    let copy = h.expect_effect(fx::clipboard_set());
    let Effect::ClipboardSet { text, .. } = &copy else { unreachable!() };
    assert_eq!(text, "the body to copy");
    h.resolve_clipboard(copy, libra_core::ClipboardResult::Ok);
    h.query(by::test_id("status")).assert_text("copied");

    // The failure path is a typed outcome too.
    assert!(h.press_shortcut("cmd+shift+c"));
    let copy = h.expect_effect(fx::clipboard_set());
    h.resolve_clipboard(copy, libra_core::ClipboardResult::Err("denied".into()));
    h.query(by::test_id("status")).assert_text("copy failed: denied");
    h.expect_no_effects();
}
