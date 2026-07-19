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

/// The M6 dialog flow through the real UI: open, type, confirm.
fn add_folder_via_dialog(h: &mut Harness<Notes>, name: &str) {
    h.click(by::test_id("new-folder"));
    h.type_text(by::test_id("dialog-name"), name);
    h.blur();
    h.click(by::test_id("dialog-confirm"));
    settle_save(h);
}

#[test]
fn adding_a_folder_and_switching_shows_its_own_notes() {
    let mut h = harness();
    add_note(&mut h, "In Notes folder");

    add_folder_via_dialog(&mut h, "Work");

    // Switched to the new (empty) folder automatically.
    h.query(by::test_id("folder-name")).assert_text("Work");
    assert_eq!(h.state().note_rows.len(), 0);

    let notes_folder_id = h.state().folder_rows[0].id;
    h.dispatch(Action::SelectFolder(notes_folder_id.to_string()));
    assert_eq!(h.state().note_rows.len(), 1);
}

/// M6: the scrim is a modal barrier — app controls beneath an open
/// dialog are unreachable, and click-outside dismisses (onDismiss).
#[test]
fn open_dialog_blocks_the_app_beneath_and_click_outside_dismisses() {
    let mut h = harness();
    h.click(by::test_id("new-note"));
    settle_save(&mut h);
    assert_eq!(h.state().note_rows.len(), 1);

    h.click(by::test_id("new-folder"));
    assert!(h.state().show_folder_dialog);
    // The dialog panel sits mid-viewport; "new-note" lives in the list
    // header far left — clicking its coordinates must hit the scrim
    // (dismissing via onDismiss), never the button beneath.
    h.click(by::test_id("new-note"));
    assert!(!h.state().show_folder_dialog, "scrim click dismissed the dialog");
    assert_eq!(h.state().note_rows.len(), 1, "the button beneath never fired");
}

#[test]
fn interval_source_is_mounted_for_live_relative_timestamps() {
    let h = harness();
    assert!(h.has_source("interval"));
}

/// §19.2 (PLAN-NOTES-APP M4): the declared shortcut set drives the app
/// through the same validated table the shell uses.
#[test]
fn shortcuts_create_navigate_and_delete() {
    let mut h = harness();
    assert!(h.press_shortcut("cmd+n"), "cmd+n creates a note");
    settle_save(&mut h);
    // Advance the clock so the second note is unambiguously newer
    // (rows sort by updated_ms; time only enters via the tick action).
    h.dispatch(Action::Tick(libra_core::Instant::at(60_000.0)));
    assert!(h.press_shortcut("cmd+n"));
    settle_save(&mut h);
    assert_eq!(h.state().note_rows.len(), 2);

    // Newest note is selected (top of the list); alt+down walks down.
    let top = h.state().note_rows[0].id;
    let bottom = h.state().note_rows[1].id;
    assert_eq!(h.state().selected_note, top);
    h.press_shortcut("cmd+alt+down");
    assert_eq!(h.state().selected_note, bottom);
    h.press_shortcut("cmd+alt+down");
    assert_eq!(h.state().selected_note, bottom, "clamps at the end");
    h.press_shortcut("cmd+alt+up");
    assert_eq!(h.state().selected_note, top);

    assert!(h.press_shortcut("cmd+backspace"), "cmd+backspace trashes the selection");
    settle_save(&mut h);
    assert_eq!(h.state().note_rows.len(), 1);
    assert!(h.state().has_trash);
}

/// §19.2 precedence: a focused text control suppresses non-`cmd`
/// chords — escape blurs instead of dispatching; unfocused, it fires.
#[test]
fn escape_clears_search_only_when_unfocused() {
    let mut h = harness();
    add_note(&mut h, "findable");
    h.type_text(by::test_id("search"), "find");
    assert_eq!(h.state().query, "find");

    assert!(!h.press_shortcut("escape"), "focused: reserved, does not dispatch");
    assert_eq!(h.state().query, "find", "query untouched while typing");

    h.blur();
    assert!(h.press_shortcut("escape"), "unfocused: the declared shortcut fires");
    assert_eq!(h.state().query, "");
}

/// §19.2 static payloads: `cmd+2` jumps to the second folder.
#[test]
fn folder_jump_shortcuts_carry_their_static_arg() {
    let mut h = harness();
    add_folder_via_dialog(&mut h, "Work");
    h.blur();

    assert!(h.press_shortcut("cmd+1"));
    h.query(by::test_id("folder-name")).assert_text("Notes");
    assert!(h.press_shortcut("cmd+2"));
    h.query(by::test_id("folder-name")).assert_text("Work");
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

/// M5: nested Splits — resizing echoes into the clamped model fields
/// and repaints; resizing back restores identical bytes.
#[test]
fn pane_splits_resize_and_round_trip() {
    let mut h = harness();
    let resting = h.frame_png();
    h.resize_split("sidebar-split", 0.3);
    assert!((h.state().split_sidebar - 0.3).abs() < 1e-4);
    assert_ne!(h.frame_png(), resting, "wider sidebar repaints");
    h.resize_split("sidebar-split", 0.05); // below the app's clamp
    assert!((h.state().split_sidebar - 0.1).abs() < 1e-4, "action clamps to 0.1 min");
    h.resize_split("sidebar-split", 0.21);
    assert_eq!(h.frame_png(), resting, "back to the default fraction: identical bytes");
}

/// M7: right-click opens the context menu at the row (selection
/// follows), a menu item acts on that note, and the barrier click
/// closes without touching the app beneath.
#[test]
fn right_click_menu_deletes_the_clicked_note() {
    let mut h = harness();
    add_note(&mut h, "First note");
    h.dispatch(Action::Tick(libra_core::Instant::at(60_000.0)));
    add_note(&mut h, "Second note");
    assert_eq!(h.state().note_rows.len(), 2);

    h.right_click(by::text("First note"));
    assert!(h.state().show_note_menu);
    assert_eq!(h.state().note_rows.iter().find(|r| r.selected).unwrap().title, "First note");

    h.click(by::test_id("menu-delete"));
    settle_save(&mut h);
    assert!(!h.state().show_note_menu, "acting closes the menu");
    assert_eq!(h.state().note_rows.len(), 1);
    assert_eq!(h.state().note_rows[0].title, "Second note");
    assert!(h.state().has_trash);
}

/// M7: the trashed-note menu offers Restore / Delete Forever, and
/// Delete Forever routes through the M6 confirm dialog.
#[test]
fn trashed_row_menu_routes_delete_forever_through_the_confirm_dialog() {
    let mut h = harness();
    add_note(&mut h, "Doomed");
    h.click(by::test_id("delete-note"));
    settle_save(&mut h);
    h.click(by::test_id("trash-folder"));

    h.right_click(by::text("Doomed"));
    h.click(by::test_id("menu-delete-forever"));
    assert!(h.state().show_confirm_dialog, "menu routes into the confirm dialog");
    h.click(by::test_id("confirm-delete"));
    settle_save(&mut h);
    assert_eq!(h.state().notes.len(), 0);
}

/// M7: escape closes the menu first (menu > dialog > search).
#[test]
fn escape_closes_the_menu_before_anything_else() {
    let mut h = harness();
    add_note(&mut h, "A note");
    h.type_text(by::test_id("search"), "a");
    h.blur();
    h.right_click(by::text("A note"));
    assert!(h.state().show_note_menu);
    assert!(h.press_shortcut("escape"));
    assert!(!h.state().show_note_menu);
    assert_eq!(h.state().query, "a", "search untouched by the menu-closing escape");
}
