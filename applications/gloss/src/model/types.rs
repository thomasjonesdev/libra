//! Shared types and row projections for the Gloss shell.

use std::fmt;

use libra_core::LibraReflect;
use serde::{Deserialize, Serialize};

pub const STORE_KEY: &str = "gloss/workspace";
pub const AUTOSAVE_MS: u64 = 800;
pub const DEFAULT_SPLIT: f32 = 0.32;
pub const GLOSS_BIN: &str = "gloss";
pub const RULES_PATH: &str = ".gloss/rules.md";
pub const TOAST_MS: u64 = 2_500;
pub const TREE_INSET: f32 = 16.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum IoKind {
    #[default]
    Idle,
    List,
    Cat,
    Apply,
    Undo,
    Backlinks,
    Rules,
    Rename,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SidebarView {
    #[default]
    Files,
    Search,
    Bookmarks,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Pending {
    #[default]
    None,
    List,
    Cat(String),
    Undo,
    Backlinks,
    Rules,
    Rename(String),
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, LibraReflect)]
pub struct NoteRow {
    pub id: String,
    pub title: String,
    pub snippet: String,
    pub selected: bool,
}

impl fmt::Display for NoteRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.id)
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, LibraReflect)]
pub struct FileRow {
    pub id: String,
    pub title: String,
    pub snippet: String,
    pub selected: bool,
    pub inset: f32,
}

impl fmt::Display for FileRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.id)
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, LibraReflect)]
pub struct FrontmatterField {
    pub key: String,
    pub value: String,
}

impl fmt::Display for FrontmatterField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.key)
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, LibraReflect)]
pub struct TabRow {
    pub id: String,
    pub title: String,
    pub selected: bool,
}

impl fmt::Display for TabRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.id)
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct WorkspaceSettings {
    #[serde(default)]
    pub last_path: String,
    #[serde(default)]
    pub vaults: Vec<VaultRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VaultRecord {
    pub path: String,
    pub name: String,
    #[serde(default)]
    pub last_note: String,
    #[serde(default)]
    pub sort_alpha: bool,
    #[serde(default = "default_split")]
    pub split_list: f32,
    #[serde(default = "default_reader")]
    pub reader_mode: bool,
}

fn default_split() -> f32 {
    DEFAULT_SPLIT
}

fn default_reader() -> bool {
    true
}

impl VaultRecord {
    pub fn from_path(path: &str) -> Self {
        VaultRecord {
            path: path.to_string(),
            name: crate::model::vault_name(path),
            last_note: String::new(),
            sort_alpha: false,
            split_list: DEFAULT_SPLIT,
            reader_mode: true,
        }
    }
}

/// Parse the Kv blob. A raw UTF-8 path (pre-JSON installs) becomes one entry.
pub fn parse_workspace_bytes(bytes: &[u8]) -> WorkspaceSettings {
    if let Ok(s) = serde_json::from_slice::<WorkspaceSettings>(bytes) {
        return s;
    }
    if let Ok(path) = std::str::from_utf8(bytes) {
        let path = path.trim();
        if !path.is_empty() && !path.starts_with('{') {
            return WorkspaceSettings {
                last_path: path.to_string(),
                vaults: vec![VaultRecord::from_path(path)],
            };
        }
    }
    WorkspaceSettings::default()
}

pub fn upsert_vault(settings: &mut WorkspaceSettings, record: VaultRecord) {
    settings.last_path = record.path.clone();
    if let Some(existing) = settings.vaults.iter_mut().find(|v| v.path == record.path) {
        *existing = record;
    } else {
        settings.vaults.push(record);
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, LibraReflect)]
pub struct VaultRow {
    pub id: String,
    pub title: String,
    pub snippet: String,
    pub selected: bool,
}

impl fmt::Display for VaultRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.id)
    }
}
