use std::fs;
use std::path::Path;

use gloss_core::{hash_bytes, LinkKind, NoteId, RelPath, Workspace};
use gloss_index::Index;
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

fn fixture_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vault-fixtures/mini")
}

fn open_mini() -> (tempfile::TempDir, Workspace, NoteId, NoteId) {
    let tmp = tempdir().unwrap();
    let vault = tmp.path().join("mini");
    copy_dir(&fixture_root(), &vault);
    let mut ws = Workspace::new();
    let root = ws.add_root(&vault, "mini").unwrap();
    let atlas = NoteId::new(root, RelPath::parse("Atlas.md").unwrap());
    let welcome = NoteId::new(root, RelPath::parse("10-Inbox/Welcome.md").unwrap());
    (tmp, ws, atlas, welcome)
}

#[test]
fn mini_fixture_graph() {
    let (_tmp, ws, atlas, welcome) = open_mini();
    let mut index = Index::open_memory().unwrap();
    let stats = index.refresh(&ws, 0).unwrap();
    assert_eq!(stats.scanned, 2);
    assert_eq!(stats.updated, 2);
    assert_eq!(stats.unchanged, 0);

    let notes = index.notes().unwrap();
    assert_eq!(notes.len(), 2);
    assert!(notes.iter().any(|n| n.title == "Atlas"));
    assert!(notes.iter().any(|n| n.title == "Welcome"));

    let out = index.outgoing(&atlas).unwrap();
    assert!(out
        .iter()
        .any(|l| l.kind == LinkKind::Wikilink && l.dst.as_ref() == Some(&welcome)));
    assert!(out
        .iter()
        .any(|l| l.kind == LinkKind::Embed && l.dst.as_ref() == Some(&welcome)));
    assert!(!out.iter().any(|l| l.raw_target == "Ghost"));

    let neighbors = index.neighbors(&atlas).unwrap();
    assert_eq!(neighbors.outgoing, vec![welcome.clone()]);
    assert_eq!(neighbors.incoming, vec![welcome]);
}

#[test]
fn second_refresh_is_hash_noop() {
    let (tmp, ws, atlas, _) = open_mini();
    let db = tmp.path().join("index.db");
    let mut index = Index::open(&db).unwrap();
    index.refresh(&ws, 1).unwrap();
    let again = index.refresh(&ws, 2).unwrap();
    assert_eq!(again.updated, 0);
    assert_eq!(again.unchanged, 2);
    assert_eq!(again.deleted, 0);

    let before = index
        .notes()
        .unwrap()
        .into_iter()
        .find(|n| n.id == atlas)
        .unwrap();
    assert_eq!(
        before.content_hash,
        hash_bytes(&ws.read(&atlas.vault_path()).unwrap())
    );
}

#[test]
fn edit_and_delete_invalidate() {
    let (_tmp, ws, atlas, welcome) = open_mini();
    let vault = ws.root(atlas.root).unwrap().abs_path().to_path_buf();
    let mut index = Index::open_memory().unwrap();
    index.refresh(&ws, 0).unwrap();

    fs::write(vault.join("Atlas.md"), "no links\n").unwrap();
    let edited = index.refresh(&ws, 3).unwrap();
    assert_eq!(edited.updated, 1);
    assert_eq!(edited.unchanged, 1);
    assert!(index.outgoing(&atlas).unwrap().is_empty());
    assert!(index.incoming(&welcome).unwrap().is_empty());

    fs::remove_file(vault.join("10-Inbox/Welcome.md")).unwrap();
    let gone = index.refresh(&ws, 4).unwrap();
    assert_eq!(gone.deleted, 1);
    assert_eq!(index.notes().unwrap().len(), 1);
}

#[test]
fn existing_wikilinks_are_not_suggestions() {
    let (_tmp, ws, _, _) = open_mini();
    let mut index = Index::open_memory().unwrap();
    index.refresh(&ws, 0).unwrap();
    assert!(index.suggest_links(&ws).unwrap().is_empty());
}

#[test]
fn title_mention_without_wikilink_is_a_suggestion() {
    let (_tmp, ws, atlas, _) = open_mini();
    let vault = ws.root(atlas.root).unwrap().abs_path().to_path_buf();
    fs::write(
        vault.join("10-Inbox/Drift.md"),
        "Inbox leftover. I keep meaning to put this on the Atlas.\n",
    )
    .unwrap();
    let mut index = Index::open_memory().unwrap();
    index.refresh(&ws, 0).unwrap();

    let hits = index.suggest_links(&ws).unwrap();
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0].src.path.as_str(), "10-Inbox/Drift.md");
    assert_eq!(hits[0].dst, atlas);
    assert_eq!(hits[0].matched, "Atlas");
}

#[test]
fn commented_title_mention_is_not_a_suggestion() {
    let (_tmp, ws, atlas, _) = open_mini();
    let vault = ws.root(atlas.root).unwrap().abs_path().to_path_buf();
    fs::write(vault.join("10-Inbox/Drift.md"), "%% Atlas %%\n").unwrap();
    let mut index = Index::open_memory().unwrap();
    index.refresh(&ws, 0).unwrap();
    assert!(index.suggest_links(&ws).unwrap().is_empty());
}
