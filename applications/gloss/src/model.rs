//! Pure domain for the Gloss shell. Commands and projections only.
//! File bytes move through gloss-cli via Process so reduce stays pure.

use std::fmt;

use gloss_core::{hash_bytes, resolve_link_target, NoteId, Op, Patch, PatchId, RelPath, RootId, RunId};
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
    Undo,
    Backlinks,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Pending {
    #[default]
    None,
    List,
    Cat(String),
    Undo,
    Backlinks,
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

pub const TREE_INSET: f32 = 16.0;

pub fn vault_name(path: &str) -> String {
    let p = path.trim().trim_end_matches('/');
    if p.is_empty() {
        return "No vault".into();
    }
    p.rsplit('/').next().unwrap_or(p).to_string()
}

pub fn parent_prefix(path: &str) -> Option<&str> {
    path.rfind('/').map(|i| &path[..i])
}

pub fn is_folder_id(id: &str, note_paths: &[String]) -> bool {
    let prefix = format!("{id}/");
    note_paths.iter().any(|p| p.starts_with(&prefix))
}

pub fn unique_inbox_path(existing: &[String]) -> String {
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

pub fn unique_folder_note(existing: &[String]) -> String {
    if !existing.iter().any(|p| p.starts_with("Untitled/")) {
        return "Untitled/Untitled.md".into();
    }
    for i in 2..1000 {
        let prefix = format!("Untitled-{i}/");
        if !existing.iter().any(|p| p.starts_with(&prefix)) {
            return format!("Untitled-{i}/Untitled.md");
        }
    }
    "Untitled-new/Untitled.md".into()
}

pub fn project_file_rows(
    note_paths: &[String],
    query: &str,
    selected_path: &str,
    expanded: &std::collections::HashSet<String>,
    sort_alpha: bool,
) -> Vec<FileRow> {
    let q = query.to_ascii_lowercase();
    let files: Vec<String> = note_paths
        .iter()
        .filter(|p| q.is_empty() || p.to_ascii_lowercase().contains(&q))
        .cloned()
        .collect();
    let mut folders = std::collections::BTreeSet::new();
    for p in &files {
        let mut rest = p.as_str();
        while let Some(parent) = parent_prefix(rest) {
            folders.insert(parent.to_string());
            rest = parent;
        }
    }
    let mut out = Vec::new();
    walk_tree(
        None,
        0,
        &folders,
        &files,
        selected_path,
        expanded,
        sort_alpha,
        &mut out,
    );
    out
}

fn walk_tree(
    parent: Option<&str>,
    depth: usize,
    folders: &std::collections::BTreeSet<String>,
    files: &[String],
    selected_path: &str,
    expanded: &std::collections::HashSet<String>,
    sort_alpha: bool,
    out: &mut Vec<FileRow>,
) {
    let mut child_folders: Vec<String> = folders
        .iter()
        .filter(|f| parent_prefix(f) == parent)
        .cloned()
        .collect();
    let mut child_files: Vec<String> = files
        .iter()
        .filter(|p| parent_prefix(p) == parent)
        .cloned()
        .collect();
    if sort_alpha {
        child_folders.sort_by(|a, b| leaf_name(a).cmp(leaf_name(b)));
        child_files.sort_by(|a, b| leaf_name(a).cmp(leaf_name(b)));
    }
    for folder in child_folders {
        let open = expanded.contains(&folder);
        let mark = if open { '-' } else { '+' };
        out.push(FileRow {
            id: folder.clone(),
            title: format!("{mark} {}", leaf_name(&folder)),
            snippet: String::new(),
            selected: false,
            inset: depth as f32 * TREE_INSET,
        });
        if open {
            walk_tree(
                Some(&folder),
                depth + 1,
                folders,
                files,
                selected_path,
                expanded,
                sort_alpha,
                out,
            );
        }
    }
    for path in child_files {
        out.push(FileRow {
            id: path.clone(),
            title: file_stem(&path),
            snippet: String::new(),
            selected: path == selected_path,
            inset: depth as f32 * TREE_INSET,
        });
    }
}

pub fn expand_ancestors_into(expanded: &mut std::collections::HashSet<String>, path: &str) {
    let mut rest = path;
    while let Some(parent) = parent_prefix(rest) {
        expanded.insert(parent.to_string());
        rest = parent;
    }
}

pub fn project_preview(body: &str, skip_title: &str) -> String {
    libra_markdown::flatten(&libra_markdown::parse_preview(body, skip_title))
}

fn leaf_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

pub fn file_stem(path: &str) -> String {
    let name = leaf_name(path);
    name.strip_suffix(".md")
        .or_else(|| name.strip_suffix(".MD"))
        .unwrap_or(name)
        .to_string()
}

/// Resolve a wikilink target against vault paths via gloss-core.
pub fn resolve_wikilink(target: &str, note_paths: &[String]) -> Option<String> {
    let notes: Vec<RelPath> = note_paths
        .iter()
        .filter_map(|p| RelPath::parse(p).ok())
        .collect();
    resolve_link_target(target, &notes).map(|p| p.to_string())
}

/// Candidate paths for Obsidian-style daily notes, most common first.
pub fn daily_note_paths(date: &str) -> Vec<String> {
    vec![
        format!("Daily/{date}.md"),
        format!("Daily Notes/{date}.md"),
        format!("10-Inbox/{date}.md"),
        format!("Journal/{date}.md"),
        format!("{date}.md"),
    ]
}

pub fn resolve_daily_note(date: &str, existing: &[String]) -> Option<String> {
    daily_note_paths(date)
        .into_iter()
        .find(|p| existing.iter().any(|e| e == p))
}

pub fn default_daily_note_path(date: &str) -> String {
    format!("Daily/{date}.md")
}

pub fn daily_note_content(date: &str) -> String {
    format!("# {date}\n\n")
}

/// Local calendar date `YYYY-MM-DD` (macOS v1 — `date(1)`).
pub fn today_local_iso() -> String {
    use std::process::Command;
    if let Ok(out) = Command::new("date").arg("+%Y-%m-%d").output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return s;
            }
        }
    }
    "1970-01-01".into()
}

pub fn project_link_rows(
    body: &str,
    note_paths: &[String],
    selected_path: &str,
) -> Vec<NoteRow> {
    let notes: Vec<RelPath> = note_paths
        .iter()
        .filter_map(|p| RelPath::parse(p).ok())
        .collect();
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for link in gloss_core::wikilinks(body) {
        if link.kind != gloss_core::LinkKind::Wikilink {
            continue;
        }
        let Some(path) = resolve_link_target(&link.raw_target, &notes) else {
            continue;
        };
        let id = path.to_string();
        if !seen.insert(id.clone()) {
            continue;
        }
        out.push(NoteRow {
            id,
            title: file_stem(&path.to_string()),
            snippet: link.alias.clone().unwrap_or(link.raw_target),
            selected: false,
        });
    }
    let _ = selected_path;
    out
}

pub fn project_palette_rows(note_paths: &[String], query: &str, limit: usize) -> Vec<NoteRow> {
    let q = query.to_ascii_lowercase();
    note_paths
        .iter()
        .filter(|p| {
            q.is_empty()
                || p.to_ascii_lowercase().contains(&q)
                || file_stem(p).to_ascii_lowercase().contains(&q)
        })
        .take(limit)
        .map(|p| NoteRow {
            id: p.clone(),
            title: file_stem(p),
            snippet: p.clone(),
            selected: false,
        })
        .collect()
}

pub fn word_count_label(body: &str) -> String {
    let n = body.split_whitespace().count();
    format!("{n} words")
}

pub fn char_count_label(body: &str) -> String {
    let n = body.chars().count();
    format!("{n} chars")
}

pub fn save_label(dirty: bool, status: &str) -> String {
    if status == "Saving…" {
        "Saving…".into()
    } else if dirty {
        "Edited".into()
    } else if status == "Saved" {
        "Saved".into()
    } else {
        String::new()
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

pub fn journal_path(root: &str) -> String {
    let root = root.trim().trim_end_matches('/');
    format!("{root}/.gloss/journal.json")
}

pub fn list_cmd(root: &str) -> CommandSpec {
    CommandSpec::new(GLOSS_BIN)
        .args(["list", "--root", root])
        .timeout_ms(15_000)
}

pub fn backlinks_cmd(root: &str, path: &str) -> CommandSpec {
    CommandSpec::new(GLOSS_BIN)
        .args(["backlinks", "--root", root, "--path", path])
        .timeout_ms(60_000)
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
    Ok(apply_cmd(root, &json))
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
    Ok(apply_cmd(root, &json))
}

pub fn undo_cmd(root: &str) -> CommandSpec {
    CommandSpec::new(GLOSS_BIN)
        .args(["undo", "--root", root, "--journal", &journal_path(root)])
        .timeout_ms(15_000)
}

fn apply_cmd(root: &str, json: &str) -> CommandSpec {
    CommandSpec::new(GLOSS_BIN)
        .args([
            "apply",
            "--root",
            root,
            "--patch-json",
            json,
            "--journal",
            &journal_path(root),
        ])
        .timeout_ms(15_000)
}

pub fn parse_list(stdout: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(stdout)
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}
