//! Coverage reporting for `libra test` (ARCHITECTURE.md §15.8,
//! PLAN-TEST-CLI).
//!
//! `cargo llvm-cov` runs the suite under instrumentation and exports the
//! LLVM coverage JSON; we parse it into two numbers: the whole-workspace
//! line coverage and the **actions-layer** line coverage — the latter is
//! "meaningful line coverage of actual behavior" (§15.8), computed by
//! filtering the export to each app's actions files.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

/// Line-coverage tally for a set of files.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Lines {
    pub covered: u64,
    pub total: u64,
}

impl Lines {
    fn add(&mut self, covered: u64, total: u64) {
        self.covered += covered;
        self.total += total;
    }
    pub fn percent(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.covered as f64 / self.total as f64 * 100.0
        }
    }
}

/// The parsed coverage report: overall lines + the actions-layer subset.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Report {
    pub overall: Lines,
    pub actions: Lines,
    /// Per-app actions coverage, keyed by the app's actions-file suffix,
    /// for the detailed block.
    pub per_actions_file: Vec<(String, Lines)>,
}

/// Parse an `llvm-cov --json` export. `is_actions_file` decides whether a
/// file's lines count toward the actions-layer tally.
pub fn parse(json: &Value, is_actions_file: impl Fn(&str) -> bool) -> Report {
    let mut report = Report::default();
    let Some(files) = json
        .get("data")
        .and_then(|d| d.get(0))
        .and_then(|d| d.get("files"))
        .and_then(|f| f.as_array())
    else {
        return report;
    };

    for file in files {
        let Some(name) = file.get("filename").and_then(|n| n.as_str()) else {
            continue;
        };
        let lines = file.get("summary").and_then(|s| s.get("lines"));
        let covered = lines.and_then(|l| l.get("covered")).and_then(Value::as_u64).unwrap_or(0);
        let total = lines.and_then(|l| l.get("count")).and_then(Value::as_u64).unwrap_or(0);

        report.overall.add(covered, total);
        if is_actions_file(name) {
            report.actions.add(covered, total);
            let mut ln = Lines::default();
            ln.add(covered, total);
            report.per_actions_file.push((short_name(name), ln));
        }
    }
    report.per_actions_file.sort_by(|a, b| a.0.cmp(&b.0));
    report
}

/// Locate the app "actions layer" files under `repo_root`: any file named
/// `actions.rs`, plus single-file apps whose `src/*.rs` carries
/// `#[libra::actions]`. Returned as path suffixes (relative to the repo)
/// so they match the absolute paths llvm-cov emits via `ends_with`.
pub fn actions_file_suffixes(repo_root: &Path) -> Vec<String> {
    let mut out = BTreeSet::new();
    let mut stack = vec![repo_root.join("examples"), repo_root.join("crates")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let p = entry.path();
            let name = entry.file_name();
            if p.is_dir() {
                if name == "target" || name == "tests" {
                    continue;
                }
                stack.push(p);
                continue;
            }
            if p.extension().is_some_and(|e| e == "rs") {
                if name == "actions.rs" {
                    if let Some(rel) = rel_suffix(repo_root, &p) {
                        out.insert(rel);
                    }
                } else if let Ok(src) = std::fs::read_to_string(&p) {
                    if src.contains("#[libra::actions]") || src.contains("# [libra :: actions]") {
                        if let Some(rel) = rel_suffix(repo_root, &p) {
                            out.insert(rel);
                        }
                    }
                }
            }
        }
    }
    out.into_iter().collect()
}

fn rel_suffix(root: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(root)
        .ok()
        .map(|r| r.to_string_lossy().into_owned())
}

/// A short, readable label for a coverage file path (`examples/notes/src/actions.rs`).
fn short_name(name: &str) -> String {
    let p = PathBuf::from(name);
    for anchor in ["examples", "crates"] {
        if let Some(idx) = p.components().position(|c| c.as_os_str() == anchor) {
            let tail: PathBuf = p.components().skip(idx).map(|c| c.as_os_str()).collect();
            return tail.to_string_lossy().into_owned();
        }
    }
    name.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn export() -> Value {
        json!({
            "type": "llvm.coverage.json.export",
            "data": [{
                "files": [
                    { "filename": "/repo/examples/notes/src/actions.rs",
                      "summary": { "lines": { "count": 100, "covered": 90 } } },
                    { "filename": "/repo/examples/notes/src/lib.rs",
                      "summary": { "lines": { "count": 50, "covered": 10 } } },
                    { "filename": "/repo/crates/libra-markup/src/lib.rs",
                      "summary": { "lines": { "count": 200, "covered": 100 } } }
                ]
            }]
        })
    }

    #[test]
    fn overall_and_actions_split() {
        let r = parse(&export(), |f| f.ends_with("actions.rs"));
        assert_eq!(r.overall, Lines { covered: 200, total: 350 });
        assert_eq!(r.actions, Lines { covered: 90, total: 100 });
        assert_eq!(r.actions.percent(), 90.0);
        assert_eq!(r.per_actions_file.len(), 1);
        assert_eq!(r.per_actions_file[0].0, "examples/notes/src/actions.rs");
    }

    #[test]
    fn empty_export_is_zero() {
        let r = parse(&json!({}), |_| true);
        assert_eq!(r.overall, Lines::default());
        assert_eq!(r.overall.percent(), 0.0);
    }
}
