//! Unit tests: title/snippet helpers and boot reduce.

use gloss::{journal_path, note_snippet, note_title, parse_list, reduce, Action, Gloss};

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
