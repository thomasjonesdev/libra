//! Unit tests (§15.2): the reducer as pure state transitions. Todos'
//! whole model — add, complete/uncomplete by row line, remove, clear —
//! is exercised here without any view or harness.

use libra_core::App;
use todos::{reduce, Action, Todos};

/// `App::init` is `app!`'s projected default — no hand-written init
/// exists; the `#[project]` hook has already run, so `summary` is
/// "0 of 0 done" (not "") from frame zero.
fn app() -> Todos {
    Todos::init().0
}

#[test]
fn init_summary_is_zero_of_zero() {
    let s = app();
    assert_eq!(s.summary, "0 of 0 done");
    assert!(s.active.is_empty() && s.done.is_empty());
}

#[test]
fn add_pushes_an_active_row_and_clears_the_draft() {
    let mut s = app();
    reduce(&mut s, Action::DraftChanged("Buy milk".into()));
    reduce(&mut s, Action::Add);
    assert_eq!(s.active, vec!["0. Buy milk"]);
    assert_eq!(s.draft, "");
    assert_eq!(s.summary, "0 of 1 done");
}

#[test]
fn add_trims_and_ignores_empty_drafts() {
    let mut s = app();
    reduce(&mut s, Action::DraftChanged("   ".into()));
    reduce(&mut s, Action::Add);
    assert!(s.items.is_empty(), "whitespace-only draft is a no-op");
    reduce(&mut s, Action::DraftChanged("  padded  ".into()));
    reduce(&mut s, Action::Add);
    assert_eq!(s.active, vec!["0. padded"]);
}

#[test]
fn complete_moves_the_row_to_done_and_back() {
    let mut s = app();
    for t in ["a", "b"] {
        reduce(&mut s, Action::DraftChanged(t.into()));
        reduce(&mut s, Action::Add);
    }
    // "0. a" and "1. b" are active.
    reduce(&mut s, Action::Complete("0. a".into()));
    assert_eq!(s.active, vec!["1. b"]);
    assert_eq!(s.done, vec!["0. a"]);
    assert_eq!(s.summary, "1 of 2 done");
    // The line is stable across the toggle, so uncomplete reverses it.
    reduce(&mut s, Action::Uncomplete("0. a".into()));
    assert_eq!(s.done, Vec::<String>::new());
    assert_eq!(s.active, vec!["0. a", "1. b"]);
}

#[test]
fn ids_keep_rows_distinct_even_with_identical_text() {
    let mut s = app();
    for _ in 0..2 {
        reduce(&mut s, Action::DraftChanged("dup".into()));
        reduce(&mut s, Action::Add);
    }
    // Same text, different ids → distinct lines → completing one leaves
    // the other active.
    reduce(&mut s, Action::Complete("0. dup".into()));
    assert_eq!(s.active, vec!["1. dup"]);
    assert_eq!(s.done, vec!["0. dup"]);
}

#[test]
fn remove_drops_a_specific_row() {
    let mut s = app();
    for t in ["a", "b", "c"] {
        reduce(&mut s, Action::DraftChanged(t.into()));
        reduce(&mut s, Action::Add);
    }
    reduce(&mut s, Action::Remove("1. b".into()));
    assert_eq!(s.active, vec!["0. a", "2. c"]);
}

#[test]
fn clear_completed_drops_only_done_rows() {
    let mut s = app();
    for t in ["a", "b", "c"] {
        reduce(&mut s, Action::DraftChanged(t.into()));
        reduce(&mut s, Action::Add);
    }
    reduce(&mut s, Action::Complete("0. a".into()));
    reduce(&mut s, Action::Complete("2. c".into()));
    reduce(&mut s, Action::ClearCompleted);
    assert_eq!(s.active, vec!["1. b"]);
    assert!(s.done.is_empty());
    assert_eq!(s.summary, "0 of 1 done");
}
