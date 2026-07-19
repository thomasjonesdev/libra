//! Pure domain layer (§5.9 module layout): data types, derivation helpers, and
//! the versioned store format. Nothing here knows about actions, effects, or
//! the view — plain Rust the unit tests exercise without a Harness.
//!
//! Architecture: ARCHITECTURE.md §5.9 (module layout), §6.3 (projections).

use libra_core::LibraReflect;
use serde::{Deserialize, Serialize};
use std::fmt;

pub const STORE_KEY: &str = "notes/store";
/// Sentinel `selected_folder` value for the "Recently Deleted" view —
/// real folder ids start at 1, so 0 never collides with one.
pub const RECENTLY_DELETED: u32 = 0;
pub const AUTOSAVE_DEBOUNCE_MS: u64 = 600;
pub const MAX_FOLDERS: usize = 6;
pub const MAX_NOTES: usize = 48;
pub const MAX_BODY_BYTES: usize = 4096;

// ---------------------------------------------------------------------
// Source-of-truth model (skipped from reflection — the view binds the
// projection rows below instead).
// ---------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Folder {
    pub id: u32,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub id: u32,
    pub folder_id: u32,
    pub body: String,
    pub created_ms: u64,
    pub updated_ms: u64,
    /// Soft delete (§: Recently Deleted). `None` = live.
    pub deleted_ms: Option<u64>,
}

/// Which modal is up (M6). Not reflected — the view binds the
/// projected `show_*`/`dialog_title` fields instead.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum DialogKind {
    NewFolder,
    RenameFolder,
    ConfirmDelete,
}

// ---------------------------------------------------------------------
// Display projections (§6.3) — structured `<for>` rows.
// ---------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, LibraReflect)]
pub struct FolderRow {
    pub id: u32,
    pub name: String,
    /// Live (non-trashed) note count in this folder.
    pub count: u32,
    /// This folder is the current view — `selected={f.selected}` wash.
    pub selected: bool,
}

impl fmt::Display for FolderRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, LibraReflect)]
pub struct NoteRow {
    pub id: u32,
    /// First non-empty line of `body` (or "Untitled").
    pub title: String,
    /// Collapsed, truncated remainder after the title line.
    pub snippet: String,
    /// "Edited 2m ago · 42 words" — recomputed on every `<Interval>` tick.
    pub meta: String,
    /// This note is open in the editor — `selected={n.selected}` wash.
    pub selected: bool,
}

impl fmt::Display for NoteRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

// ---------------------------------------------------------------------
// Versioned persistence (whole store, one Kv key).
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreV2 {
    pub version: u32,
    pub folders: Vec<Folder>,
    pub notes: Vec<Note>,
    pub next_folder_id: u32,
    pub next_note_id: u32,
}

/// A hypothetical predecessor shape (exercised by the migration test):
/// notes carried an explicit `title` separate from `body`. v2 merges
/// them — title is derived from `body`'s first line instead.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteV1 {
    pub id: u32,
    pub folder_id: u32,
    pub title: String,
    pub body: String,
    pub created_ms: u64,
    pub updated_ms: u64,
    pub deleted_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreV1 {
    pub folders: Vec<Folder>,
    pub notes: Vec<NoteV1>,
}

/// Tries the current shape first (has a `version` field V1 lacks), so
/// the common case never pays for the fallback attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StoredFormat {
    V2(StoreV2),
    V1(StoreV1),
}

pub fn migrate_v1(v1: StoreV1) -> StoreV2 {
    let notes: Vec<Note> = v1
        .notes
        .into_iter()
        .map(|n| {
            let title = n.title.trim();
            let body = if title.is_empty() || n.body.trim_start().starts_with(title) {
                n.body
            } else if n.body.is_empty() {
                title.to_string()
            } else {
                format!("{title}\n{}", n.body)
            };
            Note {
                id: n.id,
                folder_id: n.folder_id,
                body,
                created_ms: n.created_ms,
                updated_ms: n.updated_ms,
                deleted_ms: n.deleted_ms,
            }
        })
        .collect();
    let next_note_id = notes.iter().map(|n| n.id).max().map_or(1, |m| m + 1);
    let next_folder_id = v1.folders.iter().map(|f| f.id).max().map_or(1, |m| m + 1);
    StoreV2 { version: 2, folders: v1.folders, notes, next_folder_id, next_note_id }
}

// ---------------------------------------------------------------------
// Derivation helpers (pure functions — §6.3's "put it on State", the
// actual computation lives here so it's unit-testable without a
// Harness).
// ---------------------------------------------------------------------

pub fn note_title(n: &Note) -> String {
    n.body
        .lines()
        .find(|l| !l.trim().is_empty())
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .unwrap_or("Untitled")
        .to_string()
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let head: String = s.chars().take(max).collect();
        format!("{head}…")
    }
}

pub fn note_snippet(n: &Note) -> String {
    let mut lines = n.body.lines().filter(|l| !l.trim().is_empty());
    lines.next(); // the title line
    let rest = lines.collect::<Vec<_>>().join(" ");
    let collapsed = rest.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate_chars(&collapsed, 80)
}

fn relative_label(now_ms: u64, then_ms: u64) -> String {
    let secs = now_ms.saturating_sub(then_ms) / 1000;
    if secs < 60 {
        "just now".to_string()
    } else if secs < 3_600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86_400 {
        format!("{}h ago", secs / 3_600)
    } else if secs < 604_800 {
        format!("{}d ago", secs / 86_400)
    } else {
        format!("{}w ago", secs / 604_800)
    }
}

pub fn note_meta(now_ms: u64, n: &Note) -> String {
    let words = n.body.split_whitespace().count();
    let word_label = if words == 1 { "word" } else { "words" };
    format!("Edited {} · {words} {word_label}", relative_label(now_ms, n.updated_ms))
}

pub fn clip_bytes(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }
    let mut end = max_bytes;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}
