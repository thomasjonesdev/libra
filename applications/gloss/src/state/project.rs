//! Display projections rebuilt by the `#[project]` hook.

use crate::model::{
    frontmatter_scan_rows, frontmatter_title, project_bookmark_rows, project_file_rows,
    project_link_rows, project_palette_rows, save_busy, save_ok, vault_name, word_count_label,
    char_count_label, IoKind, NoteRow, SidebarView, TabRow,
};
use crate::state::Gloss;

impl Gloss {
    pub fn rebuild_rows(&mut self) {
        let full = self.full_note_text();
        self.project_note_rows(&full);
        self.project_sidebar_rows();
        self.project_tabs();
        self.project_chrome(&full);
        self.project_overlays();
        self.project_graph(&full);
        if self.split_list == 0.0 {
            self.split_list = 0.32;
        }
    }

    fn project_note_rows(&mut self, full: &str) {
        let q = self.query.to_ascii_lowercase();
        self.note_rows = self
            .note_paths
            .iter()
            .filter(|p| q.is_empty() || p.to_ascii_lowercase().contains(&q))
            .map(|p| {
                let selected = p == &self.selected_path;
                let body = if selected { full } else { "" };
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
    }

    fn project_sidebar_rows(&mut self) {
        self.file_rows = if !self.sidebar_open {
            Vec::new()
        } else {
            match self.sidebar_view {
                SidebarView::Files => project_file_rows(
                    &self.note_paths,
                    "",
                    &self.selected_path,
                    &self.expanded,
                    self.sort_alpha,
                ),
                SidebarView::Search => crate::model::project_search_rows(
                    &self.note_paths,
                    &self.query,
                    &self.selected_path,
                ),
                SidebarView::Bookmarks => project_bookmark_rows(
                    &self.bookmarks,
                    &self.note_paths,
                    &self.selected_path,
                ),
            }
        };
        self.sidebar_files = self.sidebar_open && self.sidebar_view == SidebarView::Files;
        self.sidebar_search = self.sidebar_open && self.sidebar_view == SidebarView::Search;
        self.sidebar_bookmarks =
            self.sidebar_open && self.sidebar_view == SidebarView::Bookmarks;
    }

    fn project_tabs(&mut self) {
        self.tab_rows = self
            .open_tabs
            .iter()
            .map(|p| TabRow {
                id: p.clone(),
                title: crate::model::file_stem(p),
                selected: p == &self.selected_path,
            })
            .collect();
    }

    fn project_chrome(&mut self, full: &str) {
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
            crate::model::note_title(&self.selected_path, full)
        };
        self.empty_message = self.empty_sidebar_message();
        self.word_count = word_count_label(full);
        self.char_count = char_count_label(full);
        self.save_busy = save_busy(&self.status);
        self.save_ok = save_ok(self.has_selected, &self.status);
        self.mode_label = if self.reader_mode { "Raw" } else { "Reader" }.into();
        if self.reader_mode {
            self.frontmatter_title = frontmatter_title(&self.frontmatter_yaml);
            self.has_frontmatter_title = !self.frontmatter_title.is_empty();
            self.frontmatter_fields = frontmatter_scan_rows(&self.frontmatter_yaml);
        }
    }

    fn empty_sidebar_message(&self) -> String {
        if self.vault_path.trim().is_empty() {
            return "Browse a vault folder to begin".into();
        }
        if !self.sidebar_open {
            return String::new();
        }
        if self.sidebar_view == SidebarView::Bookmarks && self.bookmarks.is_empty() {
            return "No bookmarks yet".into();
        }
        if self.file_rows.is_empty() {
            return match self.sidebar_view {
                SidebarView::Search if !self.query.trim().is_empty() => "No matching notes".into(),
                SidebarView::Search => "Search notes…".into(),
                _ => "No notes in this vault".into(),
            };
        }
        String::new()
    }

    fn project_overlays(&mut self) {
        self.palette_rows = if self.show_command_palette {
            project_palette_rows(&self.note_paths, &self.palette_query, 50)
        } else {
            Vec::new()
        };
        self.mention_rows = if self.show_mention {
            project_palette_rows(&self.note_paths, &self.mention_query, 50)
        } else {
            Vec::new()
        };
        self.has_rules = !self.rules_body.trim().is_empty();
        self.rules_excerpt = crate::model::rules_preview(&self.rules_body, 480);
    }

    fn project_graph(&mut self, full: &str) {
        if self.show_graph && self.has_selected {
            self.outgoing_rows = project_link_rows(full, &self.note_paths);
        } else if !self.show_graph {
            self.outgoing_rows.clear();
            self.backlink_rows.clear();
            self.graph_loading = false;
        }
    }
}
