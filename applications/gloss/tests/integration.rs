//! Integration tests: intercepted Process effects for list/cat/apply.

use libra_test::{by, fx};

use crate::common::{harness, ok_stdout, open_first_note};

mod common;

#[test]
fn save_apply_uses_the_gloss_cli() {
    let mut h = harness();
    open_first_note(&mut h, "Atlas.md\n", "hello");
    h.type_text(by::test_id("editor-body"), "!");
    let t = h.expect_effect(fx::timer());
    h.fire_timer(t);
    let apply = h.expect_effect(fx::process_program("gloss"));
    let libra_core::Effect::Process { command, .. } = &apply else {
        panic!("expected process");
    };
    assert_eq!(command.program, "gloss");
    assert!(command.args.iter().any(|a| a == "apply"));
    assert!(command.args.iter().any(|a| a == "--patch-json"));
    assert!(command.args.iter().any(|a| a == "--journal"));
    h.resolve_process(apply, ok_stdout(""));
}

#[test]
fn undo_last_save_emits_gloss_undo() {
    let mut h = harness();
    open_first_note(&mut h, "Atlas.md\n", "hello");
    h.type_text(by::test_id("editor-body"), "!");
    let t = h.expect_effect(fx::timer());
    h.fire_timer(t);
    let apply = h.expect_effect(fx::process_program("gloss"));
    let libra_core::Effect::Process { command, .. } = &apply else {
        panic!("expected process");
    };
    assert!(command.args.iter().any(|a| a == "apply"));
    assert!(command.args.iter().any(|a| a == "--journal"));
    h.resolve_process(apply, ok_stdout(""));
    assert!(h.state().can_undo);
    h.click(by::test_id("undo"));
    let undo = h.expect_effect(fx::process_program("gloss"));
    let libra_core::Effect::Process { command, .. } = &undo else {
        panic!("expected process");
    };
    assert!(command.args.iter().any(|a| a == "undo"));
    assert!(command.args.iter().any(|a| a == "--journal"));
    h.resolve_process(undo, ok_stdout(""));
    let cat = h.expect_effect(fx::process_program("gloss"));
    h.resolve_process(cat, ok_stdout("hello"));
    assert!(h.state().status == "Undone" || h.state().status == "Atlas.md");
    assert_eq!(h.state().editor_body, "hello");
    assert!(!h.state().can_undo);
}

#[test]
fn new_note_emits_create_apply() {
    let mut h = harness();
    h.type_text(by::test_id("vault-path"), "/tmp/gloss-fixture");
    h.click(by::test_id("open-vault"));
    let kv = h.expect_effect(fx::kv_set(gloss::STORE_KEY));
    h.resolve_kv_set(kv, libra_core::KvResult::Ok);
    let list = h.expect_effect(fx::process_program("gloss"));
    h.resolve_process(list, ok_stdout(""));
    h.click(by::test_id("new-note"));
    let create = h.expect_effect(fx::process_program("gloss"));
    let libra_core::Effect::Process { command, .. } = &create else {
        panic!("expected process");
    };
    assert!(command.args.iter().any(|a| a == "apply"));
    h.resolve_process(create, ok_stdout(""));
    assert_eq!(h.state().selected_path, "10-Inbox/Untitled.md");
}
