//! Integration tests: intercepted Process effects for list/cat/apply.

use libra_core::{Effect, FolderPick};
use libra_test::{by, fx, Harness};

use crate::common::{enter_edit, harness, ok_stdout, open_first_note, resolve_rules};

mod common;

#[test]
fn browse_picks_a_folder_and_lists() {
    let mut h = harness();
    crate::common::open_vault_dialog(&mut h);
    h.click(by::test_id("browse-vault"));
    let pick = h.expect_effect(fx::pick_folder());
    h.resolve_folder_pick(pick, FolderPick::Picked("/tmp/gloss-fixture".into()));
    let kv = h.expect_effect(fx::kv_set(gloss::STORE_KEY));
    let Effect::KvSet { value, .. } = &kv else {
        panic!("expected kv set");
    };
    let saved = gloss::parse_workspace_bytes(value);
    assert_eq!(saved.last_path, "/tmp/gloss-fixture");
    assert_eq!(saved.vaults.len(), 1);
    h.resolve_kv_set(kv, libra_core::KvResult::Ok);
    let list = h.expect_effect(fx::process_program("gloss"));
    h.resolve_process(list, ok_stdout("Atlas.md\n"));
    let cat = h.expect_effect(fx::process_program("gloss"));
    h.resolve_process(cat, ok_stdout("# Atlas\n"));
    resolve_rules(&mut h, "");
    assert_eq!(h.state().vault_path, "/tmp/gloss-fixture");
    assert_eq!(h.state().opened_root, "/tmp/gloss-fixture");
    assert_eq!(h.state().file_rows.len(), 1);
}

#[test]
fn legacy_workspace_path_reopens_the_vault() {
    let mut h = Harness::<gloss::Gloss>::with_view_dir(gloss::VIEW, &common::view_dir(), 960, 640);
    let load = h.expect_effect(fx::kv_get(gloss::STORE_KEY));
    h.resolve_kv_get(load, Some(b"/tmp/legacy-vault".to_vec()));
    let list = h.expect_effect(fx::process_program("gloss"));
    h.resolve_process(list, ok_stdout("Atlas.md\n"));
    let cat = h.expect_effect(fx::process_program("gloss"));
    h.resolve_process(cat, ok_stdout("# Atlas\n"));
    resolve_rules(&mut h, "");
    assert_eq!(h.state().vault_path, "/tmp/legacy-vault");
    assert_eq!(h.state().workspace.vaults.len(), 1);
}

#[test]
fn browse_cancel_leaves_the_path_empty() {
    let mut h = harness();
    crate::common::open_vault_dialog(&mut h);
    h.click(by::test_id("browse-vault"));
    let pick = h.expect_effect(fx::pick_folder());
    h.resolve_folder_pick(pick, FolderPick::Cancelled);
    assert!(h.state().vault_path.is_empty());
    assert!(!h
        .pending_effects()
        .any(|e| matches!(e, libra_core::Effect::Process { .. })));
}

#[test]
fn save_apply_uses_the_gloss_cli() {
    let mut h = harness();
    open_first_note(&mut h, "Atlas.md\n", "hello");
    h.type_text(by::test_id("reader-body"), "!");
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
    h.type_text(by::test_id("reader-body"), "!");
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
    let list = h.expect_effect(fx::process_program("gloss"));
    h.resolve_process(list, ok_stdout("Atlas.md\n"));
    let rules = h.expect_effect(fx::process_program("gloss"));
    h.resolve_process(rules, ok_stdout(""));
    let cat = h.expect_effect(fx::process_program("gloss"));
    h.resolve_process(cat, ok_stdout("hello"));
    assert_eq!(h.state().editor_body, "hello");
    assert!(!h.state().can_undo);
}

#[test]
fn select_during_save_does_not_treat_apply_as_cat() {
    let mut h = harness();
    open_first_note(
        &mut h,
        "Atlas.md\n10-Inbox/Welcome.md\n",
        "# Atlas\n\nindex",
    );
    enter_edit(&mut h);
    h.type_text(by::test_id("editor-body"), "!");
    let t = h.expect_effect(fx::timer());
    h.fire_timer(t);
    let apply = h.expect_effect(fx::process_program("gloss"));
    h.click(by::text("+ 10-Inbox"));
    h.click(by::text("Welcome"));
    let extra = h
        .pending_effects()
        .filter(|e| matches!(e, libra_core::Effect::Process { .. }))
        .count();
    assert_eq!(extra, 0, "select must not spawn a second process");
    assert_eq!(h.state().io, gloss::IoKind::Apply);
    assert_eq!(
        h.state().pending,
        gloss::Pending::Cat("10-Inbox/Welcome.md".into())
    );
    assert_eq!(h.state().editor_body, "# Atlas\n\nindex!");
    h.resolve_process(apply, ok_stdout(""));
    assert_eq!(h.state().editor_body, "# Atlas\n\nindex!");
    let cat = h.expect_effect(fx::process_program("gloss"));
    h.resolve_process(cat, ok_stdout("# Welcome\n\nhi"));
    assert_eq!(h.state().selected_path, "10-Inbox/Welcome.md");
    assert_eq!(h.state().editor_body, "# Welcome\n\nhi");
}

#[test]
fn rapid_note_switching_never_renames_a_note() {
    // `title_draft` is a draft of the *selected* note. If it lagged behind the
    // selection while a cat was in flight, the next click saw draft != stem and
    // filed a rename — clicking quickly renamed notes to each other's titles.
    let mut h = harness();
    open_first_note(
        &mut h,
        "Atlas.md\n10-Inbox/Welcome.md\n20-Work/Report.md\n",
        "# Atlas\n",
    );
    h.click(by::text("+ 10-Inbox"));
    h.click(by::text("Welcome"));
    let cat = h.expect_effect(fx::process_program("gloss"));
    assert_eq!(h.state().selected_path, "10-Inbox/Welcome.md");
    assert_eq!(h.state().title_draft, "Welcome");

    // Second click lands before the first cat resolves.
    h.click(by::text("+ 20-Work"));
    h.click(by::text("Report"));
    for effect in h.pending_effects() {
        if let libra_core::Effect::Process { command, .. } = effect {
            assert!(
                !command.args.iter().any(|a| a == "rename"),
                "rapid switching spawned a rename: {:?}",
                command.args
            );
        }
    }
    assert_eq!(h.state().pending, gloss::Pending::Cat("20-Work/Report.md".into()));

    h.resolve_process(cat, ok_stdout("# Welcome\n"));
    let cat = h.expect_effect(fx::process_program("gloss"));
    h.resolve_process(cat, ok_stdout("# Report\n"));
    assert_eq!(h.state().selected_path, "20-Work/Report.md");
    assert_eq!(h.state().title_draft, "Report");
    assert_eq!(
        h.state().note_paths,
        vec!["Atlas.md", "10-Inbox/Welcome.md", "20-Work/Report.md"]
    );
}

#[test]
fn new_note_emits_create_apply() {
    let mut h = harness();
    crate::common::open_vault_dialog(&mut h);
    h.click(by::test_id("browse-vault"));
    let pick = h.expect_effect(fx::pick_folder());
    h.resolve_folder_pick(
        pick,
        libra_core::FolderPick::Picked("/tmp/gloss-fixture".into()),
    );
    let kv = h.expect_effect(fx::kv_set(gloss::STORE_KEY));
    h.resolve_kv_set(kv, libra_core::KvResult::Ok);
    let list = h.expect_effect(fx::process_program("gloss"));
    h.resolve_process(list, ok_stdout(""));
    resolve_rules(&mut h, "");
    h.click(by::test_id("new-note"));
    let create = h.expect_effect(fx::process_program("gloss"));
    let libra_core::Effect::Process { command, .. } = &create else {
        panic!("expected process");
    };
    assert!(command.args.iter().any(|a| a == "apply"));
    h.resolve_process(create, ok_stdout(""));
    assert_eq!(h.state().selected_path, "10-Inbox/Untitled.md");
}

#[test]
fn tree_collapse_hides_nested_notes() {
    let mut h = harness();
    open_first_note(&mut h, "Atlas.md\n10-Inbox/Welcome.md\n", "# Atlas\n");
    assert_eq!(h.state().file_rows.len(), 2);
    h.click(by::text("+ 10-Inbox"));
    assert_eq!(h.state().file_rows.len(), 3);
    h.click(by::text("- 10-Inbox"));
    assert_eq!(h.state().file_rows.len(), 2);
}

#[test]
fn right_click_opens_in_a_new_tab() {
    let mut h = harness();
    open_first_note(&mut h, "Atlas.md\n10-Inbox/Welcome.md\n", "# Atlas\n");
    h.click(by::text("+ 10-Inbox"));
    h.right_click(by::text("Welcome"));
    assert!(h.context_menu().is_some());
    h.click_menu("openInNewTab", Some("10-Inbox/Welcome.md"));
    let cat = h.expect_effect(fx::process_program("gloss"));
    h.resolve_process(cat, ok_stdout("# Welcome\n"));
    assert_eq!(
        h.state().open_tabs,
        vec!["Atlas.md".to_string(), "10-Inbox/Welcome.md".to_string()]
    );
}

#[test]
fn tree_click_reuses_the_current_tab() {
    let mut h = harness();
    open_first_note(&mut h, "Atlas.md\n10-Inbox/Welcome.md\n", "# Atlas\n");
    h.click(by::text("+ 10-Inbox"));
    h.click(by::text("Welcome"));
    let cat = h.expect_effect(fx::process_program("gloss"));
    h.resolve_process(cat, ok_stdout("# Welcome\n"));
    assert_eq!(h.state().open_tabs, vec!["10-Inbox/Welcome.md".to_string()]);
}

#[test]
fn switching_heavy_markdown_notes_completes() {
    let mut h = harness();
    open_first_note(&mut h, "Atlas.md\n10-Inbox/Welcome.md\n", "# Atlas\n");
    let heavy: String = (0..800)
        .map(|i| format!("line {i} with [[Note{i}|link {i}]]\n"))
        .collect();
    h.click(by::text("+ 10-Inbox"));
    h.click(by::text("Welcome"));
    let cat = h.expect_effect(fx::process_program("gloss"));
    h.resolve_process(cat, ok_stdout(&heavy));
    assert_eq!(h.state().selected_path, "10-Inbox/Welcome.md");
    h.click(by::text("Atlas"));
    let cat = h.expect_effect(fx::process_program("gloss"));
    h.resolve_process(cat, ok_stdout("# Atlas\n\nshort"));
    assert_eq!(h.state().selected_path, "Atlas.md");
    h.click(by::text("Welcome"));
    let cat = h.expect_effect(fx::process_program("gloss"));
    h.resolve_process(cat, ok_stdout(&heavy));
    assert_eq!(h.state().selected_path, "10-Inbox/Welcome.md");
}

#[test]
fn close_tab_leaves_the_other_note() {
    let mut h = harness();
    open_first_note(&mut h, "Atlas.md\n10-Inbox/Welcome.md\n", "# Atlas\n");
    h.click(by::text("+ 10-Inbox"));
    h.click(by::test_id("new-tab"));
    h.click(by::text("Welcome"));
    let cat = h.expect_effect(fx::process_program("gloss"));
    h.resolve_process(cat, ok_stdout("# Welcome\n"));
    assert_eq!(h.state().open_tabs.len(), 2);
    h.dispatch(gloss::Action::CloseTab("10-Inbox/Welcome.md".into()));
    assert_eq!(h.state().open_tabs, vec!["Atlas.md"]);
    let cat = h.expect_effect(fx::process_program("gloss"));
    h.resolve_process(cat, ok_stdout("# Atlas\n"));
    assert_eq!(h.state().selected_path, "Atlas.md");
}

#[test]
fn reader_mode_shows_body_without_frontmatter_fences() {
    let mut h = harness();
    open_first_note(
        &mut h,
        "Atlas.md\n",
        "---\ntitle: Atlas\n---\n\nThe atlas.\n\n## More\n\nbody",
    );
    assert!(h.state().has_frontmatter);
    assert_eq!(h.state().frontmatter_title, "Atlas");
    h.query(by::test_id("note-title")).assert_text("Atlas");
    h.query(by::test_id("reader-body"))
        .assert_text("The atlas.\n\n## More\n\nbody");
}

#[test]
fn raw_mode_shows_the_full_file_bytes() {
    let mut h = harness();
    open_first_note(
        &mut h,
        "Atlas.md\n",
        "# Atlas\n\nsee **bold** and [[Atlas|the atlas]]\n",
    );
    h.click(by::test_id("toggle-mode"));
    h.query(by::test_id("editor-body"))
        .assert_text("# Atlas\n\nsee **bold** and [[Atlas|the atlas]]");
}

#[test]
fn follow_wikilink_opens_the_target_note() {
    let mut h = harness();
    open_first_note(
        &mut h,
        "Atlas.md\n10-Inbox/Welcome.md\n",
        "# Atlas\n\nsee [[Welcome|the welcome note]]\n",
    );
    h.click(by::text("the welcome note"));
    let cat = h.expect_effect(fx::process_program("gloss"));
    h.resolve_process(cat, ok_stdout("# Welcome\n"));
    assert_eq!(h.state().selected_path, "10-Inbox/Welcome.md");
}

#[test]
fn mode_toggle_swaps_reader_and_raw_editors() {
    let mut h = harness();
    open_first_note(&mut h, "Atlas.md\n", "# Atlas\n\nbody");
    h.query(by::test_id("reader-body")).assert_text("body");
    h.click(by::test_id("toggle-mode"));
    h.query(by::test_id("editor-body"))
        .assert_text("# Atlas\n\nbody");
    h.click(by::test_id("toggle-mode"));
    h.query(by::test_id("reader-body")).assert_text("body");
}

#[test]
fn settings_drawer_shows_the_path_and_browse() {
    let mut h = harness();
    open_first_note(&mut h, "Atlas.md\n", "# Atlas\n");
    h.click(by::test_id("settings"));
    h.query(by::test_id("settings-vault-name"))
        .assert_text("gloss-fixture");
    h.query(by::test_id("settings-vault-path"))
        .assert_text("/tmp/gloss-fixture");
}

#[test]
fn rail_opens_vault_dialog_with_saved_choices() {
    let mut h = harness();
    open_first_note(&mut h, "Atlas.md\n", "# Atlas\n");
    h.click(by::test_id("vault-picker"));
    h.query(by::test_id("vault-dialog"));
    h.query(by::text("gloss-fixture"));
    h.query(by::test_id("browse-vault")).assert_text("Browse");
}

#[test]
fn command_palette_opens_a_note() {
    let mut h = harness();
    open_first_note(&mut h, "Atlas.md\n10-Inbox/Welcome.md\n", "# Atlas\n");
    h.dispatch(gloss::Action::OpenCommandPalette);
    h.query(by::test_id("palette-dialog"));
    h.dispatch(gloss::Action::PaletteQueryChanged("Welcome".into()));
    h.dispatch(gloss::Action::PalettePick("10-Inbox/Welcome.md".into()));
    let cat = h.expect_effect(fx::process_program("gloss"));
    h.resolve_process(cat, ok_stdout("# Welcome\n"));
    assert_eq!(h.state().selected_path, "10-Inbox/Welcome.md");
}

#[test]
fn command_palette_shortcut_opens_dialog() {
    let mut h = harness();
    open_first_note(&mut h, "Atlas.md\n", "# Atlas\n");
    assert!(h.press_shortcut("cmd+k"));
    h.query(by::test_id("palette-dialog"));
    assert!(h.press_shortcut("escape"));
    assert!(!h.state().show_command_palette);
}

#[test]
fn mention_dialog_opens_while_typing() {
    let mut h = harness();
    open_first_note(&mut h, "Atlas.md\n10-Inbox/Welcome.md\n", "# Atlas\n\n");
    h.type_text(by::test_id("reader-body"), "@Wel");
    h.query(by::test_id("mention-dialog"));
    h.dispatch(gloss::Action::MentionPick("10-Inbox/Welcome.md".into()));
    assert_eq!(h.state().editor_body, "[[Welcome]]");
}

#[test]
fn graph_drawer_lists_backlinks() {
    let mut h = harness();
    open_first_note(
        &mut h,
        "Atlas.md\n10-Inbox/Welcome.md\n",
        "# Atlas\n\nsee [[Welcome]]\n",
    );
    h.dispatch(gloss::Action::OpenGraph);
    h.query(by::test_id("graph-drawer"));
    h.query(by::text("Welcome"));
    let scan = h.expect_effect(fx::process_program("gloss"));
    h.resolve_process(scan, ok_stdout("10-Inbox/Welcome.md\n"));
    assert_eq!(h.state().backlink_rows.len(), 1);
}
