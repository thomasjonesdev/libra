//! `libra docs` (§18 / PLAN-PHASE4-DX). A static site from the gallery
//! catalog: one page per component, with the committed dark and light
//! workbench snapshots when they exist.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::app::find_workspace_root;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    pub group: String,
    pub id: String,
    pub name: String,
}

pub fn docs(args: &[String]) -> ExitCode {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let root = match find_workspace_root(&cwd) {
        Ok(root) => root,
        Err(e) => {
            eprintln!("libra docs: {e}");
            return ExitCode::from(2);
        }
    };
    let out = args
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target/libra-docs"));
    let model = root.join("examples/gallery/src/model.rs");
    let source = match fs::read_to_string(&model) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("libra docs: {}: {e}", model.display());
            return ExitCode::from(2);
        }
    };
    let pages = catalog_pages(&source);
    if pages.is_empty() {
        eprintln!("libra docs: no catalog rows in {}", model.display());
        return ExitCode::from(2);
    }
    let snapshots = root.join("examples/gallery/tests/snapshots");
    match write_site(&pages, &snapshots, &out) {
        Ok(n) => {
            println!("libra docs: {n} pages in {}", out.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("libra docs: {e}");
            ExitCode::from(2)
        }
    }
}

/// Pull `("id", "Name")` rows out of the gallery's group constants.
pub fn catalog_pages(source: &str) -> Vec<Page> {
    let mut group = String::new();
    let mut pages = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("pub const ") {
            if let Some(name) = rest.split(':').next() {
                if matches!(name, "LAYOUT" | "DISPLAY" | "CONTROLS" | "DATA") {
                    group = title_group(name);
                }
            }
        }
        if group.is_empty() {
            continue;
        }
        if let Some(page) = row_from_line(trimmed, &group) {
            pages.push(page);
        }
    }
    pages
}

fn title_group(name: &str) -> String {
    match name {
        "LAYOUT" => "Layout".into(),
        "DISPLAY" => "Display".into(),
        "CONTROLS" => "Controls".into(),
        "DATA" => "Data".into(),
        other => other.to_string(),
    }
}

fn row_from_line(line: &str, group: &str) -> Option<Page> {
    let start = line.find("(\"")?;
    let rest = &line[start + 2..];
    let (id, after) = rest.split_once('"')?;
    let after = after.trim_start().trim_start_matches(',').trim_start();
    let name_src = after.strip_prefix('"')?;
    let (name, _) = name_src.split_once('"')?;
    if id.is_empty() || name.is_empty() {
        return None;
    }
    Some(Page {
        group: group.to_string(),
        id: id.to_string(),
        name: name.to_string(),
    })
}

pub fn write_site(pages: &[Page], snapshots: &Path, out: &Path) -> Result<usize, String> {
    fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut index = String::from(
        "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Libra gallery</title></head><body><h1>Libra gallery</h1>\n",
    );
    let mut current = String::new();
    for page in pages {
        if page.group != current {
            if !current.is_empty() {
                index.push_str("</ul>\n");
            }
            index.push_str(&format!("<h2>{}</h2><ul>\n", escape(&page.group)));
            current = page.group.clone();
        }
        index.push_str(&format!(
            "<li><a href=\"{}.html\">{}</a></li>\n",
            escape(&page.id),
            escape(&page.name)
        ));
        write_page(page, snapshots, out)?;
    }
    if !current.is_empty() {
        index.push_str("</ul>\n");
    }
    index.push_str("</body></html>\n");
    fs::write(out.join("index.html"), index).map_err(|e| e.to_string())?;
    Ok(pages.len())
}

fn write_page(page: &Page, snapshots: &Path, out: &Path) -> Result<(), String> {
    let mut body = format!(
        "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>{}</title></head><body><p><a href=\"index.html\">Gallery</a></p><h1>{}</h1><p>{}</p>\n",
        escape(&page.name),
        escape(&page.name),
        escape(&page.group)
    );
    for theme in ["dark", "light"] {
        let file = format!("workbench_{}_{theme}.png", page.id);
        let src = snapshots.join(&file);
        if src.is_file() {
            fs::copy(&src, out.join(&file)).map_err(|e| format!("{}: {e}", src.display()))?;
            body.push_str(&format!(
                "<h2>{}</h2><img src=\"{}\" alt=\"{} {}\">\n",
                theme,
                escape(&file),
                escape(&page.name),
                theme
            ));
        }
    }
    body.push_str("</body></html>\n");
    fs::write(out.join(format!("{}.html", page.id)), body).map_err(|e| e.to_string())
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODEL: &str = r#"
pub const LAYOUT: &[(&str, &str)] = &[
    ("column", "Column"),
    ("safe_area", "SafeArea"),
];
pub const DISPLAY: &[(&str, &str)] = &[
    ("text", "Text"),
];
pub const DEFAULT: &str = "button";
"#;

    #[test]
    fn catalog_rows_keep_their_group() {
        let pages = catalog_pages(MODEL);
        assert_eq!(
            pages,
            vec![
                Page { group: "Layout".into(), id: "column".into(), name: "Column".into() },
                Page { group: "Layout".into(), id: "safe_area".into(), name: "SafeArea".into() },
                Page { group: "Display".into(), id: "text".into(), name: "Text".into() },
            ]
        );
    }

    #[test]
    fn the_site_links_a_copied_snapshot() {
        let dir = std::env::temp_dir().join(format!(
            "libra-docs-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        let snaps = dir.join("snaps");
        let out = dir.join("out");
        fs::create_dir_all(&snaps).unwrap();
        fs::write(snaps.join("workbench_text_dark.png"), b"png").unwrap();
        let pages = vec![Page { group: "Display".into(), id: "text".into(), name: "Text".into() }];
        assert_eq!(write_site(&pages, &snaps, &out).unwrap(), 1);
        let index = fs::read_to_string(out.join("index.html")).unwrap();
        assert!(index.contains("href=\"text.html\""));
        let page = fs::read_to_string(out.join("text.html")).unwrap();
        assert!(page.contains("workbench_text_dark.png"));
        assert_eq!(fs::read(out.join("workbench_text_dark.png")).unwrap(), b"png");
        let _ = fs::remove_dir_all(&dir);
    }
}
