//! The State (§5.9 module layout): the one `#[libra::state]` struct plus its
//! inherent helpers (boot, hydration, save scheduling). The action methods that
//! mutate it live in `actions.rs`; this file defines what the state *is*.
//!
//! Architecture: ARCHITECTURE.md §5.9 (module layout).

use libra_core::Effect;

use crate::actions::Action;
use crate::model::{Folder, Note, FolderRow, NoteRow, StoreV2, RECENTLY_DELETED, STORE_KEY};

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
    /// Draft text for the interim add-folder row (stand-in for the
    /// M6 Dialog).
    pub new_folder_name: String,

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
}

impl Notes {
    pub fn init() -> (Notes, Vec<Effect<Action>>) {
        let mut state = Notes {
            folders: vec![Folder { id: 1, name: "Notes".into() }],
            next_folder_id: 2,
            next_note_id: 1,
            selected_folder: 1,
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
