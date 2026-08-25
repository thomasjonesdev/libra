//! Actions for the Gloss shell.

use libra_core::{Effect, ProcessResult, ProcessStatus};

use crate::model::{
    apply_create_cmd, apply_replace_cmd, cat_cmd, parse_list, undo_cmd, IoKind, AUTOSAVE_MS,
    STORE_KEY,
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
                    return self.open_list();
                }
            }
        }
        None
    }

    pub fn vault_path_changed(&mut self, text: String) {
        self.vault_path = text;
    }

    pub fn open_vault(&mut self) -> Vec<Effect<Action>> {
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
        self.selected_path = id.clone();
        self.status = "Opening…".into();
        Some(self.spawn(cat_cmd(self.vault_path.trim(), &id), IoKind::Cat))
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
        Some(self.spawn(cmd, IoKind::Apply))
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
        self.selected_path = path;
        self.loaded_body = content.into();
        self.editor_body = content.into();
        self.in_flight_body = self.editor_body.clone();
        self.status = "Creating…".into();
        Some(self.spawn(cmd, IoKind::Apply))
    }

    pub fn undo(&mut self) -> Option<Effect<Action>> {
        if self.vault_path.trim().is_empty() {
            self.status = "Open a vault first".into();
            return None;
        }
        if !self.can_undo {
            self.status = "Nothing to undo".into();
            return None;
        }
        self.status = "Undoing…".into();
        Some(self.spawn(undo_cmd(self.vault_path.trim()), IoKind::Undo))
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
            return None;
        }
        match kind {
            IoKind::List => {
                self.note_paths = parse_list(&result.stdout);
                self.status = format!("{} notes", self.note_paths.len());
                if self.selected_path.is_empty() {
                    if let Some(first) = self.note_paths.first().cloned() {
                        return self.select_note(first);
                    }
                }
                None
            }
            IoKind::Cat => {
                let body = String::from_utf8_lossy(&result.stdout).into_owned();
                self.loaded_body = body.clone();
                self.editor_body = body;
                self.dirty = false;
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
                if self.selected_path.is_empty() {
                    return None;
                }
                self.select_note(self.selected_path.clone())
            }
            IoKind::Idle => None,
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
