//! Unit tests (§15.2): pure `reduce` — derivation, soft delete, folder
//! rules, single-flight autosave, store migration. No Harness; these
//! exercise the model directly the way the actions/`#[project]` hook
//! see it.

use libra_core::{Effect, Instant, KvResult};
use notes::{reduce, Action, Notes, MAX_FOLDERS, MAX_NOTES};

fn boot() -> Notes {
    let (mut s, fx) = Notes::init();
    assert_eq!(fx.len(), 1, "boot issues exactly one KvGet");
    reduce(&mut s, Action::StoreLoaded(None)); // fresh install
    s
}

#[test]
fn fresh_install_starts_with_one_folder_and_no_notes() {
    let s = boot();
    assert_eq!(s.folder_rows.len(), 1);
    assert_eq!(s.folder_rows[0].name, "Notes");
    assert_eq!(s.note_rows.len(), 0);
    assert_eq!(s.empty_message, "No notes yet");
    assert!(!s.has_trash);
}

#[test]
fn title_derives_from_first_nonempty_line_and_snippet_from_the_rest() {
    let mut s = boot();
    reduce(&mut s, Action::NewNote);
    reduce(&mut s, Action::BodyChanged("\n\nShopping list\nmilk\neggs\nbread".into()));
    assert_eq!(s.note_rows.len(), 1);
    assert_eq!(s.note_rows[0].title, "Shopping list");
    assert_eq!(s.note_rows[0].snippet, "milk eggs bread");
}

#[test]
fn blank_note_titles_as_untitled() {
    let mut s = boot();
    reduce(&mut s, Action::NewNote);
    assert_eq!(s.note_rows[0].title, "Untitled");
    assert_eq!(s.note_rows[0].snippet, "");
}

#[test]
fn word_count_and_relative_time_are_in_the_meta_line() {
    let mut s = boot();
    reduce(&mut s, Action::Tick(Instant::at(0.0)));
    reduce(&mut s, Action::NewNote);
    reduce(&mut s, Action::BodyChanged("one two three".into()));
    assert!(s.note_rows[0].meta.contains("3 words"), "meta: {}", s.note_rows[0].meta);
    // Ninety minutes later, the same note reads "ago" in hours.
    reduce(&mut s, Action::Tick(Instant::at(90.0 * 60_000.0)));
    assert!(s.note_rows[0].meta.contains("1h ago"), "meta: {}", s.note_rows[0].meta);
}

#[test]
fn snippet_truncation_is_char_safe_on_multibyte_text() {
    let mut s = boot();
    reduce(&mut s, Action::NewNote);
    let long_line = "é".repeat(200);
    reduce(&mut s, Action::BodyChanged(format!("Title\n{long_line}")));
    // Must not panic slicing mid-codepoint, and must end with the ellipsis marker.
    assert!(s.note_rows[0].snippet.ends_with('…'));
}

#[test]
fn delete_note_soft_deletes_into_recently_deleted() {
    let mut s = boot();
    reduce(&mut s, Action::NewNote);
    reduce(&mut s, Action::BodyChanged("keep me".into()));
    assert_eq!(s.note_rows.len(), 1);
    reduce(&mut s, Action::DeleteNote);
    assert_eq!(s.note_rows.len(), 0, "trashed notes leave the folder view");
    assert!(s.has_trash);
    assert_eq!(s.trash_count, 1);
    // Actual removal only happens on permanent delete.
    assert_eq!(s.notes.len(), 1);
}

#[test]
fn recently_deleted_only_shows_up_once_something_is_trashed() {
    let mut s = boot();
    assert!(!s.has_trash);
    reduce(&mut s, Action::NewNote);
    reduce(&mut s, Action::DeleteNote);
    assert!(s.has_trash);
}

#[test]
fn restore_note_returns_it_to_its_folder() {
    let mut s = boot();
    reduce(&mut s, Action::NewNote);
    reduce(&mut s, Action::BodyChanged("restorable".into()));
    reduce(&mut s, Action::DeleteNote);
    reduce(&mut s, Action::OpenRecentlyDeleted);
    assert_eq!(s.note_rows.len(), 1);
    let id = s.note_rows[0].id;
    reduce(&mut s, Action::SelectNote(id.to_string()));
    reduce(&mut s, Action::RestoreNote);
    assert!(!s.has_trash);
    assert_eq!(s.selected_folder, 1);
    assert_eq!(s.note_rows.len(), 1);
}

#[test]
fn permanent_delete_only_removes_an_already_trashed_note() {
    let mut s = boot();
    reduce(&mut s, Action::NewNote);
    // Not trashed yet: permanent delete is a no-op.
    assert!(reduce(&mut s, Action::DeleteNotePermanently).is_empty());
    assert_eq!(s.notes.len(), 1);
    reduce(&mut s, Action::DeleteNote);
    reduce(&mut s, Action::OpenRecentlyDeleted);
    let id = s.note_rows[0].id;
    reduce(&mut s, Action::SelectNote(id.to_string()));
    reduce(&mut s, Action::DeleteNotePermanently);
    assert_eq!(s.notes.len(), 0);
    assert!(!s.has_trash);
}

#[test]
fn trashed_notes_are_read_only() {
    let mut s = boot();
    reduce(&mut s, Action::NewNote);
    reduce(&mut s, Action::BodyChanged("original".into()));
    reduce(&mut s, Action::DeleteNote);
    reduce(&mut s, Action::OpenRecentlyDeleted);
    let id = s.note_rows[0].id;
    reduce(&mut s, Action::SelectNote(id.to_string()));
    let fx = reduce(&mut s, Action::BodyChanged("tampered".into()));
    assert!(fx.is_empty(), "editing a trashed note is a no-op");
    let note = s.notes.iter().find(|n| n.id == id).unwrap();
    assert_eq!(note.body, "original");
}

#[test]
fn search_filters_by_title_and_body_case_insensitively() {
    let mut s = boot();
    reduce(&mut s, Action::NewNote);
    reduce(&mut s, Action::BodyChanged("Grocery Run\nneed MILK and eggs".into()));
    reduce(&mut s, Action::NewNote);
    reduce(&mut s, Action::BodyChanged("Trip Planning\npassport, tickets".into()));
    assert_eq!(s.note_rows.len(), 2);

    reduce(&mut s, Action::QueryChanged("milk".into()));
    assert_eq!(s.note_rows.len(), 1);
    assert_eq!(s.note_rows[0].title, "Grocery Run");

    reduce(&mut s, Action::QueryChanged("zzz-no-match".into()));
    assert_eq!(s.note_rows.len(), 0);
    assert_eq!(s.empty_message, "No matches");

    reduce(&mut s, Action::ClearQuery);
    assert_eq!(s.note_rows.len(), 2);
}

#[test]
fn folder_rows_track_live_note_counts_excluding_trash() {
    let mut s = boot();
    reduce(&mut s, Action::NewNote);
    reduce(&mut s, Action::NewNote);
    assert_eq!(s.folder_rows[0].count, 2);
    reduce(&mut s, Action::DeleteNote);
    assert_eq!(s.folder_rows[0].count, 1);
}

#[test]
fn add_folder_switches_selection_and_rejects_duplicates_or_blank_names() {
    let mut s = boot();
    let fx = reduce(&mut s, Action::AddFolder);
    assert!(fx.is_empty(), "blank name is rejected");
    assert_eq!(s.folders.len(), 1);

    reduce(&mut s, Action::NewFolderNameChanged("Work".into()));
    reduce(&mut s, Action::AddFolder);
    assert_eq!(s.folders.len(), 2);
    assert_eq!(s.selected_folder, 2);

    reduce(&mut s, Action::NewFolderNameChanged("work".into())); // case-insensitive dup
    reduce(&mut s, Action::AddFolder);
    assert_eq!(s.folders.len(), 2, "duplicate folder name rejected");
}

#[test]
fn folder_cap_is_enforced() {
    let mut s = boot();
    for i in 1..MAX_FOLDERS {
        reduce(&mut s, Action::NewFolderNameChanged(format!("Folder {i}")));
        reduce(&mut s, Action::AddFolder);
    }
    assert_eq!(s.folders.len(), MAX_FOLDERS);
    reduce(&mut s, Action::NewFolderNameChanged("One too many".into()));
    let fx = reduce(&mut s, Action::AddFolder);
    assert!(fx.is_empty());
    assert_eq!(s.folders.len(), MAX_FOLDERS);
}

#[test]
fn note_cap_is_enforced() {
    let mut s = boot();
    for _ in 0..MAX_NOTES {
        reduce(&mut s, Action::NewNote);
    }
    assert_eq!(s.notes.len(), MAX_NOTES);
    let fx = reduce(&mut s, Action::NewNote);
    assert!(fx.is_empty());
    assert_eq!(s.notes.len(), MAX_NOTES);
    assert_eq!(s.status, "note limit reached (48)");
}

#[test]
fn deleting_a_folder_trashes_its_notes_and_cannot_remove_the_last_folder() {
    let mut s = boot();
    reduce(&mut s, Action::NewNote);
    assert!(reduce(&mut s, Action::DeleteFolder).is_empty(), "last folder can't be deleted");
    assert_eq!(s.folders.len(), 1);

    reduce(&mut s, Action::NewFolderNameChanged("Work".into()));
    reduce(&mut s, Action::AddFolder); // now on folder 2, empty
    reduce(&mut s, Action::SelectFolder("1".into()));
    reduce(&mut s, Action::DeleteFolder);
    assert_eq!(s.folders.len(), 1);
    assert!(s.has_trash, "the deleted folder's note was trashed, not destroyed");
}

#[test]
fn autosave_debounces_and_a_superseded_generation_is_a_no_op() {
    let mut s = boot();
    reduce(&mut s, Action::NewNote);
    let fx1 = reduce(&mut s, Action::BodyChanged("v1".into()));
    let Some(Effect::Timer { action: Action::Autosave(gen1), .. }) = fx1.into_iter().next() else {
        panic!("expected a debounce timer");
    };
    let fx2 = reduce(&mut s, Action::BodyChanged("v2".into()));
    let Some(Effect::Timer { action: Action::Autosave(gen2), .. }) = fx2.into_iter().next() else {
        panic!("expected a debounce timer");
    };
    assert_ne!(gen1, gen2);

    // The stale gen1 timer fires after gen2 already superseded it: no-op.
    assert!(reduce(&mut s, Action::Autosave(gen1)).is_empty());
    assert!(!s.save_in_flight);

    // The current gen fires for real.
    let fx = reduce(&mut s, Action::Autosave(gen2));
    assert_eq!(fx.len(), 1);
    assert!(s.save_in_flight);
}

#[test]
fn exactly_one_save_in_flight_pending_edit_repersists_after_completion() {
    let mut s = boot();
    reduce(&mut s, Action::NewNote);
    let Some(Effect::Timer { action: Action::Autosave(gen1), .. }) =
        reduce(&mut s, Action::BodyChanged("v1".into())).into_iter().next()
    else {
        panic!()
    };
    reduce(&mut s, Action::Autosave(gen1));
    assert!(s.save_in_flight);

    // An edit arrives while the first save is still in flight.
    let Some(Effect::Timer { action: Action::Autosave(gen2), .. }) =
        reduce(&mut s, Action::BodyChanged("v2".into())).into_iter().next()
    else {
        panic!()
    };
    let fx = reduce(&mut s, Action::Autosave(gen2));
    assert!(fx.is_empty(), "a save is already in flight; this becomes pending");
    assert!(s.save_pending);

    // First save completes: since a save was pending, a follow-up
    // zero-delay timer is scheduled — but the flight flag stays clear
    // until that timer actually fires and calls `autosave` again.
    let fx = reduce(&mut s, Action::Saved(KvResult::Ok));
    assert_eq!(fx.len(), 1, "the pending edit schedules a follow-up save");
    assert!(!s.save_in_flight, "not in flight again until the follow-up timer fires");
    assert!(!s.save_pending);

    let Some(Effect::Timer { action: Action::Autosave(gen3), .. }) = fx.into_iter().next() else {
        panic!("expected the follow-up save's timer")
    };
    let fx = reduce(&mut s, Action::Autosave(gen3));
    assert_eq!(fx.len(), 1, "the follow-up timer actually persists");
    assert!(s.save_in_flight);
}

#[test]
fn save_failure_reports_status_and_does_not_wedge_the_flight_flag() {
    let mut s = boot();
    reduce(&mut s, Action::NewNote);
    let Some(Effect::Timer { action: Action::Autosave(gen), .. }) =
        reduce(&mut s, Action::BodyChanged("v".into())).into_iter().next()
    else {
        panic!()
    };
    reduce(&mut s, Action::Autosave(gen));
    reduce(&mut s, Action::Saved(KvResult::Err("disk full".into())));
    assert!(!s.save_in_flight);
    assert_eq!(s.status, "save failed: disk full");
}

#[test]
fn store_round_trips_through_json() {
    let mut s = boot();
    reduce(&mut s, Action::NewNote);
    reduce(&mut s, Action::BodyChanged("Round trip\nbody text".into()));
    let Some(Effect::Timer { action: Action::Autosave(gen), .. }) = reduce(
        &mut s,
        Action::BodyChanged("Round trip\nbody text".into()),
    )
    .into_iter()
    .next() else {
        panic!()
    };
    let Some(Effect::KvSet { value, .. }) = reduce(&mut s, Action::Autosave(gen)).into_iter().next() else {
        panic!("expected a KvSet")
    };

    let mut fresh = boot();
    reduce(&mut fresh, Action::StoreLoaded(Some(value)));
    assert_eq!(fresh.notes.len(), 1);
    assert_eq!(fresh.note_rows[0].title, "Round trip");
}

#[test]
fn v1_store_migrates_title_into_the_body() {
    // A hand-written v1 payload (explicit `title`, separate `body`) —
    // the shape `StoreV1`/`NoteV1` describe internally.
    let v1_json = serde_json::json!({
        "folders": [{ "id": 1, "name": "Notes" }],
        "notes": [{
            "id": 5,
            "folder_id": 1,
            "title": "Old Title",
            "body": "legacy body text",
            "created_ms": 1000,
            "updated_ms": 2000,
            "deleted_ms": null
        }]
    });
    let bytes = serde_json::to_vec(&v1_json).unwrap();

    let mut s = boot();
    reduce(&mut s, Action::StoreLoaded(Some(bytes)));
    assert_eq!(s.notes.len(), 1);
    assert_eq!(s.notes[0].body, "Old Title\nlegacy body text");
    assert_eq!(s.note_rows[0].title, "Old Title");
    // ids continue past the migrated data.
    assert_eq!(s.next_note_id, 6);
}

#[test]
fn corrupt_store_bytes_are_discarded_without_crashing() {
    let mut s = boot();
    reduce(&mut s, Action::StoreLoaded(Some(b"not json at all".to_vec())));
    assert_eq!(s.status, "couldn't read saved notes — starting fresh");
    assert_eq!(s.notes.len(), 0);
}
