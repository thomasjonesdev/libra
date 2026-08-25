//! Headless vault core. No UI types. Every mutation goes through the
//! patch layer. Paths are `(RootId, RelPath)` only.

mod hash;
mod journal;
mod parse;
mod patch;
mod path;
mod workspace;

pub use hash::{hash_bytes, ContentHash};
pub use journal::{Journal, RunRecord};
pub use parse::{split_frontmatter, wikilinks, ByteRange, FrontmatterSplit, LinkKind, Wikilink};
pub use patch::{
    apply_in_memory, apply_op, apply_patch, ApplyError, Op, Patch, PatchId, Provenance, RunId,
};
pub use path::{NoteId, PathError, RelPath, Root, RootId, VaultPath};
pub use workspace::{list_notes, Workspace, OPAQUE_DIR_NAMES};

#[cfg(test)]
mod public_path_api {
    use super::*;

    #[test]
    fn vault_path_is_root_plus_rel() {
        let rel = RelPath::parse("Inbox/a.md").unwrap();
        let id = NoteId::new(RootId(1), rel.clone());
        assert_eq!(id.root, RootId(1));
        assert_eq!(id.path, rel);
    }
}
