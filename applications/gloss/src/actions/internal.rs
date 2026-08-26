//! Private action helpers: I/O continuations and editor transitions.

use libra_core::{Effect, ProcessResult, ProcessStatus};

use crate::model::{
    apply_create_cmd, backlinks_cmd, cat_cmd, file_stem, parse_list, rules_cmd, upsert_vault,
    IoKind, NoteRow, Pending, VaultRecord, TOAST_MS,
};
use crate::state::Gloss;

use super::Action;

impl Gloss {
    pub(super) fn flash_toast(&mut self, ok: bool, title: &str) -> Option<Effect<Action>> {
        self.toast_ok = ok;
        self.toast_err = !ok;
        self.toast_title = title.into();
        Some(Effect::timer(TOAST_MS, Action::DismissToastTimer))
    }

    pub(super) fn begin_load_rules(&mut self) -> Option<Effect<Action>> {
        if self.rules_loaded || self.vault_path.trim().is_empty() {
            return None;
        }
        if self.is_busy() {
            self.pending = Pending::Rules;
            return None;
        }
        self.spawn(rules_cmd(self.vault_path.trim()), IoKind::Rules)
    }

    pub(super) fn scan_backlinks(&mut self) -> Option<Effect<Action>> {
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

    pub(super) fn create_note(&mut self, path: String, content: &str) -> Option<Effect<Action>> {
        if !self.require_vault("Open a vault first") {
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
        self.save_timer = None;
        self.expand_ancestors(&path);
        self.status = "Creating…".into();
        self.spawn(cmd, IoKind::Apply)
    }

    pub(super) fn clear_selection(&mut self) {
        self.selected_path.clear();
        self.editor_body.clear();
        self.loaded_body.clear();
        self.frontmatter_yaml.clear();
        self.frontmatter_fields.clear();
        self.frontmatter_title.clear();
        self.has_frontmatter = false;
        self.has_frontmatter_title = false;
        self.dirty = false;
        self.save_pending = false;
        self.save_timer = None;
    }

    pub(super) fn push_history(&mut self, next: &str) {
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

    pub(super) fn begin_cat(&mut self, id: String) -> Option<Effect<Action>> {
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

    pub(super) fn after_io(&mut self) -> Option<Effect<Action>> {
        match std::mem::take(&mut self.pending) {
            Pending::None => {
                if self.save_pending && self.dirty {
                    self.flush_save()
                } else {
                    None
                }
            }
            Pending::List => self.open_list(),
            Pending::Cat(path) => {
                if self.save_pending && self.dirty {
                    self.pending = Pending::Cat(path);
                    self.flush_save()
                } else {
                    self.begin_cat(path)
                }
            }
            Pending::Undo => self.undo(),
            Pending::Backlinks => self.scan_backlinks(),
            Pending::Rules => self.begin_load_rules(),
        }
    }

    pub(super) fn handle_io_done(&mut self, result: ProcessResult) -> Option<Effect<Action>> {
        let kind = self.io;
        self.io = IoKind::Idle;
        if result.status != ProcessStatus::Exited(0) {
            return self.handle_io_error(kind, &result);
        }
        let mut toast_fx = None;
        let follow = match kind {
            IoKind::List => self.finish_list(&result.stdout),
            IoKind::Cat => self.finish_cat(&result.stdout),
            IoKind::Apply => {
                toast_fx = self.finish_apply();
                None
            }
            IoKind::Undo => {
                toast_fx = self.finish_undo();
                self.open_list()
            }
            IoKind::Backlinks => {
                self.finish_backlinks(&result.stdout);
                None
            }
            IoKind::Rules => {
                self.rules_loaded = true;
                self.rules_body = String::from_utf8_lossy(&result.stdout).into_owned();
                None
            }
            IoKind::Idle => None,
        };
        follow.or_else(|| self.after_io()).or(toast_fx)
    }

    fn handle_io_error(&mut self, kind: IoKind, result: &ProcessResult) -> Option<Effect<Action>> {
        let err = String::from_utf8_lossy(&result.stderr);
        self.status = if err.trim().is_empty() {
            format!("gloss failed ({:?})", result.status)
        } else {
            err.trim().to_string()
        };
        match kind {
            IoKind::Apply => {
                self.save_pending = true;
                if matches!(self.pending, Pending::Cat(_)) {
                    self.pending = Pending::None;
                }
            }
            IoKind::Backlinks => self.graph_loading = false,
            IoKind::Rules => self.rules_loaded = true,
            _ => {}
        }
        self.flash_toast(false, "Failed")
    }

    fn finish_list(&mut self, stdout: &[u8]) -> Option<Effect<Action>> {
        self.note_paths = parse_list(stdout);
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
            let next = if !restore.is_empty() && self.note_paths.iter().any(|p| p == &restore) {
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

    fn finish_cat(&mut self, stdout: &[u8]) -> Option<Effect<Action>> {
        let body = String::from_utf8_lossy(stdout).into_owned();
        self.loaded_body = body.clone();
        self.set_note_text(&body);
        self.dirty = false;
        self.save_pending = false;
        self.save_timer = None;
        self.sync_mention();
        self.status = format!("{} notes", self.note_paths.len());
        self.begin_load_rules()
    }

    fn finish_apply(&mut self) -> Option<Effect<Action>> {
        self.loaded_body = self.in_flight_body.clone();
        self.mark_edited();
        self.can_undo = true;
        if self.dirty {
            self.save_pending = true;
        } else {
            self.status = "Saved".into();
        }
        if !self.note_paths.iter().any(|p| p == &self.selected_path) {
            self.note_paths.push(self.selected_path.clone());
            self.note_paths.sort();
        }
        None
    }

    fn finish_undo(&mut self) -> Option<Effect<Action>> {
        self.can_undo = false;
        self.status = "Undone".into();
        if !self.selected_path.is_empty() {
            self.pending = Pending::Cat(self.selected_path.clone());
        }
        self.flash_toast(true, "Undone")
    }

    fn finish_backlinks(&mut self, stdout: &[u8]) {
        let paths = parse_list(stdout);
        self.backlink_rows = paths
            .into_iter()
            .map(|p| NoteRow {
                id: p.clone(),
                title: file_stem(&p),
                snippet: p,
                selected: false,
            })
            .collect();
        self.graph_loading = false;
        self.status = format!("{} backlink(s)", self.backlink_rows.len());
    }

    pub(super) fn require_vault(&mut self, message: &str) -> bool {
        if self.vault_path.trim().is_empty() {
            self.status = message.into();
            false
        } else {
            true
        }
    }

    pub(super) fn schedule_save(&mut self) -> Vec<Effect<Action>> {
        use crate::model::AUTOSAVE_MS;
        self.save_pending = true;
        let mut fx = Vec::new();
        if let Some(id) = self.save_timer.take() {
            fx.push(Effect::CancelTimer {
                id: libra_core::TimerId(id),
            });
        }
        let id = self.alloc_timer();
        self.save_timer = Some(id.0);
        fx.push(Effect::timer_id(id, AUTOSAVE_MS, Action::FlushSave));
        fx
    }

    pub(super) fn mark_edited(&mut self) {
        self.dirty = self.full_note_text() != self.loaded_body;
    }

    pub(super) fn sync_mention(&mut self) {
        use crate::model::mention_trigger;
        if let Some((at, query)) = mention_trigger(&self.editor_body) {
            self.show_mention = true;
            self.mention_at = at;
            self.mention_query = query;
        } else {
            self.clear_mention();
        }
    }

    pub(super) fn clear_mention(&mut self) {
        self.show_mention = false;
        self.mention_at = 0;
        self.mention_query.clear();
    }

    pub(super) fn persist_workspace(&self) -> Effect<Action> {
        Effect::kv_set(
            crate::model::STORE_KEY,
            serde_json::to_vec(&self.workspace).unwrap_or_else(|_| b"{}".to_vec()),
        )
    }

    pub(super) fn snapshot_open_vault(&mut self) {
        let path = self.opened_root.trim();
        if path.is_empty() {
            return;
        }
        upsert_vault(
            &mut self.workspace,
            VaultRecord {
                path: path.to_string(),
                name: crate::model::vault_name(path),
                last_note: self.selected_path.clone(),
                sort_alpha: self.sort_alpha,
                split_list: self.split_list,
                reader_mode: self.reader_mode,
            },
        );
    }

    pub(super) fn apply_vault_record(&mut self, rec: &VaultRecord) {
        self.vault_path = rec.path.clone();
        self.opened_root = rec.path.clone();
        self.sort_alpha = rec.sort_alpha;
        self.split_list = rec.split_list;
        self.reader_mode = rec.reader_mode;
        self.restore_note = rec.last_note.clone();
        self.can_undo = false;
        self.rules_loaded = false;
        self.rules_body.clear();
    }

    pub(super) fn clear_session_notes(&mut self) {
        self.note_paths.clear();
        self.open_tabs.clear();
        self.history_back.clear();
        self.history_forward.clear();
        self.context_path.clear();
        self.clear_selection();
    }

    pub(super) fn navigate_history(&mut self, back: bool) -> Option<Effect<Action>> {
        let (next, other_stack, empty_msg) = if back {
            (
                self.history_back.pop(),
                &mut self.history_forward,
                "Nothing to go back to",
            )
        } else {
            (
                self.history_forward.pop(),
                &mut self.history_back,
                "Nothing to go forward to",
            )
        };
        let Some(next) = next else {
            self.status = empty_msg.into();
            return None;
        };
        if !self.selected_path.is_empty() {
            other_stack.push(self.selected_path.clone());
        }
        self.history_lock = true;
        self.select_note(next)
    }
}
