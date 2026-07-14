//! Integration tests (§15.4): save flow and theme toggle, no mocks.

use libra_core::{Effect, KvResult};
use libra_test::{by, fx, Harness};
use notes::{Notes, TITLE_KEY, VIEW};

fn harness() -> Harness<Notes> {
    let mut h = Harness::<Notes>::with_viewport(VIEW, 520, 200);
    let load = h.expect_effect(fx::kv_get(TITLE_KEY));
    h.resolve_kv_get(load, None);
    h
}

#[test]
fn save_persists_the_typed_title() {
    let mut h = harness();
    h.type_text(by::test_id("title"), "ship it");
    h.click(by::test_id("save"));
    let save = h.expect_effect(fx::kv_set(TITLE_KEY));
    let Effect::KvSet { value, .. } = &save else { unreachable!() };
    assert_eq!(value, b"ship it");
    h.resolve_kv_set(save, KvResult::Ok);
    h.query(by::test_id("status")).assert_text("saved");
    h.expect_no_effects();
}

/// Effect::SetTheme is data like any other effect: an integration test
/// asserts on it, and the harness applies it so subsequent frames
/// render restyled.
#[test]
fn theme_toggle_emits_set_theme_and_restyles() {
    let mut h = harness();
    let before = h.frame_png();
    h.click(by::test_id("theme"));
    let effect = h.expect_effect(fx::set_theme());
    assert_eq!(effect, Effect::SetTheme { name: "libra-light".into() });
    assert_ne!(h.frame_png(), before, "frame restyles under the new theme");
    h.query(by::test_id("theme")).assert_text("libra-light");
    h.expect_no_effects();
}
