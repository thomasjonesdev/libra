//! gloss-cli command builders and stdout parsing.

use gloss_core::{hash_bytes, NoteId, Op, Patch, PatchId, RelPath, RootId, RunId};
use libra_core::CommandSpec;

use super::paths::journal_path;
use super::types::GLOSS_BIN;

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

pub fn rules_cmd(root: &str) -> CommandSpec {
    CommandSpec::new(GLOSS_BIN)
        .args(["cat", "--root", root, "--path", super::types::RULES_PATH])
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
