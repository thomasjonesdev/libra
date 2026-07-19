//! The actions (§5.9 module layout): the crate's one `#[libra::actions]` block
//! — every state transition, the effect continuations, and the `#[project]`
//! hook. The macro generates the `Action` enum and `reduce` in this module;
//! `lib.rs` re-exports them for a flat `notes::{Action, reduce}` surface.
//!
//! Architecture: ARCHITECTURE.md §5.9 (module layout), §5.2 (macro expansion).

use libra_core::{Effect, Instant, KvResult};

use crate::model::{
    clip_bytes, migrate_v1, note_meta, note_snippet, note_title, DialogKind, Folder, FolderRow,
    Note, NoteRow, StoredFormat, AUTOSAVE_DEBOUNCE_MS, MAX_BODY_BYTES, MAX_FOLDERS, MAX_NOTES,
    RECENTLY_DELETED, STORE_KEY,
};
use crate::state::Notes;

#[libra::actions]
impl Notes {
    // ---- navigation ----

    pub fn select_folder(&mut self, id: String) {
        let Ok(id) = id.parse::<u32>() else { return };
        if !self.folders.iter().any(|f| f.id == id) {
            return;
        }
        self.selected_folder = id;
        self.query.clear();
        self.selected_note = self.first_visible_note_id(id);
    }

    pub fn open_recently_deleted(&mut self) {
        self.selected_folder = RECENTLY_DELETED;
        self.query.clear();
        self.selected_note = self.first_visible_note_id(RECENTLY_DELETED);
    }

    pub fn select_note(&mut self, id: String) {
        let Ok(id) = id.parse::<u32>() else { return };
        if self.notes.iter().any(|n| n.id == id) {
            self.selected_note = id;
        }
    }

    pub fn query_changed(&mut self, text: String) {
        self.query = text;
    }

    pub fn clear_query(&mut self) {
        self.query.clear();
    }

    // ---- notes ----

    pub fn new_note(&mut self) -> Option<Effect<Action>> {
        if self.selected_folder == RECENTLY_DELETED {
            return None;
        }
        if self.notes.len() >= MAX_NOTES {
            self.status = "note limit reached (48)".into();
            return None;
        }
        let id = self.next_note_id;
        self.next_note_id += 1;
        self.notes.push(Note {
            id,
            folder_id: self.selected_folder,
            body: String::new(),
            created_ms: self.now_ms,
            updated_ms: self.now_ms,
            deleted_ms: None,
        });
        self.selected_note = id;
        Some(self.schedule_save(0))
    }

    /// Controlled editor `onChange`. Read-only enforcement for trashed
    /// notes lives here, not in the widget (§5.3: the state decides).
    pub fn body_changed(&mut self, text: String) -> Option<Effect<Action>> {
        let id = self.selected_note;
        let n = self.notes.iter_mut().find(|n| n.id == id)?;
        if n.deleted_ms.is_some() {
            return None; // trashed notes are read-only
        }
        n.body = clip_bytes(&text, MAX_BODY_BYTES);
        n.updated_ms = self.now_ms;
        Some(self.schedule_save(AUTOSAVE_DEBOUNCE_MS))
    }

    pub fn delete_note(&mut self) -> Option<Effect<Action>> {
        let id = self.selected_note;
        let n = self.notes.iter_mut().find(|n| n.id == id)?;
        if n.deleted_ms.is_some() {
            return None;
        }
        n.deleted_ms = Some(self.now_ms);
        self.selected_note = self.first_visible_note_id(self.selected_folder);
        Some(self.schedule_save(0))
    }

    pub fn restore_note(&mut self) -> Option<Effect<Action>> {
        let id = self.selected_note;
        let folder_id = {
            let n = self.notes.iter_mut().find(|n| n.id == id && n.deleted_ms.is_some())?;
            n.deleted_ms = None;
            n.folder_id
        };
        self.selected_folder = folder_id;
        self.selected_note = id;
        Some(self.schedule_save(0))
    }

    /// Permanent delete asks first (M6): opens the confirm dialog; the
    /// actual removal happens in `dialog_confirm`.
    pub fn delete_note_permanently(&mut self) {
        let id = self.selected_note;
        if self.notes.iter().any(|n| n.id == id && n.deleted_ms.is_some()) {
            self.dialog = Some(DialogKind::ConfirmDelete);
            self.dialog_error.clear();
        }
    }

    // ---- dialogs (M6): create/rename folder + confirm permanent delete ----

    pub fn open_new_folder_dialog(&mut self) {
        self.dialog = Some(DialogKind::NewFolder);
        self.dialog_name.clear();
        self.dialog_error.clear();
    }

    /// `cmd+shift+r` / the Rename button — prefills the current name.
    pub fn open_rename_folder_dialog(&mut self) {
        let id = self.selected_folder;
        let Some(folder) = self.folders.iter().find(|f| f.id == id) else { return };
        self.dialog_name = folder.name.clone();
        self.dialog = Some(DialogKind::RenameFolder);
        self.dialog_error.clear();
    }

    pub fn dialog_name_changed(&mut self, text: String) {
        self.dialog_name = text;
        self.dialog_error.clear();
    }

    pub fn dialog_cancel(&mut self) {
        self.dialog = None;
    }

    /// Escape (§19.2): contextual — cancel an open dialog, else clear
    /// the search.
    pub fn dismiss(&mut self) {
        if self.dialog.is_some() {
            self.dialog = None;
        } else {
            self.query.clear();
        }
    }

    /// The dialog's primary button. Validation failures set the inline
    /// `dialog_error` and keep the dialog open (M6).
    pub fn dialog_confirm(&mut self) -> Option<Effect<Action>> {
        match self.dialog? {
            DialogKind::NewFolder => {
                if self.folders.len() >= MAX_FOLDERS {
                    self.dialog_error = format!("folder limit reached ({MAX_FOLDERS})");
                    return None;
                }
                match self.validate_folder_name(None) {
                    Err(e) => {
                        self.dialog_error = e;
                        None
                    }
                    Ok(name) => {
                        let id = self.next_folder_id;
                        self.next_folder_id += 1;
                        self.folders.push(Folder { id, name });
                        self.selected_folder = id;
                        self.selected_note = 0;
                        self.dialog = None;
                        Some(self.schedule_save(0))
                    }
                }
            }
            DialogKind::RenameFolder => {
                let id = self.selected_folder;
                match self.validate_folder_name(Some(id)) {
                    Err(e) => {
                        self.dialog_error = e;
                        None
                    }
                    Ok(name) => {
                        self.folders.iter_mut().find(|f| f.id == id)?.name = name;
                        self.dialog = None;
                        Some(self.schedule_save(0))
                    }
                }
            }
            DialogKind::ConfirmDelete => {
                let id = self.selected_note;
                self.dialog = None;
                let before = self.notes.len();
                self.notes.retain(|n| !(n.id == id && n.deleted_ms.is_some()));
                if self.notes.len() == before {
                    return None;
                }
                self.selected_note = self.first_visible_note_id(self.selected_folder);
                Some(self.schedule_save(0))
            }
        }
    }

    /// Deleting a folder trashes its notes rather than destroying them
    /// (matches the reference's soft-delete policy); the last remaining
    /// folder can't be removed.
    pub fn delete_folder(&mut self) -> Option<Effect<Action>> {
        let id = self.selected_folder;
        if id == RECENTLY_DELETED || self.folders.len() <= 1 {
            return None;
        }
        self.folders.retain(|f| f.id != id);
        for n in self.notes.iter_mut().filter(|n| n.folder_id == id && n.deleted_ms.is_none()) {
            n.deleted_ms = Some(self.now_ms);
        }
        self.selected_folder = self.folders.first().map_or(RECENTLY_DELETED, |f| f.id);
        self.selected_note = self.first_visible_note_id(self.selected_folder);
        Some(self.schedule_save(0))
    }

    // ---- keyboard navigation (§19.2 shortcuts) ----

    /// `cmd+alt+down` — next note in the visible (sorted/filtered) list.
    pub fn select_next_note(&mut self) {
        self.step_selection(1);
    }

    /// `cmd+alt+up` — previous note in the visible list.
    pub fn select_prev_note(&mut self) {
        self.step_selection(-1);
    }

    /// `cmd+1`…`cmd+6` — jump to the nth folder (1-based, declaration
    /// order). The arg is the §19.2 static payload.
    pub fn jump_folder(&mut self, n: String) {
        let Ok(n) = n.parse::<usize>() else { return };
        let Some(folder) = self.folders.get(n.saturating_sub(1)) else { return };
        let id = folder.id.to_string();
        self.select_folder(id);
    }

    // ---- ambient ----

    /// `<Interval every={30000} onTick={tick}>` — the only clock input
    /// in this app; relative-time labels recompute in `#[project]`.
    pub fn tick(&mut self, instant: Instant) {
        self.now_ms = instant.t_ms as u64;
    }

    pub fn toggle_theme(&mut self) -> Effect<Action> {
        self.theme = if self.theme == "libra-dark" { "libra-light".into() } else { "libra-dark".into() };
        Effect::SetTheme { name: self.theme.clone() }
    }

    // ---- continuations ----

    fn store_loaded(&mut self, bytes: Option<Vec<u8>>) {
        self.status = "ready".into();
        let Some(bytes) = bytes else { return };
        match serde_json::from_slice::<StoredFormat>(&bytes) {
            Ok(StoredFormat::V2(store)) => self.hydrate(store),
            Ok(StoredFormat::V1(v1)) => self.hydrate(migrate_v1(v1)),
            Err(_) => self.status = "couldn't read saved notes — starting fresh".into(),
        }
    }

    /// Fires after the debounce (or immediately, for structural
    /// changes). `gen` no longer matching `save_gen` means a later edit
    /// already superseded this timer — silently drop it (§12.3).
    fn autosave(&mut self, gen: u64) -> Option<Effect<Action>> {
        if gen != self.save_gen {
            return None;
        }
        if self.save_in_flight {
            self.save_pending = true;
            return None;
        }
        self.save_in_flight = true;
        self.status = "saving…".into();
        Some(Effect::kv_set_acked(STORE_KEY, self.serialize_store(), Action::Saved))
    }

    /// Exactly one write in flight: a save that arrives while an edit
    /// was made mid-flight re-persists once the first one lands.
    fn saved(&mut self, result: KvResult) -> Option<Effect<Action>> {
        self.save_in_flight = false;
        match result {
            KvResult::Ok => {
                self.dirty = false;
                self.status = "saved".into();
            }
            KvResult::Err(e) => self.status = format!("save failed: {e}"),
        }
        if self.save_pending {
            self.save_pending = false;
            self.save_gen += 1;
            return Some(Effect::timer(0, Action::Autosave(self.save_gen)));
        }
        None
    }

    /// §6.3: rebuilt after every action, so title/snippet/word-count/
    /// relative-time and the filtered lists can never go stale.
    #[project]
    fn project(&mut self) {
        self.trash_count = self.notes.iter().filter(|n| n.deleted_ms.is_some()).count() as u32;
        self.has_trash = self.trash_count > 0;

        // M6 dialog projections — the view mounts dialogs via `<if>`.
        self.show_folder_dialog =
            matches!(self.dialog, Some(DialogKind::NewFolder | DialogKind::RenameFolder));
        self.show_confirm_dialog = matches!(self.dialog, Some(DialogKind::ConfirmDelete));
        self.dialog_title = match self.dialog {
            Some(DialogKind::NewFolder) => "New Folder",
            Some(DialogKind::RenameFolder) => "Rename Folder",
            Some(DialogKind::ConfirmDelete) => "Delete Forever?",
            None => "",
        }
        .to_string();

        self.folder_rows = self
            .folders
            .iter()
            .map(|f| FolderRow {
                id: f.id,
                name: f.name.clone(),
                count: self
                    .notes
                    .iter()
                    .filter(|n| n.folder_id == f.id && n.deleted_ms.is_none())
                    .count() as u32,
                selected: f.id == self.selected_folder,
            })
            .collect();

        let query = self.query.trim().to_lowercase();
        let in_scope: Vec<&Note> = if self.selected_folder == RECENTLY_DELETED {
            self.notes.iter().filter(|n| n.deleted_ms.is_some()).collect()
        } else {
            self.notes
                .iter()
                .filter(|n| n.folder_id == self.selected_folder && n.deleted_ms.is_none())
                .collect()
        };
        let scope_count = in_scope.len();
        let mut visible: Vec<&Note> = in_scope
            .into_iter()
            .filter(|n| {
                query.is_empty()
                    || note_title(n).to_lowercase().contains(&query)
                    || n.body.to_lowercase().contains(&query)
            })
            .collect();
        visible.sort_by(|a, b| b.updated_ms.cmp(&a.updated_ms));

        let noun = if scope_count == 1 { "note" } else { "notes" };
        self.list_summary = format!("{scope_count} {noun} · {} shown", visible.len());

        self.note_rows = visible
            .iter()
            .map(|n| NoteRow {
                id: n.id,
                title: note_title(n),
                snippet: note_snippet(n),
                meta: note_meta(self.now_ms, n),
                selected: n.id == self.selected_note,
            })
            .collect();

        self.empty_message = if !self.note_rows.is_empty() {
            String::new()
        } else if !query.is_empty() {
            "No matches".into()
        } else if self.selected_folder == RECENTLY_DELETED {
            "Recently Deleted is empty".into()
        } else {
            "No notes yet".into()
        };

        self.folder_name = if self.selected_folder == RECENTLY_DELETED {
            "Recently Deleted".into()
        } else {
            self.folders.iter().find(|f| f.id == self.selected_folder).map(|f| f.name.clone()).unwrap_or_default()
        };

        match self.notes.iter().find(|n| n.id == self.selected_note) {
            Some(n) => {
                self.editor_body = n.body.clone();
                self.selected_meta = note_meta(self.now_ms, n);
                self.selected_is_trashed = n.deleted_ms.is_some();
            }
            None => {
                self.editor_body.clear();
                self.selected_meta.clear();
                self.selected_is_trashed = false;
                self.selected_note = 0;
            }
        }
    }
}
