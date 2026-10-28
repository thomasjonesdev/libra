//! `libra test --watch` (PLAN-TEST-CLI): re-run the suite when a source
//! file changes. Uses the same mtime-poll approach `libra dev` relies on
//! for `.lx` hot reload — no filesystem-watch dependency — generalized to
//! `.rs` + `.lx` sources across the workspace.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const POLL: Duration = Duration::from_millis(400);

/// Run `run` once, then re-run it whenever a watched source file's mtime
/// changes. Blocks until interrupted (Ctrl-C).
pub fn watch_loop(root: &Path, mut run: impl FnMut()) {
    let mut prev = snapshot(root);
    eprintln!(
        "libra test: watching {} source file(s) — Ctrl-C to stop",
        prev.len()
    );
    run();
    loop {
        std::thread::sleep(POLL);
        let next = snapshot(root);
        if next != prev {
            let changed = next
                .iter()
                .filter(|(k, v)| prev.get(*k) != Some(*v))
                .count();
            eprintln!("\nlibra test: {changed} file(s) changed — re-running");
            prev = next;
            run();
        }
    }
}

/// Map of every watched source file → its mtime.
fn snapshot(root: &Path) -> BTreeMap<PathBuf, SystemTime> {
    let mut map = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            let name = entry.file_name();
            if p.is_dir() {
                if name == "target" || name == ".git" {
                    continue;
                }
                stack.push(p);
                continue;
            }
            if p.extension().is_some_and(|e| e == "rs" || e == "lx") {
                if let Ok(mtime) = entry.metadata().and_then(|m| m.modified()) {
                    map.insert(p, mtime);
                }
            }
        }
    }
    map
}
