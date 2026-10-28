//! Requirement traceability matrix for `libra test --trace-matrix`
//! (ARCHITECTURE.md §15.8, PLAN-TEST-CLI).
//!
//! Tests mark themselves with the pass-through `#[libra::verifies("REQ-…")]`
//! attribute. We read those annotations from *source* (no runtime
//! registry), pair each annotated test with its artifact (the
//! `assert_snapshot("<name>")` PNG it produces, when present), and emit
//! `trace-matrix.json` + a self-contained `trace-matrix.html` table — the
//! requirement → test → artifact evidence bundle §15.8 describes.

use std::path::{Path, PathBuf};

use serde_json::json;

/// One annotated test and what it verifies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub reqs: Vec<String>,
    pub test: String,
    pub file: String,
    pub line: usize,
    /// Repo-relative snapshot PNG this test asserts, if any.
    pub artifact: Option<String>,
}

/// Scan the workspace under `repo_root` for `#[libra::verifies(...)]`
/// annotations (PLAN-SDK S3 — not hardcoded to `crates/` + `examples/`).
pub fn collect(repo_root: &Path) -> Vec<Entry> {
    let mut files = Vec::new();
    collect_rs(repo_root, &mut files);
    files.sort();

    let mut entries = Vec::new();
    for file in &files {
        let Ok(src) = std::fs::read_to_string(file) else {
            continue;
        };
        let rel = file
            .strip_prefix(repo_root)
            .unwrap_or(file)
            .to_string_lossy()
            .into_owned();
        scan_source(&src, &rel, &mut entries);
    }
    entries
}

/// Parse the annotations in one source file (split out for testing).
pub fn scan_source(src: &str, rel_path: &str, out: &mut Vec<Entry>) {
    let lines: Vec<&str> = src.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if !line.contains("verifies") || !line.contains('(') {
            continue;
        }
        // Only the `#[libra::verifies(...)]` / `#[verifies(...)]` attribute.
        let t = line.trim_start();
        if !(t.starts_with("#[libra::verifies") || t.starts_with("#[verifies")) {
            continue;
        }
        let reqs = string_literals(line);
        if reqs.is_empty() {
            continue;
        }
        // The annotated test is the next `fn <name>` below the attribute
        // (skipping any further attributes like `#[test]`).
        let Some((fn_line, name)) = lines[i + 1..]
            .iter()
            .enumerate()
            .find_map(|(k, l)| fn_name(l).map(|n| (i + 1 + k, n)))
        else {
            continue;
        };
        let artifact = artifact_in_body(&lines, fn_line, rel_path);
        out.push(Entry {
            reqs,
            test: name,
            file: rel_path.to_string(),
            line: fn_line + 1,
            artifact,
        });
    }
}

/// Write `trace-matrix.json` + `trace-matrix.html` into `out_dir`.
pub fn write(entries: &[Entry], out_dir: &Path) -> std::io::Result<(PathBuf, PathBuf)> {
    std::fs::create_dir_all(out_dir)?;
    let json_path = out_dir.join("trace-matrix.json");
    let html_path = out_dir.join("trace-matrix.html");

    let rows: Vec<_> = entries
        .iter()
        .map(|e| {
            json!({
                "requirements": e.reqs,
                "test": e.test,
                "file": e.file,
                "line": e.line,
                "artifact": e.artifact,
            })
        })
        .collect();
    let doc = json!({ "generated_by": "libra test --trace-matrix", "entries": rows });
    std::fs::write(&json_path, serde_json::to_string_pretty(&doc)?)?;
    std::fs::write(&html_path, render_html(entries))?;
    Ok((json_path, html_path))
}

fn render_html(entries: &[Entry]) -> String {
    let mut rows = String::new();
    for e in entries {
        let artifact = match &e.artifact {
            Some(a) => format!("<code>{}</code>", escape(a)),
            None => "—".to_string(),
        };
        rows.push_str(&format!(
            "<tr><td>{}</td><td><code>{}</code></td><td>{}:{}</td><td>{}</td></tr>\n",
            escape(&e.reqs.join(", ")),
            escape(&e.test),
            escape(&e.file),
            e.line,
            artifact,
        ));
    }
    format!(
        "<!doctype html><meta charset=utf-8><title>libra test — trace matrix</title>\
<style>body{{font:14px system-ui,sans-serif;margin:2rem}}table{{border-collapse:collapse;width:100%}}\
th,td{{border:1px solid #ccc;padding:.4rem .6rem;text-align:left}}th{{background:#f4f4f5}}\
code{{font:12px ui-monospace,monospace}}</style>\
<h1>Requirement trace matrix</h1><p>{} annotated test(s).</p>\
<table><thead><tr><th>Requirement</th><th>Test</th><th>Source</th><th>Artifact</th></tr></thead>\
<tbody>\n{rows}</tbody></table>",
        entries.len()
    )
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        let name = entry.file_name();
        if name == "target" || name == ".git" {
            continue;
        }
        if p.is_dir() {
            collect_rs(&p, out);
        } else if p.extension().is_some_and(|e| e == "rs") {
            out.push(p);
        }
    }
}

/// Extract every `"..."` string literal on a line (attribute args are
/// simple string literals — no escapes to worry about in practice).
fn string_literals(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find('"') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('"') else { break };
        out.push(after[..end].to_string());
        rest = &after[end + 1..];
    }
    out
}

/// `fn foo(` → `Some("foo")`, ignoring non-fn lines.
fn fn_name(line: &str) -> Option<String> {
    let t = line.trim_start();
    let rest = t.strip_prefix("fn ").or_else(|| {
        t.strip_prefix("pub fn ")
            .or_else(|| t.strip_prefix("async fn "))
    })?;
    let name: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    (!name.is_empty()).then_some(name)
}

/// Look inside a test fn's body for the first `assert_snapshot("<name>")`
/// and map it to `<tests-dir>/snapshots/<name>.png` beside that source.
fn artifact_in_body(lines: &[&str], fn_line: usize, rel_path: &str) -> Option<String> {
    for line in lines.iter().skip(fn_line + 1).take(400) {
        if let Some(idx) = line.find("assert_snapshot(") {
            let after = &line[idx..];
            if let Some(name) = string_literals(after).into_iter().next() {
                return Some(snapshot_artifact(rel_path, &name));
            }
        }
        if line == &"}" {
            break;
        }
    }
    None
}

/// `examples/gallery/tests/foo.rs` + `workbench` →
/// `examples/gallery/tests/snapshots/workbench.png`.
fn snapshot_artifact(rel_path: &str, name: &str) -> String {
    let p = Path::new(rel_path);
    let dir = match p.parent() {
        Some(parent) if parent.file_name().is_some_and(|n| n == "tests") => {
            parent.join("snapshots")
        }
        Some(parent) => parent.join("snapshots"),
        None => PathBuf::from("tests/snapshots"),
    };
    dir.join(format!("{name}.png"))
        .to_string_lossy()
        .into_owned()
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = r#"
#[test]
#[libra::verifies("REQ-UI-042")]
fn snapshot_workbench_button_dark() {
    let mut h = harness("libra-dark");
    h.assert_snapshot("workbench_button_dark");
}

#[libra::verifies("REQ-UI-001", "REQ-UI-002")]
#[test]
fn increments() {
    assert_eq!(2, 2);
}
"#;

    #[test]
    fn finds_annotations_and_artifact() {
        let mut out = Vec::new();
        scan_source(SRC, "examples/gallery/tests/gallery.rs", &mut out);
        assert_eq!(out.len(), 2);

        let a = &out[0];
        assert_eq!(a.reqs, vec!["REQ-UI-042"]);
        assert_eq!(a.test, "snapshot_workbench_button_dark");
        assert_eq!(
            a.artifact.as_deref(),
            Some("examples/gallery/tests/snapshots/workbench_button_dark.png")
        );

        let b = &out[1];
        assert_eq!(b.reqs, vec!["REQ-UI-001", "REQ-UI-002"]);
        assert_eq!(b.test, "increments");
        assert_eq!(b.artifact, None);
    }

    #[test]
    fn string_literal_scan() {
        assert_eq!(string_literals(r#"a "x" b "y""#), vec!["x", "y"]);
        assert!(string_literals("no quotes").is_empty());
    }

    #[test]
    fn html_escapes_and_lists() {
        let entries = vec![Entry {
            reqs: vec!["REQ-1".into()],
            test: "t".into(),
            file: "f.rs".into(),
            line: 3,
            artifact: None,
        }];
        let html = render_html(&entries);
        assert!(html.contains("REQ-1"));
        assert!(html.contains("f.rs:3"));
    }
}
