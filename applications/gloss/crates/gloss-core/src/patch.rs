use std::fmt;

use serde::{Deserialize, Serialize};

use crate::hash::{hash_bytes, ContentHash};
use crate::parse::ByteRange;
use crate::path::{NoteId, RelPath, RootId, VaultPath};
use crate::workspace::Workspace;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PatchId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RunId(pub String);

impl RunId {
    pub fn new(s: impl Into<String>) -> Self {
        RunId(s.into())
    }
}

impl From<&str> for RunId {
    fn from(s: &str) -> Self {
        RunId(s.to_string())
    }
}

impl From<String> for RunId {
    fn from(s: String) -> Self {
        RunId(s)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Op {
    CreateNote {
        root: RootId,
        path: RelPath,
        content: String,
    },
    ReplaceSpan {
        note: NoteId,
        start: usize,
        end: usize,
        text: String,
        expect_hash: ContentHash,
    },
    SetFrontmatter {
        note: NoteId,
        key: String,
        value: String,
        expect_hash: ContentHash,
    },
    RenameNote {
        note: NoteId,
        to: RelPath,
        rewrite_backlinks: bool,
    },
    DeleteNote {
        note: NoteId,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Provenance {
    pub model: String,
    pub prompt_version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Patch {
    pub id: PatchId,
    pub run: RunId,
    pub ops: Vec<Op>,
    pub rationale: String,
    pub provenance: Option<Provenance>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyError {
    HashMismatch {
        have: ContentHash,
        expect: ContentHash,
    },
    MissingNote,
    Exists,
    Range,
    Path(String),
    Frontmatter,
}

impl fmt::Display for ApplyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApplyError::HashMismatch { .. } => write!(f, "expect_hash mismatch; file unchanged"),
            ApplyError::MissingNote => write!(f, "note does not exist"),
            ApplyError::Exists => write!(f, "note already exists"),
            ApplyError::Range => write!(f, "byte range is outside the file or not a char boundary"),
            ApplyError::Path(msg) => write!(f, "{msg}"),
            ApplyError::Frontmatter => write!(f, "cannot edit frontmatter without a YAML block"),
        }
    }
}

impl std::error::Error for ApplyError {}

/// Apply one op on an in-memory buffer.
pub fn apply_in_memory(bytes: &[u8], op: &Op) -> Result<(Vec<u8>, Op), ApplyError> {
    match op {
        Op::ReplaceSpan {
            start,
            end,
            text,
            expect_hash,
            note,
        } => {
            let have = hash_bytes(bytes);
            if have != *expect_hash {
                return Err(ApplyError::HashMismatch {
                    have,
                    expect: *expect_hash,
                });
            }
            if !valid_range(bytes, *start, *end) {
                return Err(ApplyError::Range);
            }
            let old = bytes[*start..*end].to_vec();
            let old_text = String::from_utf8(old).map_err(|_| ApplyError::Range)?;
            let mut next = Vec::with_capacity(bytes.len() - (*end - *start) + text.len());
            next.extend_from_slice(&bytes[..*start]);
            next.extend_from_slice(text.as_bytes());
            next.extend_from_slice(&bytes[*end..]);
            let inverse = Op::ReplaceSpan {
                note: note.clone(),
                start: *start,
                end: *start + text.len(),
                text: old_text,
                expect_hash: hash_bytes(&next),
            };
            Ok((next, inverse))
        }
        Op::SetFrontmatter {
            note,
            key,
            value,
            expect_hash,
        } => {
            let have = hash_bytes(bytes);
            if have != *expect_hash {
                return Err(ApplyError::HashMismatch {
                    have,
                    expect: *expect_hash,
                });
            }
            let src = std::str::from_utf8(bytes).map_err(|_| ApplyError::Range)?;
            let (next, old_raw, range) = set_frontmatter_bytes(src, key, value)?;
            let inverse = Op::ReplaceSpan {
                note: note.clone(),
                start: range.start,
                end: range.end,
                text: old_raw,
                expect_hash: hash_bytes(next.as_bytes()),
            };
            Ok((next.into_bytes(), inverse))
        }
        Op::CreateNote { content, .. } => {
            if !bytes.is_empty() {
                return Err(ApplyError::Exists);
            }
            Ok((content.as_bytes().to_vec(), inverse_create(op)))
        }
        Op::DeleteNote { .. } | Op::RenameNote { .. } => {
            Err(ApplyError::Path("this op is not an in-memory edit".into()))
        }
    }
}

fn inverse_create(op: &Op) -> Op {
    match op {
        Op::CreateNote { root, path, .. } => Op::DeleteNote {
            note: NoteId::new(*root, path.clone()),
        },
        _ => Op::DeleteNote {
            note: NoteId::new(RootId(0), RelPath::parse("x.md").expect("static")),
        },
    }
}

pub fn apply_op(ws: &Workspace, op: &Op) -> Result<Op, ApplyError> {
    match op {
        Op::CreateNote {
            root,
            path,
            content,
        } => {
            let vp = VaultPath {
                root: *root,
                rel: path.clone(),
            };
            let abs = ws
                .resolve(&vp)
                .map_err(|e| ApplyError::Path(e.to_string()))?;
            if abs.exists() {
                return Err(ApplyError::Exists);
            }
            ws.write(&vp, content.as_bytes())
                .map_err(|e| ApplyError::Path(e.to_string()))?;
            Ok(Op::DeleteNote {
                note: NoteId::new(*root, path.clone()),
            })
        }
        Op::ReplaceSpan { note, .. } | Op::SetFrontmatter { note, .. } => {
            let vp = note.vault_path();
            let bytes = ws.read(&vp).map_err(|_| ApplyError::MissingNote)?;
            let (next, inverse) = apply_in_memory(&bytes, op)?;
            if next != bytes {
                ws.write(&vp, &next)
                    .map_err(|e| ApplyError::Path(e.to_string()))?;
            }
            Ok(inverse)
        }
        Op::DeleteNote { note } => {
            let vp = note.vault_path();
            let bytes = ws.read(&vp).map_err(|_| ApplyError::MissingNote)?;
            let content = String::from_utf8(bytes).map_err(|_| ApplyError::Range)?;
            ws.remove(&vp)
                .map_err(|e| ApplyError::Path(e.to_string()))?;
            Ok(Op::CreateNote {
                root: note.root,
                path: note.path.clone(),
                content,
            })
        }
        Op::RenameNote {
            note,
            to,
            rewrite_backlinks,
        } => {
            if *rewrite_backlinks {
                return Err(ApplyError::Path(
                    "rewrite_backlinks is not implemented".into(),
                ));
            }
            let vp = note.vault_path();
            ws.rename(&vp, to)
                .map_err(|e| ApplyError::Path(e.to_string()))?;
            Ok(Op::RenameNote {
                note: NoteId::new(note.root, to.clone()),
                to: note.path.clone(),
                rewrite_backlinks: false,
            })
        }
    }
}

/// Apply every op. On failure, invert the ops that already landed.
pub fn apply_patch(ws: &Workspace, patch: &Patch) -> Result<Vec<Op>, ApplyError> {
    let mut inverses = Vec::new();
    for op in &patch.ops {
        match apply_op(ws, op) {
            Ok(inv) => inverses.push(inv),
            Err(e) => {
                for inv in inverses.into_iter().rev() {
                    let _ = apply_op(ws, &inv);
                }
                return Err(e);
            }
        }
    }
    inverses.reverse();
    Ok(inverses)
}

fn valid_range(bytes: &[u8], start: usize, end: usize) -> bool {
    if start > end || end > bytes.len() {
        return false;
    }
    std::str::from_utf8(bytes)
        .ok()
        .is_some_and(|s| s.is_char_boundary(start) && s.is_char_boundary(end))
}

fn set_frontmatter_bytes(
    src: &str,
    key: &str,
    value: &str,
) -> Result<(String, String, ByteRange), ApplyError> {
    let split = crate::parse::split_frontmatter(src).ok_or(ApplyError::Frontmatter)?;
    let yaml = split.yaml;
    let yaml_start = split.yaml_range.start;
    let prefix = format!("{key}:");
    let mut cursor = 0;
    while cursor < yaml.len() {
        let rest = &yaml[cursor..];
        let line_len = rest.find('\n').map(|i| i + 1).unwrap_or(rest.len());
        let content = rest[..line_len].trim_end_matches(['\n', '\r']);
        let indent = content.len() - content.trim_start().len();
        let trimmed = &content[indent..];
        if let Some(after_colon) = trimmed.strip_prefix(&prefix) {
            let ws = after_colon.len() - after_colon.trim_start().len();
            let value_off = indent + prefix.len() + ws;
            if value_off > content.len() {
                return Err(ApplyError::Frontmatter);
            }
            let value_start = yaml_start + cursor + value_off;
            let value_end = yaml_start + cursor + content.len();
            let old_raw = src[value_start..value_end].to_string();
            let mut next = String::new();
            next.push_str(&src[..value_start]);
            next.push_str(value);
            next.push_str(&src[value_end..]);
            return Ok((
                next,
                old_raw,
                ByteRange {
                    start: value_start,
                    end: value_start + value.len(),
                },
            ));
        }
        cursor += line_len;
    }
    let insert_at = split.yaml_range.end;
    let insertion = if split.yaml.ends_with('\n') {
        format!("{key}: {value}\n")
    } else {
        format!("\n{key}: {value}")
    };
    let mut next = String::new();
    next.push_str(&src[..insert_at]);
    next.push_str(&insertion);
    next.push_str(&src[insert_at..]);
    Ok((
        next,
        String::new(),
        ByteRange {
            start: insert_at,
            end: insert_at + insertion.len(),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::hash_bytes;
    use crate::path::RelPath;
    use tempfile::tempdir;

    fn ws_with(file: &str, content: &str) -> (tempfile::TempDir, Workspace, RootId) {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(file), content).unwrap();
        let mut ws = Workspace::new();
        let id = ws.add_root(dir.path(), "t").unwrap();
        (dir, ws, id)
    }

    #[test]
    fn replace_then_inverse_is_identity() {
        let (_dir, ws, root) = ws_with("a.md", "hello world");
        let note = NoteId::new(root, RelPath::parse("a.md").unwrap());
        let expect = hash_bytes(b"hello world");
        let op = Op::ReplaceSpan {
            note: note.clone(),
            start: 6,
            end: 11,
            text: "there".into(),
            expect_hash: expect,
        };
        let inv = apply_op(&ws, &op).unwrap();
        assert_eq!(ws.read(&note.vault_path()).unwrap(), b"hello there");
        apply_op(&ws, &inv).unwrap();
        assert_eq!(ws.read(&note.vault_path()).unwrap(), b"hello world");
    }

    #[test]
    fn hash_mismatch_does_not_write() {
        let (_dir, ws, root) = ws_with("a.md", "hello");
        let note = NoteId::new(root, RelPath::parse("a.md").unwrap());
        let op = Op::ReplaceSpan {
            note: note.clone(),
            start: 0,
            end: 5,
            text: "nope".into(),
            expect_hash: hash_bytes(b"stale"),
        };
        assert!(matches!(
            apply_op(&ws, &op),
            Err(ApplyError::HashMismatch { .. })
        ));
        assert_eq!(ws.read(&note.vault_path()).unwrap(), b"hello");
    }

    #[test]
    fn create_and_delete_round_trip() {
        let dir = tempdir().unwrap();
        let mut ws = Workspace::new();
        let root = ws.add_root(dir.path(), "t").unwrap();
        let path = RelPath::parse("new.md").unwrap();
        let create = Op::CreateNote {
            root,
            path: path.clone(),
            content: "hi".into(),
        };
        let inv = apply_op(&ws, &create).unwrap();
        assert_eq!(
            ws.read(&VaultPath {
                root,
                rel: path.clone()
            })
            .unwrap(),
            b"hi"
        );
        apply_op(&ws, &inv).unwrap();
        assert!(!dir.path().join("new.md").exists());
    }

    #[test]
    fn failed_second_op_rolls_back() {
        let (_dir, ws, root) = ws_with("a.md", "keep");
        let note = NoteId::new(root, RelPath::parse("a.md").unwrap());
        let patch = Patch {
            id: PatchId("p".into()),
            run: RunId("r".into()),
            ops: vec![
                Op::ReplaceSpan {
                    note: note.clone(),
                    start: 0,
                    end: 4,
                    text: "gone".into(),
                    expect_hash: hash_bytes(b"keep"),
                },
                Op::ReplaceSpan {
                    note: note.clone(),
                    start: 0,
                    end: 4,
                    text: "x".into(),
                    expect_hash: hash_bytes(b"stale"),
                },
            ],
            rationale: "test".into(),
            provenance: None,
        };
        assert!(apply_patch(&ws, &patch).is_err());
        assert_eq!(ws.read(&note.vault_path()).unwrap(), b"keep");
    }

    #[test]
    fn set_frontmatter_keeps_unknown_keys() {
        let src = "---\ntitle: Old\nweird: 1\n---\n\nbody\n";
        let (_dir, ws, root) = ws_with("a.md", src);
        let note = NoteId::new(root, RelPath::parse("a.md").unwrap());
        let op = Op::SetFrontmatter {
            note: note.clone(),
            key: "title".into(),
            value: "New".into(),
            expect_hash: hash_bytes(src.as_bytes()),
        };
        apply_op(&ws, &op).unwrap();
        let got = String::from_utf8(ws.read(&note.vault_path()).unwrap()).unwrap();
        assert!(got.contains("title: New"));
        assert!(got.contains("weird: 1"));
        assert!(got.contains("body"));
    }

    fn frontmatter_op(key: &str, value: &str, src: &str) -> Op {
        Op::SetFrontmatter {
            note: NoteId::new(RootId(1), RelPath::parse("a.md").unwrap()),
            key: key.into(),
            value: value.into(),
            expect_hash: hash_bytes(src.as_bytes()),
        }
    }

    #[test]
    fn set_frontmatter_insert_inverse_is_identity() {
        let src = "---\ntitle: Hi\n---\n\nbody";
        let (next, inv) = apply_in_memory(src.as_bytes(), &frontmatter_op("tags", "x", src)).unwrap();
        let (restored, _) = apply_in_memory(&next, &inv).unwrap();
        assert_eq!(restored, src.as_bytes());
    }

    #[test]
    fn set_frontmatter_quoted_value_inverse_keeps_quotes() {
        let src = "---\ntitle: \"Old\"\n---\n\nbody\n";
        let (next, inv) =
            apply_in_memory(src.as_bytes(), &frontmatter_op("title", "New", src)).unwrap();
        assert!(
            String::from_utf8_lossy(&next).contains("title: New"),
            "{}",
            String::from_utf8_lossy(&next)
        );
        let (restored, _) = apply_in_memory(&next, &inv).unwrap();
        assert_eq!(restored, src.as_bytes());
    }

    #[test]
    fn set_frontmatter_value_does_not_collide_with_key() {
        let src = "---\ntitle: t\n---\n\n";
        let (next, _) = apply_in_memory(src.as_bytes(), &frontmatter_op("title", "Z", src)).unwrap();
        assert_eq!(next, b"---\ntitle: Z\n---\n\n");
    }
}
