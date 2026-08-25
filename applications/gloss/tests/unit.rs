//! Unit tests: title/snippet helpers and boot reduce.

use gloss::{
    journal_path, note_snippet, note_title, parse_list, reduce, Action, Gloss, IoKind, Pending,
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
    assert_eq!(s.empty_message, "Open a vault folder to begin");
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
