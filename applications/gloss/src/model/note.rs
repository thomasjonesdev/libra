//! Note text: frontmatter, titles, snippets, and assembly.

use gloss_core::RelPath;

#[derive(Debug, Clone, Copy)]
pub struct LeafName<'a>(pub &'a str);

pub fn leaf_name(path: &str) -> LeafName<'_> {
    LeafName(path.rsplit('/').next().unwrap_or(path))
}

pub fn file_stem(path: &str) -> String {
    let LeafName(name) = leaf_name(path);
    name.strip_suffix(".md")
        .or_else(|| name.strip_suffix(".MD"))
        .unwrap_or(name)
        .to_string()
}

/// Split a note into `(yaml, body, has_frontmatter)`.
pub fn split_note(full: &str) -> (String, String, bool) {
    match gloss_core::split_frontmatter(full) {
        Some(split) => (
            split.yaml.to_string(),
            split.body.trim_start_matches('\n').to_string(),
            true,
        ),
        None => (String::new(), full.to_string(), false),
    }
}

/// Rebuild file bytes from YAML (without fences) and body.
pub fn assemble_note(yaml: &str, body: &str) -> String {
    if yaml.trim().is_empty() {
        return body.to_string();
    }
    let yaml = yaml.trim_end();
    let body = body.trim_start_matches('\n');
    if body.is_empty() {
        format!("---\n{yaml}\n---\n")
    } else {
        format!("---\n{yaml}\n---\n\n{body}")
    }
}

pub fn parse_frontmatter_fields(yaml: &str) -> Vec<super::types::FrontmatterField> {
    yaml.lines()
        .filter_map(|line| {
            let t = line.trim();
            if t.is_empty() || t.starts_with('#') {
                return None;
            }
            let (key, rest) = t.split_once(':')?;
            let key = key.trim();
            if key.is_empty() {
                return None;
            }
            let value = rest.trim().trim_matches('"').to_string();
            Some(super::types::FrontmatterField {
                key: key.to_string(),
                value,
            })
        })
        .collect()
}

pub fn frontmatter_title(yaml: &str) -> String {
    parse_frontmatter_fields(yaml)
        .into_iter()
        .find(|f| f.key == "title")
        .map(|f| f.value)
        .unwrap_or_default()
}

/// Single-line `key: value` rows for the reader properties scan list.
pub fn frontmatter_scan_rows(yaml: &str) -> Vec<super::types::FrontmatterField> {
    parse_frontmatter_fields(yaml)
        .into_iter()
        .filter(|f| !f.value.is_empty() && !f.value.contains('\n'))
        .collect()
}

pub fn full_note_text(reader_mode: bool, has_frontmatter: bool, yaml: &str, editor: &str) -> String {
    if reader_mode && has_frontmatter {
        assemble_note(yaml, editor)
    } else {
        editor.to_string()
    }
}

pub fn apply_loaded_note(
    reader_mode: bool,
    full: &str,
) -> (String, String, bool, String, Vec<super::types::FrontmatterField>) {
    let (yaml, body, has) = split_note(full);
    let title = frontmatter_title(&yaml);
    let fields = frontmatter_scan_rows(&yaml);
    let editor = if reader_mode { body } else { full.to_string() };
    (yaml, editor, has, title, fields)
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
