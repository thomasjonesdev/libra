//! Snapshot tests (§15.5): caret/selection render byte-exactly. Names
//! unchanged from before the test reorg.

use libra_test::{by, fx, Harness};
use libra_text::EditOp;
use notes::{Notes, TITLE_KEY, VIEW};

fn harness() -> Harness<Notes> {
    let mut h = Harness::<Notes>::with_viewport(VIEW, 520, 200);
    let load = h.expect_effect(fx::kv_get(TITLE_KEY));
    h.resolve_kv_get(load, None);
    h
}

#[test]
fn snapshot_focused_input_with_selection() {
    let mut h = harness();
    h.type_text(by::test_id("title"), "select me");
    h.edit(EditOp::Home(false));
    h.edit(EditOp::WordRight(true)); // "select" selected, caret after it
    h.assert_snapshot("notes_selection");
}

#[test]
fn snapshot_placeholder_unfocused() {
    let mut h = harness();
    h.blur();
    h.assert_snapshot("notes_placeholder");
}

/// §11: snapshot identity includes the theme — same state, different
/// theme, different (stable) bytes.
#[test]
fn snapshot_light_theme() {
    let mut h = harness().theme("libra-light");
    h.type_text(by::test_id("title"), "light mode");
    h.assert_snapshot("notes_light");
}

#[test]
fn light_and_dark_render_different_bytes() {
    let dark = Harness::<Notes>::with_viewport(VIEW, 520, 200).frame_png();
    let light = Harness::<Notes>::with_viewport(VIEW, 520, 200).theme("libra-light").frame_png();
    assert_ne!(dark, light, "theme must be part of the frame's identity");
}
