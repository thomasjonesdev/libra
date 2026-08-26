//! Gloss app state. Display fields plus skipped I/O bookkeeping.

mod project;

use std::collections::HashSet;

use libra_core::{Effect, ProcessId};

use crate::actions::Action;
use crate::model::{
    apply_loaded_note, full_note_text, list_cmd, expand_ancestors_into, IoKind, Pending,
    SidebarView, STORE_KEY,
};

#[derive(Default)]
#[libra::state]
pub struct Gloss {
    pub vault_path: String,
    pub vault_name: String,
    pub query: String,
    pub selected_path: String,
    pub selected_title: String,
    pub editor_body: String,
    pub status: String,
    pub empty_message: String,
    pub word_count: String,
    pub char_count: String,
    pub save_busy: bool,
    pub save_ok: bool,
    pub mode_label: String,
    pub has_vault: bool,
    pub has_selected: bool,
    pub has_tabs: bool,
    pub can_undo: bool,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub reader_mode: bool,
    pub has_frontmatter: bool,
    pub has_frontmatter_title: bool,
    pub frontmatter_title: String,
    pub frontmatter_yaml: String,
    pub frontmatter_fields: Vec<crate::model::FrontmatterField>,
    pub show_settings: bool,
    pub show_command_palette: bool,
    pub palette_query: String,
    pub palette_rows: Vec<crate::model::NoteRow>,
    pub show_mention: bool,
    pub mention_query: String,
    pub mention_rows: Vec<crate::model::NoteRow>,
    pub has_rules: bool,
    pub rules_excerpt: String,
    pub toast_ok: bool,
    pub toast_err: bool,
    pub toast_title: String,
    pub show_graph: bool,
    pub graph_loading: bool,
    pub outgoing_rows: Vec<crate::model::NoteRow>,
    pub backlink_rows: Vec<crate::model::NoteRow>,
    pub split_list: f32,
    pub sidebar_open: bool,
    pub sidebar_files: bool,
    pub sidebar_search: bool,
    pub sidebar_bookmarks: bool,
    pub note_rows: Vec<crate::model::NoteRow>,
    pub file_rows: Vec<crate::model::FileRow>,
    pub tab_rows: Vec<crate::model::TabRow>,

    #[libra(skip)]
    pub loaded_body: String,
    #[libra(skip)]
    pub note_paths: Vec<String>,
    #[libra(skip)]
    pub io: IoKind,
    #[libra(skip)]
    pub pending: Pending,
    #[libra(skip)]
    pub opened_root: String,
    #[libra(skip)]
    pub next_process_id: u64,
    #[libra(skip)]
    pub next_timer_id: u64,
    #[libra(skip)]
    pub dirty: bool,
    #[libra(skip)]
    pub save_pending: bool,
    #[libra(skip)]
    pub save_timer: Option<u64>,
    #[libra(skip)]
    pub in_flight_body: String,
    #[libra(skip)]
    pub expanded: HashSet<String>,
    #[libra(skip)]
    pub sort_alpha: bool,
    #[libra(skip)]
    pub sidebar_view: SidebarView,
    #[libra(skip)]
    pub split_list_saved: f32,
    #[libra(skip)]
    pub bookmarks: Vec<String>,
    #[libra(skip)]
    pub open_tabs: Vec<String>,
    #[libra(skip)]
    pub open_new_tab: bool,
    #[libra(skip)]
    pub history_back: Vec<String>,
    #[libra(skip)]
    pub history_forward: Vec<String>,
    #[libra(skip)]
    pub history_lock: bool,
    #[libra(skip)]
    pub daily_date_override: Option<String>,
    #[libra(skip)]
    pub mention_at: usize,
    #[libra(skip)]
    pub rules_loaded: bool,
    #[libra(skip)]
    pub rules_body: String,
}

impl Gloss {
    pub fn alloc_process(&mut self) -> ProcessId {
        let id = self.next_process_id;
        self.next_process_id += 1;
        ProcessId(id)
    }

    pub fn alloc_timer(&mut self) -> libra_core::TimerId {
        let id = self.next_timer_id;
        self.next_timer_id += 1;
        libra_core::TimerId(id)
    }

    pub fn is_busy(&self) -> bool {
        self.io != IoKind::Idle
    }

    pub fn spawn(
        &mut self,
        command: libra_core::CommandSpec,
        kind: IoKind,
    ) -> Option<Effect<Action>> {
        if self.is_busy() {
            return None;
        }
        self.io = kind;
        let id = self.alloc_process();
        Some(Effect::process_id(id, command, Action::IoDone))
    }

    pub fn full_note_text(&self) -> String {
        full_note_text(
            self.reader_mode,
            self.has_frontmatter,
            &self.frontmatter_yaml,
            &self.editor_body,
        )
    }

    pub fn set_note_text(&mut self, full: &str) {
        let (yaml, editor, has, title, fields) = apply_loaded_note(self.reader_mode, full);
        self.frontmatter_yaml = yaml;
        self.editor_body = editor;
        self.has_frontmatter = has;
        self.frontmatter_title = title.clone();
        self.has_frontmatter_title = !title.is_empty();
        self.frontmatter_fields = fields;
    }

    pub fn expand_ancestors(&mut self, path: &str) {
        expand_ancestors_into(&mut self.expanded, path);
    }

    pub fn place_in_tab(&mut self, path: &str) {
        if path.is_empty() {
            return;
        }
        if self.open_tabs.iter().any(|p| p == path) {
            self.open_new_tab = false;
            return;
        }
        if self.open_new_tab {
            self.open_tabs.push(path.to_string());
            self.open_new_tab = false;
            return;
        }
        if let Some(i) = self.open_tabs.iter().position(|p| p == &self.selected_path) {
            self.open_tabs[i] = path.to_string();
            return;
        }
        self.open_tabs.push(path.to_string());
    }

    pub fn init() -> (Gloss, Vec<Effect<Action>>) {
        let state = Gloss {
            split_list: 0.32,
            split_list_saved: 0.32,
            sidebar_open: true,
            sidebar_view: SidebarView::Files,
            reader_mode: true,
            status: "Browse a vault folder".into(),
            ..Default::default()
        };
        let mut state = state;
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
        if self.is_busy() {
            self.pending = Pending::List;
            return None;
        }
        self.rules_loaded = false;
        self.rules_body.clear();
        self.status = "Listing…".into();
        self.spawn(list_cmd(root), IoKind::List)
    }

    /// §19.2 shortcut table — validated at startup.
    pub fn shortcuts() -> Vec<libra_core::Shortcut> {
        use libra_core::Shortcut;
        vec![
            Shortcut::action("cmd+k", "openCommandPalette"),
            Shortcut::action("cmd+p", "openCommandPalette"),
            Shortcut::action("cmd+d", "openDailyNote"),
            Shortcut::action("cmd+n", "newNote"),
            Shortcut::action("cmd+g", "openGraph"),
            Shortcut::action("cmd+e", "toggleReaderMode"),
            Shortcut::action("escape", "dismiss"),
        ]
    }
}
