//! Component tests: harness verbs against the semantics tree.

use libra_core::Effect;
use libra_test::{by, fx};

use crate::common::{harness, ok_stdout, open_and_list, open_first_note, resolve_rules};

mod common;

#[test]
fn empty_vault_prompt_is_visible() {
    let mut h = harness();
    h.query(by::test_id("empty"))
        .assert_text("Browse a vault folder to begin");
    h.query(by::test_id("vault-picker"));
    h.click(by::test_id("vault-picker"));
    h.query(by::test_id("browse-vault")).assert_text("Browse");
    h.query(by::test_id("vault-name")).assert_text("No vault");
}

#[test]
fn opening_a_vault_lists_notes() {
    let mut h = harness();
    open_and_list(&mut h, "Atlas.md\n10-Inbox/Welcome.md\n");
    let cat = h.expect_effect(fx::process_program("gloss"));
    h.resolve_process(cat, ok_stdout("# Atlas\n\nindex"));
    resolve_rules(&mut h, "");
    assert_eq!(h.state().note_rows.len(), 2);
    assert_eq!(h.state().file_rows.len(), 2);
    assert_eq!(h.state().selected_path, "Atlas.md");
    assert_eq!(h.state().vault_name, "gloss-fixture");
    h.query(by::test_id("note-title")).assert_text("Atlas");
    h.query(by::test_id("vault-name")).assert_text("gloss-fixture");
}

#[test]
fn typing_in_the_editor_marks_dirty_and_schedules_save() {
    let mut h = harness();
    open_first_note(&mut h, "Atlas.md\n", "# Atlas\n\nindex");
    h.type_text(by::test_id("reader-body"), " more");
    assert!(h
        .pending_effects()
        .any(|e| matches!(e, Effect::Timer { .. })));
    let t = h.expect_effect(fx::timer());
    h.fire_timer(t);
    let apply = h.expect_effect(fx::process_program("gloss"));
    h.resolve_process(apply, ok_stdout(""));
    assert_eq!(h.state().status, "Saved");
    h.query(by::test_id("toast")).assert_text("Saved");
}
