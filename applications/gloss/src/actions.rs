//! Actions for the Gloss shell.

use libra_core::{Effect, FolderPick, ProcessResult, ProcessStatus};

use crate::model::{
    apply_create_cmd, apply_replace_cmd, backlinks_cmd, cat_cmd, daily_note_content,
    default_daily_note_path, insert_wikilink_mention, is_folder_id, mention_trigger, parse_list,
    parse_workspace_bytes, resolve_daily_note, rules_cmd, split_note, today_local_iso, undo_cmd,
    unique_folder_note, unique_inbox_path, upsert_vault, IoKind, NoteRow, Pending, VaultRecord,
    AUTOSAVE_MS, STORE_KEY, TOAST_MS,
};
use crate::state::Gloss;

#[libra::actions]
impl Gloss {
    #[project]
    fn project(&mut self) {
        self.rebuild_rows();
    }

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
                apply_record(self, &rec);
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
        snapshot_open_vault(self);
        let path = self.vault_path.trim().to_string();
        let rec = self
            .workspace
            .vaults
            .iter()
            .find(|v| v.path == path)
            .cloned()
            .unwrap_or_else(|| VaultRecord::from_path(&path));
        apply_record(self, &rec);
        upsert_vault(&mut self.workspace, rec);
        let mut out = vec![persist_workspace(self)];
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
        snapshot_open_vault(self);
        let rec = self
            .workspace
            .vaults
            .iter()
            .find(|v| v.path == path)
            .cloned()
            .unwrap_or_else(|| VaultRecord::from_path(&path));
        apply_record(self, &rec);
        upsert_vault(&mut self.workspace, rec);
        clear_session_notes(self);
        let mut out = vec![persist_workspace(self)];
        if let Some(list) = self.open_list() {
            out.push(list);
        }
        out
    }

    pub fn close_vault(&mut self) -> Effect<Action> {
        snapshot_open_vault(self);
        self.workspace.last_path.clear();
        clear_session_notes(self);
        self.vault_path.clear();
        self.opened_root.clear();
        self.can_undo = false;
        persist_workspace(self)
    }

    pub fn remove_vault(&mut self, path: String) -> Vec<Effect<Action>> {
        self.workspace.vaults.retain(|v| v.path != path);
        if self.workspace.last_path == path {
            self.workspace.last_path.clear();
        }
        if self.opened_root == path {
            return vec![self.close_vault()];
        }
        vec![persist_workspace(self)]
    }

    pub fn file_context(&mut self, id: String) {
        self.context_path = id;
    }

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
        match crate::model::resolve_wikilink(&target, &self.note_paths) {
            Some(path) => self.select_note(path),
            None => {
                self.status = format!("No note matching '{target}'");
                None
            }
        }
    }

    pub fn body_changed(&mut self, text: String) -> Option<Effect<Action>> {
        if self.selected_path.is_empty() {
            return None;
        }
        self.editor_body = text;
        self.sync_mention();
        self.dirty = self.full_note_text() != self.loaded_body;
        self.save_pending = true;
        Some(Effect::timer(AUTOSAVE_MS, Action::FlushSave))
    }

    fn sync_mention(&mut self) {
        if let Some((at, query)) = mention_trigger(&self.editor_body) {
            self.show_mention = true;
            self.mention_at = at;
            self.mention_query = query;
        } else {
            self.show_mention = false;
            self.mention_at = 0;
            self.mention_query.clear();
        }
    }

    pub fn close_mention(&mut self) {
        self.show_mention = false;
        self.mention_at = 0;
        self.mention_query.clear();
    }

    pub fn mention_query_changed(&mut self, text: String) {
        self.mention_query = text;
    }

    pub fn mention_pick(&mut self, id: String) -> Option<Effect<Action>> {
        if id.is_empty() {
            return None;
        }
        let stem = crate::model::file_stem(&id);
        let query_len = self.mention_query.len();
        let at = self.mention_at;
        let body = insert_wikilink_mention(&self.editor_body, at, query_len, &stem);
        self.close_mention();
        self.body_changed(body)
    }

    pub fn dismiss(&mut self) {
        if self.show_vault_picker {
            self.close_vault_picker();
            return;
        }
        if self.show_command_palette {
            self.close_command_palette();
            return;
        }
        if self.show_mention {
            self.close_mention();
            return;
        }
        if self.show_graph {
            self.close_graph();
            return;
        }
        if self.show_settings {
            self.close_settings();
            return;
        }
        if self.toast_ok || self.toast_err {
            self.dismiss_toast();
        }
    }

    pub fn flush_save(&mut self) -> Option<Effect<Action>> {
        if !self.save_pending || !self.dirty {
            return None;
        }
        if self.is_busy() {
            return None;
        }
        self.save_pending = false;
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

    pub fn open_daily_note(&mut self) -> Option<Effect<Action>> {
        if self.vault_path.trim().is_empty() {
            self.status = "Open a vault first".into();
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

    pub fn open_command_palette(&mut self) {
        if self.vault_path.trim().is_empty() {
            self.status = "Open a vault first".into();
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

    pub fn new_folder(&mut self) -> Option<Effect<Action>> {
        self.create_note(unique_folder_note(&self.note_paths), "# Untitled\n\n")
    }

    pub fn undo(&mut self) -> Option<Effect<Action>> {
        if self.vault_path.trim().is_empty() {
            self.status = "Open a vault first".into();
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
        self.spawn(undo_cmd(self.vault_path.trim()), IoKind::Undo)
    }

    pub fn list_split_changed(&mut self, value: String) {
        if let Ok(v) = value.parse::<f32>() {
            self.split_list = v.clamp(0.18, 0.55);
        }
    }

    pub fn toggle_sort(&mut self) {
        self.sort_alpha = !self.sort_alpha;
    }

    pub fn collapse_all(&mut self) {
        self.expanded.clear();
    }

    pub fn frontmatter_changed(&mut self, text: String) -> Option<Effect<Action>> {
        if self.selected_path.is_empty() || !self.reader_mode {
            return None;
        }
        self.frontmatter_yaml = text;
        self.has_frontmatter = !self.frontmatter_yaml.trim().is_empty();
        self.dirty = self.full_note_text() != self.loaded_body;
        self.save_pending = true;
        Some(Effect::timer(AUTOSAVE_MS, Action::FlushSave))
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

    pub fn go_back(&mut self) -> Option<Effect<Action>> {
        let Some(prev) = self.history_back.pop() else {
            self.status = "Nothing to go back to".into();
            return None;
        };
        if !self.selected_path.is_empty() {
            self.history_forward.push(self.selected_path.clone());
        }
        self.history_lock = true;
        self.select_note(prev)
    }

    pub fn go_forward(&mut self) -> Option<Effect<Action>> {
        let Some(next) = self.history_forward.pop() else {
            self.status = "Nothing to go forward to".into();
            return None;
        };
        if !self.selected_path.is_empty() {
            self.history_back.push(self.selected_path.clone());
        }
        self.history_lock = true;
        self.select_note(next)
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

    pub fn io_done(&mut self, result: ProcessResult) -> Option<Effect<Action>> {
        let kind = self.io;
        self.io = IoKind::Idle;
        if result.status != ProcessStatus::Exited(0) {
            let err = String::from_utf8_lossy(&result.stderr);
            self.status = if err.trim().is_empty() {
                format!("gloss failed ({:?})", result.status)
            } else {
                err.trim().to_string()
            };
            if kind == IoKind::Apply {
                self.save_pending = true;
                if matches!(self.pending, Pending::Cat(_)) {
                    self.pending = Pending::None;
                }
            }
            if kind == IoKind::Backlinks {
                self.graph_loading = false;
            }
            if kind == IoKind::Rules {
                self.rules_loaded = true;
            }
            return self.flash_toast(false, "Failed");
        }
        let mut toast_fx = None;
        let follow = match kind {
            IoKind::List => {
                self.note_paths = parse_list(&result.stdout);
                self.status = format!("{} notes", self.note_paths.len());
                if let Pending::Cat(ref path) = self.pending {
                    if !self.note_paths.iter().any(|p| p == path) {
                        if self.selected_path == *path {
                            self.clear_selection();
                        }
                        self.pending = Pending::None;
                    }
                }
                if !self.selected_path.is_empty()
                    && !self.note_paths.iter().any(|p| p == &self.selected_path)
                {
                    self.clear_selection();
                }
                self.open_tabs
                    .retain(|p| self.note_paths.iter().any(|n| n == p));
                let restore = std::mem::take(&mut self.restore_note);
                if self.selected_path.is_empty() {
                    let next =
                        if !restore.is_empty() && self.note_paths.iter().any(|p| p == &restore) {
                            Some(restore)
                        } else {
                            self.note_paths.first().cloned()
                        };
                    if let Some(path) = next {
                        self.pending = Pending::Rules;
                        self.begin_cat(path)
                    } else {
                        self.begin_load_rules()
                    }
                } else {
                    self.begin_load_rules()
                }
            }
            IoKind::Cat => {
                let body = String::from_utf8_lossy(&result.stdout).into_owned();
                self.loaded_body = body.clone();
                self.set_note_text(&body);
                self.dirty = false;
                self.save_pending = false;
                self.sync_mention();
                self.status = format!("{} notes", self.note_paths.len());
                self.begin_load_rules()
            }
            IoKind::Apply => {
                let saved = self.in_flight_body.clone();
                self.loaded_body = saved.clone();
                self.set_note_text(&saved);
                self.dirty = false;
                self.can_undo = true;
                self.status = "Saved".into();
                toast_fx = self.flash_toast(true, "Saved");
                if !self.note_paths.iter().any(|p| p == &self.selected_path) {
                    self.note_paths.push(self.selected_path.clone());
                    self.note_paths.sort();
                }
                None
            }
            IoKind::Undo => {
                self.can_undo = false;
                self.status = "Undone".into();
                toast_fx = self.flash_toast(true, "Undone");
                if !self.selected_path.is_empty() {
                    self.pending = Pending::Cat(self.selected_path.clone());
                }
                self.open_list()
            }
            IoKind::Backlinks => {
                let paths = parse_list(&result.stdout);
                self.backlink_rows = paths
                    .into_iter()
                    .map(|p| NoteRow {
                        id: p.clone(),
                        title: crate::model::file_stem(&p),
                        snippet: p,
                        selected: false,
                    })
                    .collect();
                self.graph_loading = false;
                self.status = format!("{} backlink(s)", self.backlink_rows.len());
                None
            }
            IoKind::Rules => {
                self.rules_loaded = true;
                self.rules_body = String::from_utf8_lossy(&result.stdout).into_owned();
                None
            }
            IoKind::Idle => None,
        };
        if follow.is_some() {
            return follow;
        }
        if let Some(fx) = self.after_io() {
            return Some(fx);
        }
        if toast_fx.is_some() {
            return toast_fx;
        }
        None
    }
}

impl Gloss {
    fn flash_toast(&mut self, ok: bool, title: &str) -> Option<Effect<Action>> {
        self.toast_ok = ok;
        self.toast_err = !ok;
        self.toast_title = title.into();
        Some(Effect::timer(TOAST_MS, Action::DismissToastTimer))
    }

    fn begin_load_rules(&mut self) -> Option<Effect<Action>> {
        if self.rules_loaded || self.vault_path.trim().is_empty() {
            return None;
        }
        if self.is_busy() {
            self.pending = Pending::Rules;
            return None;
        }
        self.spawn(rules_cmd(self.vault_path.trim()), IoKind::Rules)
    }

    fn scan_backlinks(&mut self) -> Option<Effect<Action>> {
        if self.selected_path.is_empty() {
            return None;
        }
        if self.is_busy() {
            self.pending = Pending::Backlinks;
            return None;
        }
        self.graph_loading = true;
        self.backlink_rows.clear();
        self.status = "Scanning backlinks…".into();
        self.spawn(
            backlinks_cmd(self.vault_path.trim(), &self.selected_path),
            IoKind::Backlinks,
        )
    }

    fn create_note(&mut self, path: String, content: &str) -> Option<Effect<Action>> {
        if self.vault_path.trim().is_empty() {
            self.status = "Open a vault first".into();
            return None;
        }
        let content = content.to_string();
        let cmd = match apply_create_cmd(self.vault_path.trim(), &path, &content) {
            Ok(c) => c,
            Err(e) => {
                self.status = e;
                return None;
            }
        };
        if self.is_busy() {
            self.status = "Wait for the current save".into();
            return None;
        }
        self.push_history(&path);
        self.place_in_tab(&path);
        self.selected_path = path.clone();
        self.loaded_body = content.clone();
        self.set_note_text(&content);
        self.in_flight_body = self.full_note_text();
        self.dirty = false;
        self.save_pending = false;
        self.expand_ancestors(&path);
        self.status = "Creating…".into();
        self.spawn(cmd, IoKind::Apply)
    }

    fn clear_selection(&mut self) {
        self.selected_path.clear();
        self.editor_body.clear();
        self.loaded_body.clear();
        self.frontmatter_yaml.clear();
        self.frontmatter_fields.clear();
        self.frontmatter_title.clear();
        self.has_frontmatter = false;
        self.has_frontmatter_title = false;
        self.dirty = false;
    }

    fn push_history(&mut self, next: &str) {
        if self.history_lock {
            self.history_lock = false;
            return;
        }
        if self.selected_path.is_empty() || self.selected_path == next {
            return;
        }
        self.history_back.push(self.selected_path.clone());
        self.history_forward.clear();
    }

    fn begin_cat(&mut self, id: String) -> Option<Effect<Action>> {
        if self.is_busy() {
            self.pending = Pending::Cat(id);
            return None;
        }
        self.push_history(&id);
        self.place_in_tab(&id);
        self.selected_path = id.clone();
        self.expand_ancestors(&id);
        self.status = "Opening…".into();
        self.spawn(cat_cmd(self.vault_path.trim(), &id), IoKind::Cat)
    }

    fn after_io(&mut self) -> Option<Effect<Action>> {
        match std::mem::take(&mut self.pending) {
            Pending::None => {
                if self.save_pending && self.dirty {
                    self.flush_save()
                } else {
                    None
                }
            }
            Pending::List => self.open_list(),
            Pending::Cat(path) => self.begin_cat(path),
            Pending::Undo => self.undo(),
            Pending::Backlinks => self.scan_backlinks(),
            Pending::Rules => self.begin_load_rules(),
        }
    }
}

fn persist_workspace(g: &Gloss) -> Effect<Action> {
    Effect::kv_set(
        STORE_KEY,
        serde_json::to_vec(&g.workspace).unwrap_or_else(|_| b"{}".to_vec()),
    )
}

fn snapshot_open_vault(g: &mut Gloss) {
    let path = g.opened_root.trim();
    if path.is_empty() {
        return;
    }
    upsert_vault(
        &mut g.workspace,
        VaultRecord {
            path: path.to_string(),
            name: crate::model::vault_name(path),
            last_note: g.selected_path.clone(),
            sort_alpha: g.sort_alpha,
            split_list: g.split_list,
            reader_mode: g.reader_mode,
        },
    );
}

fn apply_record(g: &mut Gloss, rec: &VaultRecord) {
    g.vault_path = rec.path.clone();
    g.opened_root = rec.path.clone();
    g.sort_alpha = rec.sort_alpha;
    g.split_list = rec.split_list;
    g.reader_mode = rec.reader_mode;
    g.restore_note = rec.last_note.clone();
    g.can_undo = false;
    g.rules_loaded = false;
    g.rules_body.clear();
}

fn clear_session_notes(g: &mut Gloss) {
    g.note_paths.clear();
    g.open_tabs.clear();
    g.history_back.clear();
    g.history_forward.clear();
    g.context_path.clear();
    g.clear_selection();
}
