use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use gloss_core::{
    apply_patch, hash_bytes, list_notes, wikilinks, NoteId, Op, Patch, PatchId, RunId, Workspace,
};
use tempfile::tempdir;

fn copy_dir(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let to = dst.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &to);
        } else {
            fs::copy(entry.path(), to).unwrap();
        }
    }
}

fn snapshot(dir: &Path) -> Vec<(String, u64, Vec<u8>)> {
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, u64, Vec<u8>)>) {
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let rel = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if path.is_dir() {
            walk(root, &path, out);
            continue;
        }
        let meta = fs::metadata(&path).unwrap();
        let mtime = meta.mtime() as u64;
        let bytes = fs::read(&path).unwrap();
        out.push((rel, mtime, bytes));
    }
}

fn fixture_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vault-fixtures/mini")
}

#[test]
fn noop_patch_is_byte_and_mtime_identical() {
    let tmp = tempdir().unwrap();
    let vault = tmp.path().join("mini");
    copy_dir(&fixture_root(), &vault);
    let before = snapshot(&vault);

    let mut ws = Workspace::new();
    let root = ws.add_root(&vault, "mini").unwrap();
    let notes = list_notes(ws.root(root).unwrap()).unwrap();
    assert!(!notes.is_empty());
    for rel in notes {
        let note = NoteId::new(root, rel.clone());
        let bytes = ws.read(&note.vault_path()).unwrap();
        let patch = Patch {
            id: PatchId("noop".into()),
            run: RunId("noop".into()),
            ops: vec![Op::ReplaceSpan {
                note,
                start: 0,
                end: 0,
                text: String::new(),
                expect_hash: hash_bytes(&bytes),
            }],
            rationale: "noop".into(),
            provenance: None,
        };
        apply_patch(&ws, &patch).unwrap();
    }

    let after = snapshot(&vault);
    assert_eq!(before, after);
}

#[test]
fn fixture_wikilinks_parse() {
    let src = fs::read_to_string(fixture_root().join("10-Inbox/Welcome.md")).unwrap();
    let links = wikilinks(&src);
    let targets: Vec<_> = links.iter().map(|l| l.raw_target.as_str()).collect();
    assert!(targets.contains(&"Atlas"));
    assert!(!targets.contains(&"Ghost"));
}
