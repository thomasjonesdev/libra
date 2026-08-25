//! Gloss app state. Display fields plus skipped I/O bookkeeping.

use std::collections::HashSet;

use libra_core::{Effect, ProcessId};

use crate::actions::Action;
use crate::model::{
    char_count_label, frontmatter_scan_rows, frontmatter_title, full_note_text, list_cmd,
    project_file_rows, project_link_rows, project_mention_rows, project_palette_rows, rules_preview,
    save_label, vault_name, word_count_label, FileRow, IoKind, NoteRow, Pending, TabRow, STORE_KEY,
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
    pub save_label: String,
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
    pub palette_rows: Vec<NoteRow>,
    pub show_mention: bool,
    pub mention_query: String,
    pub mention_rows: Vec<NoteRow>,
    pub has_rules: bool,
    pub rules_excerpt: String,
    pub toast_ok: bool,
    pub toast_err: bool,
    pub toast_title: String,
    pub show_graph: bool,
    pub graph_loading: bool,
    pub outgoing_rows: Vec<NoteRow>,
    pub backlink_rows: Vec<NoteRow>,
    pub split_list: f32,
    pub note_rows: Vec<NoteRow>,
    pub file_rows: Vec<FileRow>,
    pub tab_rows: Vec<TabRow>,

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
    pub dirty: bool,
    #[libra(skip)]
    pub save_pending: bool,
    #[libra(skip)]
    pub in_flight_body: String,
    #[libra(skip)]
    pub expanded: HashSet<String>,
    #[libra(skip)]
    pub sort_alpha: bool,
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

    pub fn is_busy(&self) -> bool {
        self.io != IoKind::Idle
    }

    pub fn spawn(&mut self, command: libra_core::CommandSpec, kind: IoKind) -> Option<Effect<Action>> {
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
        let (yaml, editor, has, title, fields) =
            crate::model::apply_loaded_note(self.reader_mode, full);
        self.frontmatter_yaml = yaml;
        self.editor_body = editor;
        self.has_frontmatter = has;
        self.frontmatter_title = title.clone();
        self.has_frontmatter_title = !title.is_empty();
        self.frontmatter_fields = fields;
    }

    pub fn rebuild_rows(&mut self) {
        let full = self.full_note_text();
        let q = self.query.to_ascii_lowercase();
        self.note_rows = self
            .note_paths
            .iter()
            .filter(|p| q.is_empty() || p.to_ascii_lowercase().contains(&q))
            .map(|p| {
                let selected = p == &self.selected_path;
                let body = if selected { full.as_str() } else { "" };
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
        self.file_rows = project_file_rows(
            &self.note_paths,
            &self.query,
            &self.selected_path,
            &self.expanded,
            self.sort_alpha,
        );
        self.tab_rows = self
            .open_tabs
            .iter()
            .map(|p| TabRow {
                id: p.clone(),
                title: crate::model::file_stem(p),
                selected: p == &self.selected_path,
            })
            .collect();
        self.vault_name = vault_name(&self.vault_path);
        self.has_vault = !self.vault_path.trim().is_empty() && !self.note_paths.is_empty()
            || self.io == IoKind::List
            || !self.vault_path.trim().is_empty();
        self.has_selected = !self.selected_path.is_empty();
        self.has_tabs = !self.open_tabs.is_empty();
        self.can_go_back = !self.history_back.is_empty();
        self.can_go_forward = !self.history_forward.is_empty();
        self.selected_title = if self.selected_path.is_empty() {
            String::new()
        } else {
            crate::model::note_title(&self.selected_path, &full)
        };
        self.empty_message = if self.vault_path.trim().is_empty() {
            "Browse a vault folder to begin".into()
        } else if self.file_rows.is_empty() {
            "No notes in this vault".into()
        } else {
            String::new()
        };
        self.word_count = word_count_label(&full);
        self.char_count = char_count_label(&full);
        self.save_label = save_label(self.dirty, &self.status);
        self.mode_label = if self.reader_mode { "Raw" } else { "Reader" }.into();
        if self.reader_mode {
            self.frontmatter_title = frontmatter_title(&self.frontmatter_yaml);
            self.has_frontmatter_title = !self.frontmatter_title.is_empty();
            self.frontmatter_fields = frontmatter_scan_rows(&self.frontmatter_yaml);
        }
        self.palette_rows = if self.show_command_palette {
            project_palette_rows(&self.note_paths, &self.palette_query, 50)
        } else {
            Vec::new()
        };
        self.mention_rows = if self.show_mention {
            project_mention_rows(&self.note_paths, &self.mention_query, 50)
        } else {
            Vec::new()
        };
        self.has_rules = !self.rules_body.trim().is_empty();
        self.rules_excerpt = rules_preview(&self.rules_body, 480);
        if self.show_graph && self.has_selected {
            self.outgoing_rows =
                project_link_rows(&full, &self.note_paths, &self.selected_path);
        } else if !self.show_graph {
            self.outgoing_rows.clear();
            self.backlink_rows.clear();
            self.graph_loading = false;
        }
        if self.split_list == 0.0 {
            self.split_list = 0.32;
        }
    }

    pub fn expand_ancestors(&mut self, path: &str) {
        crate::model::expand_ancestors_into(&mut self.expanded, path);
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
        let mut state = Gloss::default();
        state.split_list = 0.32;
        state.reader_mode = true;
        state.status = "Browse a vault folder".into();
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
}

impl Gloss {
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
