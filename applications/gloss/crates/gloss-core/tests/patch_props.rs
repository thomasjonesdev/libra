use gloss_core::{
    apply_op, apply_patch, hash_bytes, ApplyError, NoteId, Op, Patch, PatchId, RelPath, RunId,
    Workspace,
};
use proptest::prelude::*;
use tempfile::tempdir;

fn is_hash_mismatch(err: &ApplyError) -> bool {
    matches!(err, ApplyError::HashMismatch { .. })
}

fn rel_path() -> impl Strategy<Value = String> {
    prop::collection::vec("[A-Za-z0-9_-]{1,8}", 1..3)
        .prop_map(|parts| format!("{}.md", parts.join("/")))
}

proptest! {
    #[test]
    fn replace_inverse_is_identity(body in "[a-zA-Z0-9 \n]{1,80}", insert in "[a-zA-Z0-9 ]{0,20}") {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("n.md"), &body).unwrap();
        let mut ws = Workspace::new();
        let root = ws.add_root(dir.path(), "t").unwrap();
        let note = NoteId::new(root, RelPath::parse("n.md").unwrap());
        let end = body.len();
        let op = Op::ReplaceSpan {
            note: note.clone(),
            start: 0,
            end,
            text: insert.clone(),
            expect_hash: hash_bytes(body.as_bytes()),
        };
        let inv = apply_op(&ws, &op).unwrap();
        apply_op(&ws, &inv).unwrap();
        let got = ws.read(&note.vault_path()).unwrap();
        prop_assert_eq!(got, body.as_bytes());
    }

    #[test]
    fn stale_hash_never_mutates(body in "[a-z]{1,20}", other in "[a-z]{1,20}") {
        prop_assume!(body != other);
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("n.md"), &body).unwrap();
        let mut ws = Workspace::new();
        let root = ws.add_root(dir.path(), "t").unwrap();
        let note = NoteId::new(root, RelPath::parse("n.md").unwrap());
        let op = Op::ReplaceSpan {
            note: note.clone(),
            start: 0,
            end: body.len(),
            text: "x".into(),
            expect_hash: hash_bytes(other.as_bytes()),
        };
        let err = apply_op(&ws, &op).unwrap_err();
        prop_assert!(is_hash_mismatch(&err));
        prop_assert_eq!(ws.read(&note.vault_path()).unwrap(), body.as_bytes());
    }

    #[test]
    fn parent_segments_never_parse(s in "\\.\\./[A-Za-z]{1,8}") {
        prop_assert!(RelPath::parse(&s).is_err());
    }

    #[test]
    fn create_paths_stay_in_root(rel in rel_path(), content in "[a-z]{0,40}") {
        let dir = tempdir().unwrap();
        let mut ws = Workspace::new();
        let root = ws.add_root(dir.path(), "t").unwrap();
        let path = RelPath::parse(&rel).unwrap();
        let op = Op::CreateNote { root, path: path.clone(), content: content.clone() };
        apply_op(&ws, &op).unwrap();
        let abs = ws.resolve(&gloss_core::VaultPath { root, rel: path }).unwrap();
        prop_assert!(abs.starts_with(ws.root(root).unwrap().abs_path()));
        prop_assert_eq!(std::fs::read(&abs).unwrap(), content.as_bytes());
    }

    #[test]
    fn patch_rollback_restores_bytes(body in "[a-z]{4,12}") {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("n.md"), &body).unwrap();
        let mut ws = Workspace::new();
        let root = ws.add_root(dir.path(), "t").unwrap();
        let note = NoteId::new(root, RelPath::parse("n.md").unwrap());
        let patch = Patch {
            id: PatchId("p".into()),
            run: RunId("r".into()),
            ops: vec![
                Op::ReplaceSpan {
                    note: note.clone(),
                    start: 0,
                    end: body.len(),
                    text: "xxxx".into(),
                    expect_hash: hash_bytes(body.as_bytes()),
                },
                Op::ReplaceSpan {
                    note: note.clone(),
                    start: 0,
                    end: 1,
                    text: "y".into(),
                    expect_hash: hash_bytes(b"nope"),
                },
            ],
            rationale: String::new(),
            provenance: None,
        };
        prop_assert!(apply_patch(&ws, &patch).is_err());
        prop_assert_eq!(ws.read(&note.vault_path()).unwrap(), body.as_bytes());
    }
}
