//! Component tests (§15.3): §10.1's harness verbs in anger — type_text
//! drives the same pure editing core the shell drives, dispatching
//! onChange through the real update per keystroke.

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
fn type_text_flows_keystrokes_through_update() {
    let mut h = harness();
    h.type_text(by::test_id("title"), "Hello");
    h.query(by::test_id("echo")).assert_text("title is Hello");
    assert_eq!(h.state().title, "Hello");
    assert_eq!(h.state().status, "unsaved");
}

#[test]
fn backspace_and_word_ops_edit_the_state() {
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
fn undo_round_trips_through_state() {
    let mut h = harness();
    h.type_text(by::test_id("title"), "abc def");
    h.edit(EditOp::Undo); // word-grouped: drops "def"
    assert_eq!(h.state().title, "abc ");
    h.edit(EditOp::Redo);
    assert_eq!(h.state().title, "abc def");
}

#[test]
fn select_all_then_type_replaces() {
    let mut h = harness();
    h.type_text(by::test_id("title"), "old title");
    h.edit(EditOp::SelectAll);
    h.edit(EditOp::Insert('N'));
    assert_eq!(h.state().title, "N");
}

#[test]
fn multibyte_typing_is_safe() {
    let mut h = harness();
    h.type_text(by::test_id("title"), "héllo 🎉");
    assert_eq!(h.state().title, "héllo 🎉");
    h.edit(EditOp::Backspace);
    assert_eq!(h.state().title, "héllo ");
}
