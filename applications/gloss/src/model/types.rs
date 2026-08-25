//! Shared types and row projections for the Gloss shell.

use std::fmt;

use libra_core::LibraReflect;
use serde::{Deserialize, Serialize};

pub const STORE_KEY: &str = "gloss/workspace";
pub const AUTOSAVE_MS: u64 = 500;
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
