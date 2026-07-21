//! The State (§5.9 module layout): the one `#[libra::state]` struct plus its
//! inherent helpers (boot, hydration, save scheduling). The action methods that
//! mutate it live in `actions.rs`; this file defines what the state *is*.
//!
//! Architecture: ARCHITECTURE.md §5.9 (module layout).

use libra_core::Effect;

use crate::actions::Action;
use crate::model::{
    DialogKind, Folder, FolderRow, Note, NoteRow, SortOrder, StoreV2, RECENTLY_DELETED, STORE_KEY,
};

#[derive(Default)]
#[libra::state]
pub struct Notes {
    // ---- source of truth (not bound; skip) ----
    #[libra(skip)]
    pub folders: Vec<Folder>,
    #[libra(skip)]
    pub notes: Vec<Note>,
    #[libra(skip)]
    pub next_folder_id: u32,
    #[libra(skip)]
    pub next_note_id: u32,
    #[libra(skip)]
    pub dirty: bool,
    #[libra(skip)]
    pub save_in_flight: bool,
    #[libra(skip)]
    pub save_pending: bool,
    #[libra(skip)]
    pub save_gen: u64,

    // ---- ambient / editing state (bound scalars) ----
    pub selected_folder: u32,
    pub selected_note: u32,
    pub query: String,
    pub now_ms: u64,
    pub theme: String,
    pub status: String,
    /// Controlled mirror of the selected note's body (Input value).
    pub editor_body: String,
    /// M5 pane fractions — model-owned (the `<Split>` controlled shape):
    /// the divider drag dispatches `onResize`, these echo it back, and
    /// rehydration restores the layout across restarts.
    pub split_sidebar: f32,
    pub split_list: f32,
    /// Which modal is up (M6) — source of truth for the `show_*`
    /// projections; not itself reflected.
    #[libra(skip)]
    pub dialog: Option<DialogKind>,
    /// M7: the note whose context menu is open (right-clicked row).
    #[libra(skip)]
    pub menu_note: Option<u32>,
    /// Controlled value of the dialog's name field.
    pub dialog_name: String,
    /// Inline validation line ("name can't be empty", "already exists").
    pub dialog_error: String,
    /// Flips true once `StoreLoaded` fires (success or failure) — drives
    /// the `is_loading` skeleton projection.
    #[libra(skip)]
    pub loaded: bool,
    /// Set when a save fails; cleared on the next successful save.
    /// Source of truth for the dismissable save-error Alert.
    #[libra(skip)]
    pub save_error: Option<String>,
    /// Set when the persisted store couldn't be parsed at boot.
    /// Source of truth for the dismissable store-warning Alert.
    #[libra(skip)]
    pub store_warning: Option<String>,
    /// Note list ordering (Settings drawer Radio group).
    #[libra(skip)]
    pub sort_order: SortOrder,
    /// Settings drawer open/closed.
    #[libra(skip)]
    pub settings_open: bool,

    // ---- display projections (§6.3, rebuilt by #[project]) ----
    pub folder_rows: Vec<FolderRow>,
    pub note_rows: Vec<NoteRow>,
    pub has_trash: bool,
    pub trash_count: u32,
    pub empty_message: String,
    pub folder_name: String,
    pub selected_meta: String,
    pub selected_is_trashed: bool,
    /// Status-bar line: "7 notes · 7 shown" (scope count · post-search
    /// count), mirroring the reference app's footer.
    pub list_summary: String,
    /// M6 dialog projections (§6.3, from `dialog`).
    pub show_folder_dialog: bool,
    pub show_confirm_dialog: bool,
    pub dialog_title: String,
    /// M7: a note row's context menu is open.
    pub show_note_menu: bool,
    /// The store hasn't answered `StoreLoaded` yet — drives the
    /// note-list Skeleton placeholder.
    pub is_loading: bool,
    /// From `save_error` — a dismissable danger Alert.
    pub show_save_error: bool,
    pub error_message: String,
    /// From `store_warning` — a dismissable warning Alert.
    pub show_store_warning: bool,
    pub warning_message: String,
    /// From `settings_open` — the Settings Drawer.
    pub show_settings: bool,
    /// From `theme` — bound to the Settings drawer's dark-mode Switch.
    pub is_dark_theme: bool,
    /// From `sort_order` — bound to the Settings drawer's Radio group.
    pub sort_by_updated: bool,
    pub sort_by_created: bool,
    pub sort_by_title: bool,
    /// A note is open in the editor — the Breadcrumb's trailing crumb.
    pub has_selected_note: bool,
    pub selected_title: String,
}

impl Notes {
    pub fn init() -> (Notes, Vec<Effect<Action>>) {
        let mut state = Notes {
            folders: vec![Folder { id: 1, name: "Notes".into() }],
            next_folder_id: 2,
            next_note_id: 1,
            selected_folder: 1,
            split_sidebar: 0.21,
            split_list: 0.4,
            theme: "libra-dark".into(),
            status: "loading…".into(),
            ..Default::default()
        };
        state.__libra_project();
        (state, vec![Effect::kv_get(STORE_KEY, Action::StoreLoaded)])
    }

    pub(crate) fn hydrate(&mut self, store: StoreV2) {
        self.folders = store.folders;
        self.notes = store.notes;
        self.next_folder_id = store.next_folder_id;
        self.next_note_id = store.next_note_id;
        if !self.folders.iter().any(|f| f.id == self.selected_folder) {
            self.selected_folder = self.folders.first().map_or(RECENTLY_DELETED, |f| f.id);
        }
        self.selected_note = self.first_visible_note_id(self.selected_folder);
    }

    /// The most-recently-edited visible note in `scope` (a real folder
    /// id, or `RECENTLY_DELETED` for the trash view) — matches the
    /// sort order `#[project]` uses, so "first" means "top of the list".
    pub(crate) fn first_visible_note_id(&self, scope: u32) -> u32 {
        let mut visible: Vec<&Note> = if scope == RECENTLY_DELETED {
            self.notes.iter().filter(|n| n.deleted_ms.is_some()).collect()
        } else {
            self.notes.iter().filter(|n| n.folder_id == scope && n.deleted_ms.is_none()).collect()
        };
        visible.sort_by(|a, b| b.updated_ms.cmp(&a.updated_ms));
        visible.first().map_or(0, |n| n.id)
    }

    pub(crate) fn serialize_store(&self) -> Vec<u8> {
        let store = StoreV2 {
            version: 2,
            folders: self.folders.clone(),
            notes: self.notes.clone(),
            next_folder_id: self.next_folder_id,
            next_note_id: self.next_note_id,
        };
        serde_json::to_vec(&store).unwrap_or_default()
    }

    /// §19.2 declarations — validated against the reflected actions at
    /// startup (a typo'd action name is a launch error, not a dead key).
    pub fn shortcuts() -> Vec<libra_core::Shortcut> {
        use libra_core::Shortcut;
        let mut s = vec![
            Shortcut::action("cmd+n", "newNote"),
            Shortcut::action("cmd+shift+n", "openNewFolderDialog"),
            Shortcut::action("cmd+shift+r", "openRenameFolderDialog"),
            Shortcut::action("cmd+backspace", "deleteNote"),
            Shortcut::action("cmd+shift+c", "copyNote"),
            Shortcut::action("cmd+alt+down", "selectNextNote"),
            Shortcut::action("cmd+alt+up", "selectPrevNote"),
            Shortcut::action("cmd+shift+s", "openSettings"),
            // Escape is contextual: open dialog → cancel it; else clear
            // the search (the action decides — §19.2 static payloads).
            Shortcut::action("escape", "dismiss"),
        ];
        for n in 1..=crate::model::MAX_FOLDERS {
            s.push(Shortcut::with_arg(&format!("cmd+{n}"), "jumpFolder", &n.to_string()));
        }
        s
    }

    /// M6 folder-name validation, shared by create and rename. `Ok` is
    /// the trimmed name; `Err` is the inline `dialog_error` line.
    pub(crate) fn validate_folder_name(&self, exclude: Option<u32>) -> Result<String, String> {
        let name = self.dialog_name.trim().to_string();
        if name.is_empty() {
            return Err("name can't be empty".into());
        }
        if self
            .folders
            .iter()
            .any(|f| Some(f.id) != exclude && f.name.eq_ignore_ascii_case(&name))
        {
            return Err(format!("\"{name}\" already exists"));
        }
        Ok(name)
    }

    /// Move the note selection by `delta` within the visible (projected)
    /// row order — the keyboard-navigation core.
    pub(crate) fn step_selection(&mut self, delta: i64) {
        if self.note_rows.is_empty() {
            return;
        }
        let idx = self
            .note_rows
            .iter()
            .position(|r| r.id == self.selected_note)
            .map(|i| (i as i64 + delta).clamp(0, self.note_rows.len() as i64 - 1) as usize)
            .unwrap_or(0);
        self.selected_note = self.note_rows[idx].id;
    }

    /// Bumps the save generation and schedules the (possibly-debounced)
    /// autosave timer. A timer whose generation no longer matches
    /// `save_gen` when it fires was superseded by a later edit and is a
    /// silent no-op (`autosave` in `actions.rs`) — that's the whole
    /// debounce.
    pub(crate) fn schedule_save(&mut self, after_ms: u64) -> Effect<Action> {
        self.dirty = true;
        self.save_gen += 1;
        Effect::timer(after_ms, Action::Autosave(self.save_gen))
    }
}
