//! Snapshot tests (§15.5): byte-exact PNG comparison across the three
//! panes, empty/populated states, and both shipped themes.

use libra_core::KvResult;
use libra_test::{by, fx, Harness};
use notes::{Notes, STORE_KEY, VIEW};

fn harness() -> Harness<Notes> {
    let mut h = Harness::<Notes>::with_viewport(VIEW, 900, 600);
    let load = h.expect_effect(fx::kv_get(STORE_KEY));
    h.resolve_kv_get(load, None);
    h
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

#[test]
fn snapshot_empty_state_dark() {
    let h = harness();
    h.assert_snapshot("notes_empty_dark");
}

#[test]
fn snapshot_empty_state_light() {
    let h = harness().theme("libra-light");
    h.assert_snapshot("notes_empty_light");
}

#[test]
fn snapshot_one_note_with_body() {
    let mut h = harness();
    h.click(by::test_id("new-note"));
    settle_save(&mut h);
    h.type_text(by::test_id("editor-body"), "Grocery list\nmilk, eggs, bread");
    settle_save(&mut h);
    h.assert_snapshot("notes_with_body");
}

#[test]
fn snapshot_recently_deleted_visible_after_a_delete() {
    let mut h = harness();
    h.click(by::test_id("new-note"));
    settle_save(&mut h);
    h.click(by::test_id("delete-note"));
    settle_save(&mut h);
    h.assert_snapshot("notes_recently_deleted");
}

#[test]
fn light_and_dark_render_different_bytes() {
    let dark = Harness::<Notes>::with_viewport(VIEW, 900, 600).frame_png();
    let light = Harness::<Notes>::with_viewport(VIEW, 900, 600).theme("libra-light").frame_png();
    assert_ne!(dark, light, "theme must be part of the frame's identity");
}
