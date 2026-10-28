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
    assert_eq!(h.state().file_rows.len(), 2);
    assert_eq!(h.state().selected_path, "Atlas.md");
    assert_eq!(h.state().vault_name, "gloss-fixture");
    h.query(by::test_id("note-title")).assert_text("Atlas");
    h.query(by::test_id("vault-name")).assert_text("gloss-fixture");
}

#[test]
fn renaming_the_title_updates_the_sidebar() {
    let mut h = harness();
    open_first_note(&mut h, "Atlas.md\n", "# Atlas\n\nindex");
    h.dispatch(gloss::Action::TitleChanged("Maps".into()));
    h.click(by::test_id("note-title"));
    h.press_enter();
    assert_eq!(h.state().io, gloss::IoKind::Rename);
    let rename = h.expect_effect(fx::process_program("gloss"));
    h.resolve_process(rename, ok_stdout(""));
    assert_eq!(h.state().selected_path, "Maps.md");
    assert_eq!(h.state().title_draft, "Maps");
    assert!(h
        .state()
        .file_rows
        .iter()
        .any(|r| r.id == "Maps.md" && r.title == "Maps"));
    h.query(by::test_id("note-title")).assert_text("Maps");
}

#[test]
fn blur_commits_a_pending_title_rename() {
    let mut h = harness();
    open_first_note(&mut h, "Atlas.md\n", "# Atlas\n\nindex");
    h.dispatch(gloss::Action::TitleChanged("Maps".into()));
    h.click(by::test_id("note-title"));
    h.click(by::test_id("reader-body"));
    let rename = h.expect_effect(fx::process_program("gloss"));
    h.resolve_process(rename, ok_stdout(""));
    assert_eq!(h.state().selected_path, "Maps.md");
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
    h.query(by::test_id("save-icon")).assert_text("loader");
    let apply = h.expect_effect(fx::process_program("gloss"));
    h.resolve_process(apply, ok_stdout(""));
    assert_eq!(h.state().status, "Saved");
    h.query(by::test_id("save-icon")).assert_text("check");
}

#[test]
fn typing_during_save_keeps_the_newer_editor_text() {
    let mut h = harness();
    open_first_note(&mut h, "Atlas.md\n", "# Atlas\n\nindex");
    h.type_text(by::test_id("reader-body"), " more");
    let t = h.expect_effect(fx::timer());
    h.fire_timer(t);
    let apply = h.expect_effect(fx::process_program("gloss"));
    h.type_text(by::test_id("reader-body"), "!");
    assert_eq!(h.state().editor_body, "index more!");
    h.resolve_process(apply, ok_stdout(""));
    assert_eq!(h.state().editor_body, "index more!");
    assert!(h.state().dirty);
    let apply2 = h.expect_effect(fx::process_program("gloss"));
    h.resolve_process(apply2, ok_stdout(""));
    assert_eq!(h.state().editor_body, "index more!");
    assert!(!h.state().dirty);
    h.query(by::test_id("save-icon")).assert_text("check");
}

fn has_test_id(h: &libra_test::Harness<gloss::Gloss>, id: &str) -> bool {
    h.laid()
        .items
        .iter()
        .any(|item| item.semantics.test_id.as_deref() == Some(id))
}

#[test]
fn toggling_the_sidebar_unmounts_the_list_pane() {
    let mut h = harness();
    open_first_note(&mut h, "Atlas.md\n", "# Atlas\n\nindex");
    assert!(has_test_id(&h, "list-split"));
    assert!(has_test_id(&h, "note-scroll"));
    h.click(by::test_id("sidebar-toggle"));
    assert!(!h.state().sidebar_open);
    assert!(!has_test_id(&h, "list-split"));
    assert!(!has_test_id(&h, "note-scroll"));
    assert!(!has_test_id(&h, "sidebar-files"));
    assert!(has_test_id(&h, "list-toolbar"));
    h.query(by::test_id("sidebar-toggle"));
    h.query(by::test_id("note-title")).assert_text("Atlas");
    h.click(by::test_id("sidebar-toggle"));
    assert!(h.state().sidebar_open);
    assert!(has_test_id(&h, "list-split"));
    assert!(has_test_id(&h, "note-scroll"));
}
