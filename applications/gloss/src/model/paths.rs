//! Vault paths, daily notes, and wikilink resolution.

use gloss_core::{resolve_link_target, RelPath};

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
    unique_note_path(existing, "10-Inbox/Untitled.md", |i| format!("10-Inbox/Untitled-{i}.md"))
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

fn unique_note_path(
    existing: &[String],
    default: &str,
    numbered: impl Fn(u32) -> String,
) -> String {
    if !existing.iter().any(|p| p == default) {
        return default.into();
    }
    for i in 2..1000 {
        let p = numbered(i);
        if !existing.iter().any(|e| e == &p) {
            return p;
        }
    }
    format!("{default}-new")
}

pub fn expand_ancestors_into(expanded: &mut std::collections::HashSet<String>, path: &str) {
    let mut rest = path;
    while let Some(parent) = parent_prefix(rest) {
        expanded.insert(parent.to_string());
        rest = parent;
    }
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

pub fn journal_path(root: &str) -> String {
    let root = root.trim().trim_end_matches('/');
    format!("{root}/.gloss/journal.json")
}
