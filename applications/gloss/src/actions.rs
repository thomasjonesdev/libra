//! Actions for the Gloss shell.

use libra_core::{Effect, ProcessResult, ProcessStatus};

use crate::model::{
    apply_create_cmd, apply_replace_cmd, cat_cmd, parse_list, undo_cmd, IoKind, Pending,
    AUTOSAVE_MS, STORE_KEY,
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
            if let Ok(path) = String::from_utf8(raw) {
                let path = path.trim().to_string();
                if !path.is_empty() {
                    self.vault_path = path;
                    self.opened_root = self.vault_path.clone();
                    return self.open_list();
                }
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

    pub fn open_vault(&mut self) -> Vec<Effect<Action>> {
        self.opened_root = self.vault_path.trim().to_string();
        self.can_undo = false;
        let mut out = Vec::new();
        out.push(Effect::kv_set(
            STORE_KEY,
            self.vault_path.trim().as_bytes().to_vec(),
        ));
        if let Some(list) = self.open_list() {
            out.push(list);
        }
        out
    }

    pub fn query_changed(&mut self, text: String) {
        self.query = text;
    }

    pub fn clear_query(&mut self) {
        self.query.clear();
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

    pub fn body_changed(&mut self, text: String) -> Option<Effect<Action>> {
        if self.selected_path.is_empty() {
            return None;
        }
        self.editor_body = text;
        self.dirty = true;
        self.save_pending = true;
        Some(Effect::timer(AUTOSAVE_MS, Action::FlushSave))
    }

    pub fn flush_save(&mut self) -> Option<Effect<Action>> {
        if !self.save_pending || !self.dirty {
            return None;
        }
        if self.is_busy() {
            return None;
        }
        self.save_pending = false;
        self.in_flight_body = self.editor_body.clone();
        let root = self.vault_path.trim();
        let cmd = match apply_replace_cmd(
            root,
            &self.selected_path,
            &self.loaded_body,
            &self.editor_body,
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
        if self.vault_path.trim().is_empty() {
            self.status = "Open a vault first".into();
            return None;
        }
        let path = unique_inbox_path(&self.note_paths);
        let content = "# Untitled\n\n";
        let cmd = match apply_create_cmd(self.vault_path.trim(), &path, content) {
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
        self.selected_path = path;
        self.loaded_body = content.into();
        self.editor_body = content.into();
        self.in_flight_body = self.editor_body.clone();
        self.dirty = false;
        self.save_pending = false;
        self.status = "Creating…".into();
        self.spawn(cmd, IoKind::Apply)
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
            return None;
        }
        let follow = match kind {
            IoKind::List => {
                self.note_paths = parse_list(&result.stdout);
                self.status = format!("{} notes", self.note_paths.len());
                if let Pending::Cat(ref path) = self.pending {
                    if !self.note_paths.iter().any(|p| p == path) {
                        if self.selected_path == *path {
                            self.selected_path.clear();
                            self.editor_body.clear();
                            self.loaded_body.clear();
                            self.dirty = false;
                        }
                        self.pending = Pending::None;
                    }
                }
                if !self.selected_path.is_empty()
                    && !self.note_paths.iter().any(|p| p == &self.selected_path)
                {
                    self.selected_path.clear();
                    self.editor_body.clear();
                    self.loaded_body.clear();
                    self.dirty = false;
                }
                if self.selected_path.is_empty() {
                    if let Some(first) = self.note_paths.first().cloned() {
                        self.begin_cat(first)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            IoKind::Cat => {
                let body = String::from_utf8_lossy(&result.stdout).into_owned();
                self.loaded_body = body.clone();
                self.editor_body = body;
                self.dirty = false;
                self.save_pending = false;
                self.status = self.selected_path.clone();
                None
            }
            IoKind::Apply => {
                self.loaded_body = self.in_flight_body.clone();
                self.dirty = self.editor_body != self.loaded_body;
                self.can_undo = true;
                self.status = "Saved".into();
                if !self.note_paths.iter().any(|p| p == &self.selected_path) {
                    self.note_paths.push(self.selected_path.clone());
                    self.note_paths.sort();
                }
                None
            }
            IoKind::Undo => {
                self.can_undo = false;
                self.status = "Undone".into();
                if !self.selected_path.is_empty() {
                    self.pending = Pending::Cat(self.selected_path.clone());
                }
                self.open_list()
            }
            IoKind::Idle => None,
        };
        if follow.is_some() {
            return follow;
        }
        self.after_io()
    }
}

impl Gloss {
    fn begin_cat(&mut self, id: String) -> Option<Effect<Action>> {
        if self.is_busy() {
            self.pending = Pending::Cat(id);
            return None;
        }
        self.selected_path = id.clone();
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
        }
    }
}

fn unique_inbox_path(existing: &[String]) -> String {
    if !existing.iter().any(|p| p == "10-Inbox/Untitled.md") {
        return "10-Inbox/Untitled.md".into();
    }
    for i in 2..1000 {
        let p = format!("10-Inbox/Untitled-{i}.md");
        if !existing.iter().any(|e| e == &p) {
            return p;
        }
    }
    "10-Inbox/Untitled-new.md".into()
}
