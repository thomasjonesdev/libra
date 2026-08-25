//! Pure domain for the Gloss shell. Commands and projections only.
//! File bytes move through gloss-cli via Process so reduce stays pure.

use std::fmt;

use gloss_core::{hash_bytes, NoteId, Op, Patch, PatchId, RelPath, RootId, RunId};
use libra_core::{CommandSpec, LibraReflect};
use serde::{Deserialize, Serialize};

pub const STORE_KEY: &str = "gloss/workspace";
pub const AUTOSAVE_MS: u64 = 500;
pub const GLOSS_BIN: &str = "gloss";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum IoKind {
    #[default]
    Idle,
    List,
    Cat,
    Apply,
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

pub fn note_title(path: &str, body: &str) -> String {
    for line in body.lines() {
        let t = line.trim();
        if t.is_empty() || t == "---" {
            continue;
        }
        if let Some(rest) = t.strip_prefix("# ") {
            return rest.to_string();
        }
        if let Some(rest) = t.strip_prefix("title:") {
            let v = rest.trim().trim_matches('"');
            if !v.is_empty() {
                return v.to_string();
            }
        }
        if !t.starts_with('#') && !t.contains(':') {
            return t.to_string();
        }
    }
    RelPath::parse(path)
        .map(|p| p.file_name().to_string())
        .unwrap_or_else(|_| path.to_string())
}

pub fn note_snippet(body: &str) -> String {
    let mut parts = Vec::new();
    for line in body.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with("---") || t.starts_with('#') || t.contains(':') {
            continue;
        }
        parts.push(t);
        if parts.len() == 3 {
            break;
        }
    }
    let s = parts.join(" ");
    if s.chars().count() > 80 {
        format!("{}…", s.chars().take(79).collect::<String>())
    } else {
        s
    }
}

pub fn list_cmd(root: &str) -> CommandSpec {
    CommandSpec::new(GLOSS_BIN)
        .args(["list", "--root", root])
        .timeout_ms(15_000)
}

pub fn cat_cmd(root: &str, path: &str) -> CommandSpec {
    CommandSpec::new(GLOSS_BIN)
        .args(["cat", "--root", root, "--path", path])
        .timeout_ms(15_000)
}

pub fn apply_replace_cmd(
    root: &str,
    path: &str,
    old: &str,
    new: &str,
) -> Result<CommandSpec, String> {
    let rel = RelPath::parse(path).map_err(|e| e.to_string())?;
    let patch = Patch {
        id: PatchId("edit".into()),
        run: RunId("edit".into()),
        ops: vec![Op::ReplaceSpan {
            note: NoteId::new(RootId(1), rel),
            start: 0,
            end: old.len(),
            text: new.to_string(),
            expect_hash: hash_bytes(old.as_bytes()),
        }],
        rationale: "editor save".into(),
        provenance: None,
    };
    let json = serde_json::to_string(&patch).map_err(|e| e.to_string())?;
    Ok(CommandSpec::new(GLOSS_BIN)
        .args(["apply", "--root", root, "--patch-json", &json])
        .timeout_ms(15_000))
}

pub fn apply_create_cmd(root: &str, path: &str, content: &str) -> Result<CommandSpec, String> {
    let rel = RelPath::parse(path).map_err(|e| e.to_string())?;
    let patch = Patch {
        id: PatchId("create".into()),
        run: RunId("create".into()),
        ops: vec![Op::CreateNote {
            root: RootId(1),
            path: rel,
            content: content.to_string(),
        }],
        rationale: "new note".into(),
        provenance: None,
    };
    let json = serde_json::to_string(&patch).map_err(|e| e.to_string())?;
    Ok(CommandSpec::new(GLOSS_BIN)
        .args(["apply", "--root", root, "--patch-json", &json])
        .timeout_ms(15_000))
}

pub fn parse_list(stdout: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(stdout)
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}
