//! Unit tests: title/snippet helpers and boot reduce.

use gloss::{
    default_daily_note_path, file_stem, journal_path, note_snippet, note_title, parse_list,
    project_file_rows, reduce, resolve_daily_note, resolve_wikilink, vault_name, Action, Gloss,
    IoKind, Pending,
};
use libra_core::{ProcessResult, ProcessStatus};

fn boot() -> Gloss {
    let (mut s, fx) = Gloss::init();
    assert_eq!(fx.len(), 1);
    reduce(&mut s, Action::WorkspaceLoaded(None));
    s
}

#[test]
fn fresh_state_asks_for_a_vault() {
    let s = boot();
    assert_eq!(s.empty_message, "Browse a vault folder to begin");
    assert_eq!(s.vault_name, "No vault");
    assert!(!s.has_selected);
    assert!(s.note_rows.is_empty());
}

#[test]
fn title_prefers_heading() {
    assert_eq!(note_title("n.md", "# Hello\n\nbody"), "Hello");
}

#[test]
fn title_uses_frontmatter_when_present() {
    assert_eq!(
        note_title("n.md", "---\ntitle: Atlas\n---\n\nbody"),
        "Atlas"
    );
}

#[test]
fn snippet_skips_frontmatter() {
    let s = note_snippet("---\ntitle: A\n---\n\nfirst line\nsecond");
    assert!(s.contains("first line"));
}

#[test]
fn parse_list_drops_blank_lines() {
    assert_eq!(parse_list(b"a.md\n\nb.md\n"), vec!["a.md", "b.md"]);
}

#[test]
fn journal_lives_under_gloss_dir() {
    assert_eq!(
        journal_path("/tmp/vault/"),
        "/tmp/vault/.gloss/journal.json"
    );
}

#[test]
fn query_filters_rows() {
    let mut s = boot();
    s.note_paths = vec!["Atlas.md".into(), "10-Inbox/Welcome.md".into()];
    reduce(&mut s, Action::QueryChanged("inbox".into()));
    assert_eq!(s.note_rows.len(), 1);
    assert_eq!(s.note_rows[0].id, "10-Inbox/Welcome.md");
}

fn ok_proc(stdout: &str) -> ProcessResult {
    ProcessResult {
        status: ProcessStatus::Exited(0),
        stdout: stdout.as_bytes().to_vec(),
        stderr: vec![],
        finished_at_unix_ms: 1,
    }
}

fn editing(path: &str, body: &str) -> Gloss {
    let mut s = boot();
    s.vault_path = "/tmp/v".into();
    s.opened_root = "/tmp/v".into();
    s.selected_path = path.into();
    s.editor_body = body.into();
    s.loaded_body = body.into();
    s
}

#[test]
fn select_while_apply_in_flight_queues_cat() {
    let mut s = editing("Atlas.md", "hello!");
    s.dirty = true;
    s.save_pending = true;
    s.io = IoKind::Apply;
    s.in_flight_body = "hello!".into();
    reduce(&mut s, Action::SelectNote("Welcome.md".into()));
    assert_eq!(s.io, IoKind::Apply);
    assert_eq!(s.pending, Pending::Cat("Welcome.md".into()));
    assert_eq!(s.selected_path, "Atlas.md");
    assert_eq!(s.editor_body, "hello!");
}

#[test]
fn apply_done_starts_pending_cat() {
    let mut s = editing("Atlas.md", "hello!");
    s.io = IoKind::Apply;
    s.pending = Pending::Cat("Welcome.md".into());
    s.in_flight_body = "hello!".into();
    reduce(&mut s, Action::IoDone(ok_proc("")));
    assert_eq!(s.io, IoKind::Cat);
    assert_eq!(s.selected_path, "Welcome.md");
    assert_eq!(s.editor_body, "hello!");
    assert_eq!(s.loaded_body, "hello!");
}

#[test]
fn select_while_dirty_saves_before_cat() {
    let mut s = editing("Atlas.md", "hello!");
    s.dirty = true;
    s.save_pending = true;
    reduce(&mut s, Action::SelectNote("Welcome.md".into()));
    assert_eq!(s.io, IoKind::Apply);
    assert_eq!(s.pending, Pending::Cat("Welcome.md".into()));
    assert_eq!(s.selected_path, "Atlas.md");
}

#[test]
fn vault_name_is_the_last_segment() {
    assert_eq!(vault_name("/Users/ada/MiVault"), "MiVault");
    assert_eq!(vault_name("/tmp/v/"), "v");
    assert_eq!(vault_name(""), "No vault");
}

#[test]
fn sidebar_label_is_the_file_stem() {
    assert_eq!(file_stem("10-Inbox/Welcome.md"), "Welcome");
    assert_eq!(file_stem("Atlas.md"), "Atlas");
    let mut expanded = std::collections::HashSet::new();
    expanded.insert("10-Inbox".into());
    let rows = project_file_rows(
        &["10-Inbox/Welcome.md".into()],
        "",
        "10-Inbox/Welcome.md",
        &expanded,
        false,
    );
    assert_eq!(rows[1].id, "10-Inbox/Welcome.md");
    assert_eq!(rows[1].title, "Welcome");
    assert!(rows[1].selected);
}

#[test]
fn file_rows_project_folders_from_prefixes() {
    let rows = project_file_rows(
        &["Atlas.md".into(), "10-Inbox/Welcome.md".into()],
        "",
        "Atlas.md",
        &Default::default(),
        false,
    );
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].id, "10-Inbox");
    assert_eq!(rows[0].title, "+ 10-Inbox");
    assert_eq!(rows[0].inset, 0.0);
    assert_eq!(rows[1].id, "Atlas.md");
}

#[test]
fn expanding_a_folder_reveals_its_notes() {
    let mut expanded = std::collections::HashSet::new();
    expanded.insert("10-Inbox".into());
    let rows = project_file_rows(
        &["Atlas.md".into(), "10-Inbox/Welcome.md".into()],
        "",
        "",
        &expanded,
        false,
    );
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].id, "10-Inbox");
    assert_eq!(rows[0].title, "- 10-Inbox");
    assert_eq!(rows[1].id, "10-Inbox/Welcome.md");
    assert_eq!(rows[1].inset, 16.0);
    assert_eq!(rows[2].id, "Atlas.md");
}

#[test]
fn changing_vault_path_clears_can_undo() {
    let mut s = boot();
    s.opened_root = "/tmp/a".into();
    s.can_undo = true;
    reduce(&mut s, Action::VaultPathChanged("/tmp/b".into()));
    assert!(!s.can_undo);
}

#[test]
fn failed_apply_does_not_mark_clean() {
    let mut s = editing("Atlas.md", "hello!");
    s.io = IoKind::Apply;
    s.dirty = true;
    s.in_flight_body = "hello!".into();
    reduce(
        &mut s,
        Action::IoDone(ProcessResult {
            status: ProcessStatus::Exited(1),
            stdout: vec![],
            stderr: b"expect_hash mismatch".to_vec(),
            finished_at_unix_ms: 1,
        }),
    );
    assert_eq!(s.io, IoKind::Idle);
    assert!(s.save_pending);
    assert_eq!(s.editor_body, "hello!");
}

#[test]
fn opening_a_note_reuses_the_current_tab() {
    let mut s = editing("Atlas.md", "hello");
    s.note_paths = vec!["Atlas.md".into(), "Welcome.md".into()];
    s.open_tabs = vec!["Atlas.md".into()];
    reduce(&mut s, Action::SelectNote("Welcome.md".into()));
    assert_eq!(s.open_tabs, vec!["Welcome.md"]);
    assert_eq!(s.selected_path, "Welcome.md");
}

#[test]
fn opening_an_already_open_note_does_not_duplicate_tabs() {
    let mut s = editing("Atlas.md", "hello");
    s.note_paths = vec!["Atlas.md".into(), "Welcome.md".into()];
    s.open_tabs = vec!["Atlas.md".into(), "Welcome.md".into()];
    reduce(&mut s, Action::SelectNote("Welcome.md".into()));
    assert_eq!(s.open_tabs, vec!["Atlas.md".to_string(), "Welcome.md".to_string()]);
    assert_eq!(s.selected_path, "Welcome.md");
}

#[test]
fn new_tab_then_select_appends() {
    let mut s = editing("Atlas.md", "hello");
    s.note_paths = vec!["Atlas.md".into(), "Welcome.md".into()];
    s.open_tabs = vec!["Atlas.md".into()];
    reduce(&mut s, Action::NewTab);
    reduce(&mut s, Action::SelectNote("Welcome.md".into()));
    assert_eq!(s.open_tabs, vec!["Atlas.md".to_string(), "Welcome.md".to_string()]);
}

#[test]
fn open_in_new_tab_appends() {
    let mut s = editing("Atlas.md", "hello");
    s.note_paths = vec!["Atlas.md".into(), "Welcome.md".into()];
    s.open_tabs = vec!["Atlas.md".into()];
    reduce(&mut s, Action::OpenInNewTab("Welcome.md".into()));
    assert_eq!(s.open_tabs, vec!["Atlas.md".to_string(), "Welcome.md".to_string()]);
}

#[test]
fn close_tab_selects_the_remaining_tab() {
    let mut s = editing("Atlas.md", "hello");
    s.note_paths = vec!["Atlas.md".into(), "Welcome.md".into()];
    s.open_tabs = vec!["Atlas.md".into(), "Welcome.md".into()];
    reduce(&mut s, Action::CloseTab("Atlas.md".into()));
    assert_eq!(s.selected_path, "Welcome.md");
    assert_eq!(s.open_tabs, vec!["Welcome.md"]);
    assert_eq!(s.io, IoKind::Cat);
}

#[test]
fn close_inactive_tab_keeps_the_current_note() {
    let mut s = editing("Atlas.md", "hello");
    s.open_tabs = vec!["Atlas.md".into(), "Welcome.md".into()];
    reduce(&mut s, Action::CloseTab("Welcome.md".into()));
    assert_eq!(s.selected_path, "Atlas.md");
    assert_eq!(s.open_tabs, vec!["Atlas.md"]);
    assert_eq!(s.io, IoKind::Idle);
}

#[test]
fn folder_click_toggles_expand_without_io() {
    let mut s = boot();
    s.note_paths = vec!["10-Inbox/Welcome.md".into()];
    s.rebuild_rows();
    assert!(s.expanded.is_empty());
    reduce(&mut s, Action::SelectFile("10-Inbox".into()));
    assert!(s.expanded.contains("10-Inbox"));
    assert_eq!(s.io, IoKind::Idle);
    reduce(&mut s, Action::SelectFile("10-Inbox".into()));
    assert!(!s.expanded.contains("10-Inbox"));
}

#[test]
fn split_note_separates_frontmatter() {
    use gloss::{assemble_note, split_note};
    let full = "---\ntitle: Atlas\n---\n\nbody\n";
    let (yaml, body, has) = split_note(full);
    assert!(has);
    assert_eq!(yaml, "title: Atlas");
    assert_eq!(body, "body\n");
    assert_eq!(assemble_note(&yaml, &body), full);
}

#[test]
fn toggle_reader_mode_round_trips_full_file() {
    let mut s = editing("Atlas.md", "---\ntitle: Atlas\n---\n\nhello");
    reduce(&mut s, Action::ToggleReaderMode);
    assert!(!s.reader_mode);
    assert_eq!(s.editor_body, "---\ntitle: Atlas\n---\n\nhello");
    reduce(&mut s, Action::ToggleReaderMode);
    assert!(s.reader_mode);
    assert_eq!(s.frontmatter_yaml, "title: Atlas");
    assert_eq!(s.editor_body, "hello");
}

#[test]
fn wikilink_resolves_exact_stem_and_case() {
    let paths = vec!["Atlas.md".into(), "10-Inbox/Welcome.md".into()];
    assert_eq!(resolve_wikilink("Atlas", &paths).as_deref(), Some("Atlas.md"));
    assert_eq!(
        resolve_wikilink("Welcome", &paths).as_deref(),
        Some("10-Inbox/Welcome.md")
    );
    assert_eq!(
        resolve_wikilink("welcome#Heading", &paths).as_deref(),
        Some("10-Inbox/Welcome.md")
    );
    assert_eq!(
        resolve_wikilink("10-Inbox/Welcome.md", &paths).as_deref(),
        Some("10-Inbox/Welcome.md")
    );
    assert_eq!(resolve_wikilink("Missing", &paths), None);
}

#[test]
fn follow_wikilink_opens_the_resolved_note() {
    let mut s = editing("Atlas.md", "see [[Welcome]]");
    s.note_paths = vec!["Atlas.md".into(), "10-Inbox/Welcome.md".into()];
    reduce(&mut s, Action::FollowWikilink("Welcome".into()));
    assert_eq!(s.selected_path, "10-Inbox/Welcome.md");
    assert_eq!(s.io, IoKind::Cat);
}

#[test]
fn daily_note_paths_prefer_existing_locations() {
    let paths = vec![
        "10-Inbox/2026-08-25.md".into(),
        "Atlas.md".into(),
    ];
    assert_eq!(
        resolve_daily_note("2026-08-25", &paths).as_deref(),
        Some("10-Inbox/2026-08-25.md")
    );
    assert_eq!(default_daily_note_path("2026-08-25"), "Daily/2026-08-25.md");
}

#[test]
fn open_daily_note_creates_when_missing() {
    let mut s = editing("Atlas.md", "body");
    s.note_paths = vec!["Atlas.md".into()];
    s.daily_date_override = Some("2026-08-25".into());
    reduce(&mut s, Action::OpenDailyNote);
    assert_eq!(s.selected_path, "Daily/2026-08-25.md");
    assert_eq!(s.io, IoKind::Apply);
}

#[test]
fn mention_pick_inserts_wikilink() {
    let mut s = editing("Atlas.md", "see @Wel");
    s.note_paths = vec!["Atlas.md".into(), "10-Inbox/Welcome.md".into()];
    reduce(&mut s, Action::BodyChanged("see @Wel".into()));
    assert!(s.show_mention);
    assert_eq!(s.mention_query, "Wel");
    reduce(
        &mut s,
        Action::MentionPick("10-Inbox/Welcome.md".into()),
    );
    assert!(!s.show_mention);
    assert_eq!(s.editor_body, "see [[Welcome]]");
}

#[test]
fn dismiss_closes_command_palette() {
    let mut s = editing("Atlas.md", "body");
    s.note_paths = vec!["Atlas.md".into()];
    s.show_command_palette = true;
    reduce(&mut s, Action::Dismiss);
    assert!(!s.show_command_palette);
}

#[test]
fn rules_load_populates_settings_excerpt() {
    let mut s = editing("Atlas.md", "body");
    s.vault_path = "/tmp/v".into();
    s.io = IoKind::Rules;
    reduce(
        &mut s,
        Action::IoDone(ok_proc("# Vault rules\n\nBe concise.\n")),
    );
    assert!(s.rules_loaded);
    assert!(s.has_rules);
    assert!(s.rules_excerpt.contains("Be concise"));
}

#[test]
fn shortcuts_table_is_valid() {
    let shortcuts = Gloss::shortcuts();
    assert_eq!(shortcuts.len(), 7);
    assert!(
        shortcuts
            .iter()
            .any(|s| s.chord == "cmd+k" && s.action == "openCommandPalette")
    );
}
