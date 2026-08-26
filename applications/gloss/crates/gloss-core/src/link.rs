//! Wikilink resolution and backlink scan (L0). Same rules as the editor
//! shell — exact path, `stem.md`, then case-insensitive stem match.

use crate::parse::{wikilinks, LinkKind};
use crate::path::{PathError, RelPath, RootId, VaultPath};
use crate::workspace::{list_notes, Workspace};

/// Resolve a wikilink target string against the vault's note list.
pub fn resolve_link_target(target: &str, notes: &[RelPath]) -> Option<RelPath> {
    let target = target.split('#').next().unwrap_or(target).trim();
    if target.is_empty() {
        return None;
    }
    if let Ok(rel) = RelPath::parse(target) {
        if notes.iter().any(|n| n == &rel) {
            return Some(rel);
        }
    }
    let with_md = if target.ends_with(".md") || target.ends_with(".MD") {
        target.to_string()
    } else {
        format!("{target}.md")
    };
    if let Ok(rel) = RelPath::parse(&with_md) {
        if notes.iter().any(|n| n == &rel) {
            return Some(rel);
        }
    }
    let needle = {
        let stem = with_md.rsplit('/').next().unwrap_or(&with_md);
        let stem = stem
            .strip_suffix(".md")
            .or_else(|| stem.strip_suffix(".MD"))
            .unwrap_or(stem);
        stem.chars().collect::<String>().to_ascii_lowercase()
    };
    let suffix = format!("/{needle}.md");
    notes
        .iter()
        .find(|p| {
            p.file_stem_key() == needle
                || p.nfc_key() == format!("{needle}.md")
                || p.nfc_key().ends_with(&suffix)
        })
        .cloned()
}

/// Notes whose bodies contain a wikilink that resolves to `target`.
pub fn find_backlinks(
    ws: &Workspace,
    root: RootId,
    target: &RelPath,
) -> Result<Vec<RelPath>, PathError> {
    let root_ref = ws.root(root).ok_or(PathError::OutsideRoot)?;
    let notes = list_notes(root_ref)?;
    let mut out = Vec::new();
    for note in &notes {
        if note == target {
            continue;
        }
        let bytes = ws.read(&VaultPath {
            root,
            rel: note.clone(),
        })?;
        let src = String::from_utf8_lossy(&bytes);
        let links = wikilinks(&src);
        if links.iter().any(|l| {
            l.kind == LinkKind::Wikilink
                && resolve_link_target(&l.raw_target, &notes).as_ref() == Some(target)
        }) {
            out.push(note.clone());
        }
    }
    out.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Workspace;

    fn notes(paths: &[&str]) -> Vec<RelPath> {
        paths.iter().map(|p| RelPath::parse(p).unwrap()).collect()
    }

    #[test]
    fn resolves_stem_and_path() {
        let list = notes(&["Atlas.md", "10-Inbox/Welcome.md"]);
        assert_eq!(
            resolve_link_target("Atlas", &list).map(|p| p.to_string()),
            Some("Atlas.md".into())
        );
        assert_eq!(
            resolve_link_target("Welcome", &list).map(|p| p.to_string()),
            Some("10-Inbox/Welcome.md".into())
        );
        assert_eq!(
            resolve_link_target("10-Inbox/Welcome.md", &list).map(|p| p.to_string()),
            Some("10-Inbox/Welcome.md".into())
        );
    }

    #[test]
    fn find_backlinks_on_fixture_layout() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Atlas.md"), "see [[Welcome]]\n").unwrap();
        std::fs::create_dir_all(dir.path().join("10-Inbox")).unwrap();
        std::fs::write(
            dir.path().join("10-Inbox/Welcome.md"),
            "see [[Atlas|the atlas]]\n",
        )
        .unwrap();
        let mut ws = Workspace::new();
        let id = ws.add_root(dir.path(), "mini").unwrap();
        let target = RelPath::parse("Atlas.md").unwrap();
        let links = find_backlinks(&ws, id, &target).unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].as_str(), "10-Inbox/Welcome.md");
    }
}
