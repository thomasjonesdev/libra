//! View projections: sidebar rows, palette, mentions, links, labels.

use gloss_core::{resolve_link_target, RelPath};

use super::note::{file_stem, leaf_name};
use super::paths::parent_prefix;
use super::types::{FileRow, NoteRow, TREE_INSET};

fn flat_file_row(path: String, selected_path: &str) -> FileRow {
    FileRow {
        id: path.clone(),
        title: file_stem(&path),
        snippet: String::new(),
        selected: path == selected_path,
        inset: 0.0,
    }
}

fn filter_paths(note_paths: &[String], query: &str) -> Vec<String> {
    let q = query.to_ascii_lowercase();
    note_paths
        .iter()
        .filter(|p| q.is_empty() || p.to_ascii_lowercase().contains(&q))
        .cloned()
        .collect()
}

pub fn project_file_rows(
    note_paths: &[String],
    query: &str,
    selected_path: &str,
    expanded: &std::collections::HashSet<String>,
    sort_alpha: bool,
) -> Vec<FileRow> {
    let files = filter_paths(note_paths, query);
    let mut folders = std::collections::BTreeSet::new();
    for p in &files {
        let mut rest = p.as_str();
        while let Some(parent) = parent_prefix(rest) {
            folders.insert(parent.to_string());
            rest = parent;
        }
    }
    let mut out = Vec::new();
    let ctx = TreeCtx {
        folders: &folders,
        files: &files,
        selected_path,
        expanded,
        sort_alpha,
    };
    walk_tree(None, 0, &ctx, &mut out);
    out
}

pub fn project_search_rows(
    note_paths: &[String],
    query: &str,
    selected_path: &str,
) -> Vec<FileRow> {
    let mut paths = filter_paths(note_paths, query);
    paths.sort_by_key(|a| file_stem(a));
    paths
        .into_iter()
        .map(|path| flat_file_row(path, selected_path))
        .collect()
}

pub fn project_bookmark_rows(
    bookmarks: &[String],
    note_paths: &[String],
    selected_path: &str,
) -> Vec<FileRow> {
    bookmarks
        .iter()
        .filter(|p| note_paths.iter().any(|n| n == *p))
        .map(|path| flat_file_row(path.clone(), selected_path))
        .collect()
}

struct TreeCtx<'a> {
    folders: &'a std::collections::BTreeSet<String>,
    files: &'a [String],
    selected_path: &'a str,
    expanded: &'a std::collections::HashSet<String>,
    sort_alpha: bool,
}

fn walk_tree(parent: Option<&str>, depth: usize, ctx: &TreeCtx<'_>, out: &mut Vec<FileRow>) {
    let mut child_folders: Vec<String> = ctx
        .folders
        .iter()
        .filter(|f| parent_prefix(f) == parent)
        .cloned()
        .collect();
    let mut child_files: Vec<String> = ctx
        .files
        .iter()
        .filter(|p| parent_prefix(p) == parent)
        .cloned()
        .collect();
    if ctx.sort_alpha {
        child_folders.sort_by_key(|a| leaf_name(a).0.to_string());
        child_files.sort_by_key(|a| leaf_name(a).0.to_string());
    }
    for folder in child_folders {
        let open = ctx.expanded.contains(&folder);
        let mark = if open { '-' } else { '+' };
        out.push(FileRow {
            id: folder.clone(),
            title: format!("{mark} {}", leaf_name(&folder).0),
            snippet: String::new(),
            selected: false,
            inset: depth as f32 * TREE_INSET,
        });
        if open {
            walk_tree(Some(&folder), depth + 1, ctx, out);
        }
    }
    for path in child_files {
        out.push(FileRow {
            id: path.clone(),
            title: file_stem(&path),
            snippet: String::new(),
            selected: path == ctx.selected_path,
            inset: depth as f32 * TREE_INSET,
        });
    }
}

pub fn project_preview(body: &str, skip_title: &str) -> String {
    libra_markdown::flatten(&libra_markdown::parse_preview(body, skip_title))
}

pub fn project_link_rows(body: &str, note_paths: &[String]) -> Vec<NoteRow> {
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

/// Active `@mention` at line start or after whitespace — `(byte_index, query)`.
pub fn mention_trigger(text: &str) -> Option<(usize, String)> {
    let bytes = text.as_bytes();
    let mut last = None;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'@' {
            let prev_ok = i == 0
                || matches!(bytes[i - 1], b' ' | b'\n' | b'\t' | b'(' | b'[');
            if prev_ok {
                let rest = &text[i + 1..];
                let end = rest
                    .find(|c: char| c.is_whitespace() || c == '\n')
                    .unwrap_or(rest.len());
                let query = &rest[..end];
                if query
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '/')
                {
                    last = Some((i, query.to_string()));
                }
            }
        }
        i += 1;
    }
    last
}

/// Replace `@query` at `at` with `[[target_stem]]`.
pub fn insert_wikilink_mention(text: &str, at: usize, query_len: usize, target_stem: &str) -> String {
    let mut out = String::new();
    out.push_str(&text[..at]);
    out.push_str(&format!("[[{target_stem}]]"));
    out.push_str(&text[at + 1 + query_len..]);
    out
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

pub fn rules_preview(body: &str, max_chars: usize) -> String {
    let t = body.trim();
    if t.is_empty() {
        return String::new();
    }
    if t.chars().count() <= max_chars {
        return t.to_string();
    }
    format!("{}…", t.chars().take(max_chars).collect::<String>())
}
