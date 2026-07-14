//! Notes tier suite — §10.1's harness verbs in anger: type_text drives
//! the same pure editing core the shell drives, dispatching onChange
//! through the real update per keystroke.

use libra_core::{App, KvResult};
use libra_test::{by, cmd, Harness};
use libra_text::EditOp;
use notes::{Msg, Notes, TITLE_KEY, VIEW};

fn harness() -> Harness<Notes> {
    let mut h = Harness::<Notes>::with_viewport(VIEW, 520, 200);
    let load = h.expect_cmd(cmd::kv_get(TITLE_KEY));
    h.resolve_kv_get(load, None);
    h
}

// ---------------------------------------------------------------------
// Tier 1 — pure update
// ---------------------------------------------------------------------

#[test]
fn tier1_title_changed_is_an_event_not_a_setter_chain() {
    let (mut m, _) = Notes::init();
    assert!(m.update(Msg::TitleChanged("draft".into())).is_empty());
    assert_eq!((m.title.as_str(), m.status.as_str()), ("draft", "unsaved"));
    let cmds = m.update(Msg::Save);
    assert_eq!(cmds.len(), 1);
    m.update(Msg::Saved(KvResult::Ok));
    assert_eq!(m.status, "saved");
}

// ---------------------------------------------------------------------
// Tier 2 — typing through the real pipeline (§10.1 harness verbs)
// ---------------------------------------------------------------------

#[test]
fn tier2_type_text_flows_keystrokes_through_update() {
    let mut h = harness();
    h.type_text(by::test_id("title"), "Hello");
    h.query(by::test_id("echo")).assert_text("title is Hello");
    assert_eq!(h.model().title, "Hello");
    assert_eq!(h.model().status, "unsaved");
}

#[test]
fn tier2_backspace_and_word_ops_edit_the_model() {
    let mut h = harness();
    h.type_text(by::test_id("title"), "hello world");
    h.edit(EditOp::Backspace);
    h.query(by::test_id("echo")).assert_text("title is hello worl");
    // Select the last word and replace it:
    h.edit(EditOp::WordLeft(true));
    h.edit(EditOp::Insert('W'));
    h.query(by::test_id("echo")).assert_text("title is hello W");
}

#[test]
fn tier2_undo_round_trips_through_the_model() {
    let mut h = harness();
    h.type_text(by::test_id("title"), "abc def");
    h.edit(EditOp::Undo); // word-grouped: drops "def"
    assert_eq!(h.model().title, "abc ");
    h.edit(EditOp::Redo);
    assert_eq!(h.model().title, "abc def");
}

#[test]
fn tier2_select_all_then_type_replaces() {
    let mut h = harness();
    h.type_text(by::test_id("title"), "old title");
    h.edit(EditOp::SelectAll);
    h.edit(EditOp::Insert('N'));
    assert_eq!(h.model().title, "N");
}

#[test]
fn tier2_multibyte_typing_is_safe() {
    let mut h = harness();
    h.type_text(by::test_id("title"), "héllo 🎉");
    assert_eq!(h.model().title, "héllo 🎉");
    h.edit(EditOp::Backspace);
    assert_eq!(h.model().title, "héllo ");
}

// ---------------------------------------------------------------------
// Tier 3 — save flow, no mocks
// ---------------------------------------------------------------------

#[test]
fn tier3_save_persists_the_typed_title() {
    let mut h = harness();
    h.type_text(by::test_id("title"), "ship it");
    h.click(by::test_id("save"));
    let save = h.expect_cmd(cmd::kv_set(TITLE_KEY));
    let libra_core::Cmd::KvSet { value, .. } = &save else { unreachable!() };
    assert_eq!(value, b"ship it");
    h.resolve_kv_set(save, KvResult::Ok);
    h.query(by::test_id("status")).assert_text("saved");
    h.expect_no_cmds();
}

// ---------------------------------------------------------------------
// Tier 4 — caret/selection render byte-exactly
// ---------------------------------------------------------------------

#[test]
fn tier4_snapshot_focused_input_with_selection() {
    let mut h = harness();
    h.type_text(by::test_id("title"), "select me");
    h.edit(EditOp::Home(false));
    h.edit(EditOp::WordRight(true)); // "select" selected, caret after it
    h.assert_snapshot("notes_selection");
}

#[test]
fn tier4_snapshot_placeholder_unfocused() {
    let mut h = harness();
    h.blur();
    h.assert_snapshot("notes_placeholder");
}

/// §11: snapshot identity includes the theme — same model, different
/// theme, different (stable) bytes.
#[test]
fn tier4_snapshot_light_theme() {
    let mut h = harness().theme("libra-light");
    h.type_text(by::test_id("title"), "light mode");
    h.assert_snapshot("notes_light");
}

/// Cmd::SetTheme is data like any other effect: Tier-3 asserts on it,
/// and the harness applies it so subsequent frames render restyled.
#[test]
fn tier3_theme_toggle_emits_set_theme_and_restyles() {
    let mut h = harness();
    let before = h.frame_png();
    h.click(by::test_id("theme"));
    let cmd = h.expect_cmd(cmd::set_theme());
    assert_eq!(cmd, libra_core::Cmd::SetTheme { name: "libra-light".into() });
    assert_ne!(h.frame_png(), before, "frame restyles under the new theme");
    h.query(by::test_id("theme")).assert_text("libra-light");
    h.expect_no_cmds();
}

#[test]
fn tier4_light_and_dark_render_different_bytes() {
    let dark = Harness::<Notes>::with_viewport(VIEW, 520, 200).frame_png();
    let light = Harness::<Notes>::with_viewport(VIEW, 520, 200).theme("libra-light").frame_png();
    assert_ne!(dark, light, "theme must be part of the frame's identity");
}
