//! Gloss app state. Display fields plus skipped I/O bookkeeping.

use libra_core::{Effect, ProcessId};

use crate::actions::Action;
use crate::model::{list_cmd, IoKind, NoteRow, STORE_KEY};

#[derive(Default)]
#[libra::state]
pub struct Gloss {
    pub vault_path: String,
    pub query: String,
    pub selected_path: String,
    pub selected_title: String,
    pub editor_body: String,
    pub status: String,
    pub empty_message: String,
    pub has_vault: bool,
    pub has_selected: bool,
    pub can_undo: bool,
    pub split_list: f32,
    pub note_rows: Vec<NoteRow>,

    #[libra(skip)]
    pub loaded_body: String,
    #[libra(skip)]
    pub note_paths: Vec<String>,
    #[libra(skip)]
    pub io: IoKind,
    #[libra(skip)]
    pub next_process_id: u64,
    #[libra(skip)]
    pub dirty: bool,
    #[libra(skip)]
    pub save_pending: bool,
    #[libra(skip)]
    pub in_flight_body: String,
}

impl Gloss {
    pub fn alloc_process(&mut self) -> ProcessId {
        let id = self.next_process_id;
        self.next_process_id += 1;
        ProcessId(id)
    }

    pub fn spawn(&mut self, command: libra_core::CommandSpec, kind: IoKind) -> Effect<Action> {
        self.io = kind;
        let id = self.alloc_process();
        Effect::process_id(id, command, Action::IoDone)
    }

    pub fn rebuild_rows(&mut self) {
        let q = self.query.to_ascii_lowercase();
        self.note_rows = self
            .note_paths
            .iter()
            .filter(|p| q.is_empty() || p.to_ascii_lowercase().contains(&q))
            .map(|p| {
                let selected = p == &self.selected_path;
                let body = if selected {
                    self.editor_body.as_str()
                } else {
                    ""
                };
                NoteRow {
                    id: p.clone(),
                    title: crate::model::note_title(p, body),
                    snippet: if selected {
                        crate::model::note_snippet(body)
                    } else {
                        String::new()
                    },
                    selected,
                }
            })
            .collect();
        self.has_vault = !self.vault_path.trim().is_empty() && !self.note_paths.is_empty()
            || self.io == IoKind::List
            || !self.vault_path.trim().is_empty();
        self.has_selected = !self.selected_path.is_empty();
        self.selected_title = if self.selected_path.is_empty() {
            String::new()
        } else {
            crate::model::note_title(&self.selected_path, &self.editor_body)
        };
        self.empty_message = if self.vault_path.trim().is_empty() {
            "Open a vault folder to begin".into()
        } else if self.note_rows.is_empty() {
            "No notes in this vault".into()
        } else {
            String::new()
        };
        if self.split_list == 0.0 {
            self.split_list = 0.32;
        }
    }

    pub fn init() -> (Gloss, Vec<Effect<Action>>) {
        let mut state = Gloss::default();
        state.split_list = 0.32;
        state.status = "Set a vault path".into();
        state.rebuild_rows();
        (
            state,
            vec![Effect::kv_get(STORE_KEY, Action::WorkspaceLoaded)],
        )
    }

    pub fn open_list(&mut self) -> Option<Effect<Action>> {
        let root = self.vault_path.trim();
        if root.is_empty() {
            self.status = "Vault path is empty".into();
            return None;
        }
        self.status = "Listing…".into();
        Some(self.spawn(list_cmd(root), IoKind::List))
    }
}
