//! Actions for the Gloss shell (§5.9 module layout).
//!
//! The macro generates the `Action` enum and `reduce` in this module;
//! `lib.rs` re-exports them for a flat `gloss::{Action, reduce}` surface.

mod internal;

use libra_core::{Effect, FolderPick, ProcessResult};

use crate::model::{
    apply_replace_cmd, daily_note_content, default_daily_note_path, insert_wikilink_mention,
    is_folder_id, parse_workspace_bytes, resolve_daily_note, resolve_wikilink, split_note,
    today_local_iso, unique_folder_note, unique_inbox_path, upsert_vault, SidebarView, IoKind,
    Pending, VaultRecord,
};
use crate::state::Gloss;

#[libra::actions]
impl Gloss {
    #[project]
    fn project(&mut self) {
        self.rebuild_rows();
    }

    // ---- vault ----

    pub fn workspace_loaded(&mut self, bytes: Option<Vec<u8>>) -> Option<Effect<Action>> {
        if let Some(raw) = bytes {
            self.workspace = parse_workspace_bytes(&raw);
            if let Some(rec) = self
                .workspace
                .vaults
                .iter()
                .find(|v| v.path == self.workspace.last_path)
                .cloned()
                .or_else(|| self.workspace.vaults.first().cloned())
            {
                self.apply_vault_record(&rec);
                return self.open_list();
            }
        }
        None
    }

    pub fn vault_path_changed(&mut self, text: String) {
        self.vault_path = text;
        if self.vault_path.trim() != self.opened_root {
            self.can_undo = false;
        }
    }

    pub fn browse_vault(&mut self) -> Effect<Action> {
        Effect::pick_folder(Action::FolderPicked)
    }

    pub fn open_vault_picker(&mut self) {
        self.show_vault_picker = true;
    }

    pub fn close_vault_picker(&mut self) {
        self.show_vault_picker = false;
    }

    pub fn folder_picked(&mut self, pick: FolderPick) -> Vec<Effect<Action>> {
        match pick {
            FolderPick::Cancelled => Vec::new(),
            FolderPick::Picked(path) => {
                self.vault_path = path;
                self.show_vault_picker = false;
                self.open_vault()
            }
        }
    }

    pub fn open_vault(&mut self) -> Vec<Effect<Action>> {
        self.snapshot_open_vault();
        let path = self.vault_path.trim().to_string();
        let rec = self
            .workspace
            .vaults
            .iter()
            .find(|v| v.path == path)
            .cloned()
            .unwrap_or_else(|| VaultRecord::from_path(&path));
        self.apply_vault_record(&rec);
        upsert_vault(&mut self.workspace, rec);
        let mut out = vec![self.persist_workspace()];
        if let Some(list) = self.open_list() {
            out.push(list);
        }
        out
    }

    pub fn switch_vault(&mut self, path: String) -> Vec<Effect<Action>> {
        let path = path.trim().to_string();
        self.show_vault_picker = false;
        if path.is_empty() || path == self.opened_root {
            return Vec::new();
        }
        self.snapshot_open_vault();
        let rec = self
            .workspace
            .vaults
            .iter()
            .find(|v| v.path == path)
            .cloned()
            .unwrap_or_else(|| VaultRecord::from_path(&path));
        self.apply_vault_record(&rec);
        upsert_vault(&mut self.workspace, rec);
        self.clear_session_notes();
        let mut out = vec![self.persist_workspace()];
        if let Some(list) = self.open_list() {
            out.push(list);
        }
        out
    }

    pub fn close_vault(&mut self) -> Effect<Action> {
        self.snapshot_open_vault();
        self.workspace.last_path.clear();
        self.clear_session_notes();
        self.vault_path.clear();
        self.opened_root.clear();
        self.can_undo = false;
        self.persist_workspace()
    }

    pub fn remove_vault(&mut self, path: String) -> Vec<Effect<Action>> {
        self.workspace.vaults.retain(|v| v.path != path);
        if self.workspace.last_path == path {
            self.workspace.last_path.clear();
        }
        if self.opened_root == path {
            return vec![self.close_vault()];
        }
        vec![self.persist_workspace()]
    }

    pub fn file_context(&mut self, id: String) {
        self.context_path = id;
    }

    // ---- sidebar ----

    pub fn query_changed(&mut self, text: String) {
        self.query = text;
    }

    pub fn clear_query(&mut self) {
        self.query.clear();
    }

    pub fn select_file(&mut self, id: String) -> Option<Effect<Action>> {
        if id.is_empty() {
            return None;
        }
        if is_folder_id(&id, &self.note_paths) {
            if self.expanded.contains(&id) {
                self.expanded.remove(&id);
            } else {
                self.expanded.insert(id);
            }
            return None;
        }
        self.select_note(id)
    }

    pub fn list_split_changed(&mut self, value: String) {
        if let Ok(v) = value.parse::<f32>() {
            let v = v.clamp(0.18, 0.55);
            self.split_list = v;
            if self.sidebar_open {
                self.split_list_saved = v;
            }
        }
    }

    pub fn show_files_view(&mut self) {
        self.sidebar_view = SidebarView::Files;
    }

    pub fn show_search_view(&mut self) {
        self.sidebar_view = SidebarView::Search;
    }

    pub fn show_bookmarks_view(&mut self) {
        self.sidebar_view = SidebarView::Bookmarks;
    }

    pub fn toggle_sidebar(&mut self) {
        if self.sidebar_open {
            self.split_list_saved = self.split_list;
            self.sidebar_open = false;
            self.split_list = 0.18;
        } else {
            self.sidebar_open = true;
            self.split_list = self.split_list_saved.clamp(0.18, 0.55);
        }
    }

    pub fn toggle_sort(&mut self) {
        self.sort_alpha = !self.sort_alpha;
    }

    pub fn collapse_all(&mut self) {
        self.expanded.clear();
    }

    // ---- notes ----

    pub fn select_note(&mut self, id: String) -> Option<Effect<Action>> {
        if id.is_empty() {
            return None;
        }
        if id == self.selected_path && self.io != IoKind::Cat {
            return None;
        }
        if self.dirty && !self.selected_path.is_empty() && id != self.selected_path {
            self.pending = Pending::Cat(id);
            return self.flush_save();
        }
        self.begin_cat(id)
    }

    pub fn follow_wikilink(&mut self, target: String) -> Option<Effect<Action>> {
        match resolve_wikilink(&target, &self.note_paths) {
            Some(path) => self.select_note(path),
            None => {
                self.status = format!("No note matching '{target}'");
                None
            }
        }
    }

    pub fn body_changed(&mut self, text: String) -> Vec<Effect<Action>> {
        if self.selected_path.is_empty() {
            return Vec::new();
        }
        self.editor_body = text;
        self.sync_mention();
        self.mark_edited();
        self.schedule_save()
    }

    pub fn frontmatter_changed(&mut self, text: String) -> Vec<Effect<Action>> {
        if self.selected_path.is_empty() || !self.reader_mode {
            return Vec::new();
        }
        self.frontmatter_yaml = text;
        self.has_frontmatter = !self.frontmatter_yaml.trim().is_empty();
        self.mark_edited();
        self.schedule_save()
    }

    pub fn flush_save(&mut self) -> Option<Effect<Action>> {
        if !self.save_pending || !self.dirty || self.selected_path.is_empty() {
            return None;
        }
        if self.is_busy() {
            return None;
        }
        self.save_pending = false;
        self.save_timer = None;
        self.in_flight_body = self.full_note_text();
        let root = self.vault_path.trim();
        let cmd = match apply_replace_cmd(
            root,
            &self.selected_path,
            &self.loaded_body,
            &self.in_flight_body,
        ) {
            Ok(c) => c,
            Err(e) => {
                self.status = e;
                return None;
            }
        };
        self.status = "Saving…".into();
        self.spawn(cmd, IoKind::Apply)
    }

    pub fn new_note(&mut self) -> Option<Effect<Action>> {
        self.create_note(unique_inbox_path(&self.note_paths), "# Untitled\n\n")
    }

    pub fn new_folder(&mut self) -> Option<Effect<Action>> {
        self.create_note(unique_folder_note(&self.note_paths), "# Untitled\n\n")
    }

    pub fn open_daily_note(&mut self) -> Option<Effect<Action>> {
        if !self.require_vault("Open a vault first") {
            return None;
        }
        let date = self
            .daily_date_override
            .clone()
            .unwrap_or_else(today_local_iso);
        if let Some(path) = resolve_daily_note(&date, &self.note_paths) {
            return self.select_note(path);
        }
        let path = default_daily_note_path(&date);
        self.create_note(path, &daily_note_content(&date))
    }

    pub fn toggle_reader_mode(&mut self) {
        if self.reader_mode {
            self.editor_body = self.full_note_text();
            self.has_frontmatter = false;
            self.frontmatter_yaml.clear();
            self.frontmatter_fields.clear();
            self.frontmatter_title.clear();
            self.has_frontmatter_title = false;
            self.reader_mode = false;
            return;
        }
        let (yaml, body, has) = split_note(&self.editor_body);
        self.frontmatter_yaml = yaml;
        self.has_frontmatter = has;
        self.editor_body = body;
        self.reader_mode = true;
    }

    pub fn undo(&mut self) -> Option<Effect<Action>> {
        if !self.require_vault("Open a vault first") {
            return None;
        }
        if self.vault_path.trim() != self.opened_root {
            self.can_undo = false;
            self.status = "Open this vault first".into();
            return None;
        }
        if !self.can_undo {
            self.status = "Nothing to undo".into();
            return None;
        }
        if self.is_busy() {
            self.pending = Pending::Undo;
            return None;
        }
        self.status = "Undoing…".into();
        self.spawn(crate::model::undo_cmd(self.vault_path.trim()), IoKind::Undo)
    }

    // ---- mentions ----

    pub fn close_mention(&mut self) {
        self.clear_mention();
    }

    pub fn mention_query_changed(&mut self, text: String) {
        self.mention_query = text;
    }

    pub fn mention_pick(&mut self, id: String) -> Vec<Effect<Action>> {
        if id.is_empty() {
            return Vec::new();
        }
        let stem = crate::model::file_stem(&id);
        let query_len = self.mention_query.len();
        let at = self.mention_at;
        let body = insert_wikilink_mention(&self.editor_body, at, query_len, &stem);
        self.clear_mention();
        self.body_changed(body)
    }

    // ---- command palette ----

    pub fn open_command_palette(&mut self) {
        if !self.require_vault("Open a vault first") {
            return;
        }
        self.show_command_palette = true;
        self.palette_query.clear();
    }

    pub fn close_command_palette(&mut self) {
        self.show_command_palette = false;
        self.palette_query.clear();
    }

    pub fn palette_query_changed(&mut self, text: String) {
        self.palette_query = text;
    }

    pub fn palette_pick(&mut self, id: String) -> Option<Effect<Action>> {
        self.close_command_palette();
        self.select_note(id)
    }

    // ---- graph ----

    pub fn open_graph(&mut self) -> Option<Effect<Action>> {
        if self.selected_path.is_empty() {
            self.status = "Select a note first".into();
            return None;
        }
        self.show_graph = true;
        self.scan_backlinks()
    }

    pub fn close_graph(&mut self) {
        self.show_graph = false;
        self.graph_loading = false;
        self.backlink_rows.clear();
    }

    pub fn graph_pick(&mut self, id: String) -> Option<Effect<Action>> {
        self.close_graph();
        self.select_note(id)
    }

    // ---- tabs & history ----

    pub fn go_back(&mut self) -> Option<Effect<Action>> {
        self.navigate_history(true)
    }

    pub fn go_forward(&mut self) -> Option<Effect<Action>> {
        self.navigate_history(false)
    }

    pub fn new_tab(&mut self) {
        self.open_new_tab = true;
        self.status = "Next note opens in a new tab".into();
    }

    pub fn open_in_new_tab(&mut self, id: String) -> Option<Effect<Action>> {
        if id.is_empty() || is_folder_id(&id, &self.note_paths) {
            return None;
        }
        self.open_new_tab = true;
        self.select_note(id)
    }

    pub fn close_tab(&mut self, id: String) -> Option<Effect<Action>> {
        if id.is_empty() {
            return None;
        }
        self.open_tabs.retain(|p| p != &id);
        self.history_back.retain(|p| p != &id);
        self.history_forward.retain(|p| p != &id);
        if self.selected_path != id {
            return None;
        }
        if let Some(next) = self.open_tabs.last().cloned() {
            self.history_lock = true;
            self.select_note(next)
        } else {
            self.clear_selection();
            None
        }
    }

    // ---- overlays ----

    pub fn dismiss(&mut self) {
        if self.show_vault_picker {
            self.close_vault_picker();
        } else if self.show_command_palette {
            self.close_command_palette();
        } else if self.show_mention {
            self.clear_mention();
        } else if self.show_graph {
            self.close_graph();
        } else if self.show_settings {
            self.close_settings();
        } else if self.toast_ok || self.toast_err {
            self.dismiss_toast();
        }
    }

    pub fn open_settings(&mut self) {
        self.show_settings = true;
    }

    pub fn close_settings(&mut self) {
        self.show_settings = false;
    }

    pub fn dismiss_toast(&mut self) {
        self.toast_ok = false;
        self.toast_err = false;
        self.toast_title.clear();
    }

    pub fn dismiss_toast_timer(&mut self) {
        self.dismiss_toast();
    }

    // ---- I/O ----

    pub fn io_done(&mut self, result: ProcessResult) -> Option<Effect<Action>> {
        self.handle_io_done(result)
    }
}
