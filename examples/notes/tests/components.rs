//! Component tests (§15.3): harness verbs driving the real semantics
//! tree — folder/note selection, search filtering, trash flow, the
//! editor's controlled `Input` (until Textarea lands, [[PLAN-TEXTAREA]]).

use libra_core::{Effect, KvResult};
use libra_test::{by, fx, Harness};
use notes::{Action, Notes, STORE_KEY, VIEW};

fn harness() -> Harness<Notes> {
    let mut h = Harness::<Notes>::with_viewport(VIEW, 900, 600);
    let load = h.expect_effect(fx::kv_get(STORE_KEY));
    h.resolve_kv_get(load, None);
    h
}

/// Fires every pending debounce/immediate timer and resolves every
/// resulting `KvSet`, until the autosave machinery is fully settled.
fn settle_save(h: &mut Harness<Notes>) {
    loop {
        if h.pending_effects().any(|e| matches!(e, Effect::Timer { .. })) {
            let t = h.expect_effect(fx::timer());
            h.fire_timer(t);
            continue;
        }
        if h.pending_effects().any(|e| matches!(e, Effect::KvSet { .. })) {
            let kv = h.expect_effect(fx::kv_set(STORE_KEY));
            h.resolve_kv_set(kv, KvResult::Ok);
            continue;
        }
        break;
    }
}

fn add_note(h: &mut Harness<Notes>, body: &str) {
    h.click(by::test_id("new-note"));
    settle_save(h);
    if !body.is_empty() {
        h.type_text(by::test_id("editor-body"), body);
        settle_save(h);
    }
}

#[test]
fn new_note_appears_untitled_and_becomes_the_editor_body() {
    let mut h = harness();
    h.click(by::test_id("new-note"));
    assert_eq!(h.state().note_rows.len(), 1);
    assert_eq!(h.state().note_rows[0].title, "Untitled");
    assert_eq!(h.state().editor_body, "");
    settle_save(&mut h);
}

#[test]
fn typing_in_the_editor_updates_the_derived_title_live() {
    let mut h = harness();
    add_note(&mut h, "");
    h.type_text(by::test_id("editor-body"), "Hello world");
    h.query(by::test_id("folder-name")).assert_text("Notes");
    assert_eq!(h.state().note_rows[0].title, "Hello world");
    assert_eq!(h.state().editor_body, "Hello world");
    settle_save(&mut h);
}

#[test]
fn search_hides_and_shows_notes_live() {
    let mut h = harness();
    add_note(&mut h, "Alpha project notes");
    add_note(&mut h, "Beta rollout plan");
    assert_eq!(h.state().note_rows.len(), 2);

    h.type_text(by::test_id("search"), "alpha");
    assert_eq!(h.state().note_rows.len(), 1);
    assert_eq!(h.state().note_rows[0].title, "Alpha project notes");

    h.click(by::test_id("clear-search"));
    assert_eq!(h.state().note_rows.len(), 2);
}

#[test]
fn empty_state_message_differs_for_no_notes_vs_no_matches() {
    let mut h = harness();
    h.query(by::test_id("empty")).assert_text("No notes yet");
    add_note(&mut h, "Only note");
    h.type_text(by::test_id("search"), "nonexistent");
    h.query(by::test_id("empty")).assert_text("No matches");
}

#[test]
fn delete_note_moves_it_to_recently_deleted_and_it_can_be_restored() {
    let mut h = harness();
    add_note(&mut h, "Temporary");
    assert!(!h.state().has_trash);

    h.click(by::test_id("delete-note"));
    settle_save(&mut h);
    assert!(h.state().has_trash);
    assert_eq!(h.state().note_rows.len(), 0, "trashed notes leave the folder's own list");

    h.click(by::test_id("trash-folder"));
    assert_eq!(h.state().note_rows.len(), 1);
    assert!(h.state().selected_is_trashed);

    h.click(by::test_id("restore"));
    settle_save(&mut h);
    assert!(!h.state().has_trash);
}

#[test]
fn recently_deleted_row_is_absent_until_something_is_trashed() {
    let mut h = harness();
    add_note(&mut h, "Whatever");
    assert!(!h.state().has_trash);
    h.click(by::test_id("delete-note"));
    settle_save(&mut h);
    h.query(by::test_id("trash-folder")).assert_text("Recently Deleted");
}

#[test]
fn adding_a_folder_and_switching_shows_its_own_notes() {
    let mut h = harness();
    add_note(&mut h, "In Notes folder");

    h.type_text(by::test_id("new-folder"), "Work");
    h.click(by::test_id("add-folder"));
    settle_save(&mut h);

    // Switched to the new (empty) folder automatically.
    h.query(by::test_id("folder-name")).assert_text("Work");
    assert_eq!(h.state().note_rows.len(), 0);

    let notes_folder_id = h.state().folder_rows[0].id;
    h.dispatch(Action::SelectFolder(notes_folder_id.to_string()));
    assert_eq!(h.state().note_rows.len(), 1);
}

#[test]
fn interval_source_is_mounted_for_live_relative_timestamps() {
    let h = harness();
    assert!(h.has_source("interval"));
}

/// M2 tail (PLAN-NOTES-APP / PLAN-SCROLL): both list panes are scroll
/// viewports; a long note list scrolls — offset is a deterministic
/// harness input, and rows keep working after the offset applies.
#[test]
fn note_list_scrolls_when_it_overflows() {
    let mut h = harness();
    for i in 0..30 {
        add_note(&mut h, &format!("Note number {i}\nbody text"));
    }
    h.query(by::test_id("note-scroll")).assert_text("");
    h.query(by::test_id("folder-scroll")).assert_text("");

    let resting = h.frame_png();
    h.scroll("note-scroll", 200.0);
    assert_ne!(h.frame_png(), resting, "scrolled list renders different bytes");
    h.scroll("note-scroll", -200.0);
    assert_eq!(h.frame_png(), resting, "scrolling back restores identical bytes");
}
