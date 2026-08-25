use std::fmt;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

/// Stable id for a vault root. Assigned by [`crate::Workspace::add_root`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RootId(pub u64);

/// Relative path inside a root. `/`-separated, no `..`, never absolute.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RelPath(String);

/// A note identity. The path is the identity until an integer index exists.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NoteId {
    pub root: RootId,
    pub path: RelPath,
}

/// `(RootId, RelPath)` as a located file.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VaultPath {
    pub root: RootId,
    pub rel: RelPath,
}

/// An opened vault root. The absolute path lives only here, at the boundary.
#[derive(Debug, Clone)]
pub struct Root {
    pub id: RootId,
    pub name: String,
    abs: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathError {
    Empty,
    Absolute,
    Escape,
    InvalidSegment,
    OutsideRoot,
    Io(String),
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PathError::Empty => write!(f, "path is empty"),
            PathError::Absolute => write!(f, "path must be relative to a root"),
            PathError::Escape => write!(f, "path escapes the vault root"),
            PathError::InvalidSegment => write!(f, "path has an empty or illegal segment"),
            PathError::OutsideRoot => write!(f, "resolved path is outside the root"),
            PathError::Io(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for PathError {}

impl RelPath {
    pub fn parse(raw: &str) -> Result<Self, PathError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(PathError::Empty);
        }
        let unified = trimmed.replace('\\', "/");
        if unified.starts_with('/') {
            return Err(PathError::Absolute);
        }
        let mut chars = unified.chars();
        if chars.next().is_some_and(|c| c.is_ascii_alphabetic()) && chars.next() == Some(':') {
            return Err(PathError::Absolute);
        }
        let mut parts: Vec<&str> = Vec::new();
        for seg in unified.split('/') {
            if seg.is_empty() || seg == "." {
                return Err(PathError::InvalidSegment);
            }
            if seg == ".." {
                return Err(PathError::Escape);
            }
            if seg.contains('\0') {
                return Err(PathError::InvalidSegment);
            }
            parts.push(seg);
        }
        if parts.is_empty() {
            return Err(PathError::Empty);
        }
        Ok(RelPath(parts.join("/")))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn as_path(&self) -> &Path {
        Path::new(&self.0)
    }

    /// NFC form for case-insensitive / accent-insensitive comparison.
    pub fn nfc_key(&self) -> String {
        self.0.nfc().collect::<String>().to_ascii_lowercase()
    }

    pub fn file_name(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or(&self.0)
    }

    pub fn file_stem_key(&self) -> String {
        let name = self.file_name();
        let stem = name.strip_suffix(".md").unwrap_or(name);
        stem.nfc().collect::<String>().to_ascii_lowercase()
    }
}

impl fmt::Display for RelPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl NoteId {
    pub fn new(root: RootId, path: RelPath) -> Self {
        NoteId { root, path }
    }

    pub fn vault_path(&self) -> VaultPath {
        VaultPath {
            root: self.root,
            rel: self.path.clone(),
        }
    }
}

impl Root {
    pub fn open(
        id: RootId,
        name: impl Into<String>,
        abs: impl Into<PathBuf>,
    ) -> Result<Self, PathError> {
        let abs = abs.into();
        let canon = std::fs::canonicalize(&abs).map_err(|e| PathError::Io(e.to_string()))?;
        Ok(Root {
            id,
            name: name.into(),
            abs: canon,
        })
    }

    pub fn abs_path(&self) -> &Path {
        &self.abs
    }

    pub fn resolve(&self, rel: &RelPath) -> Result<PathBuf, PathError> {
        let mut out = self.abs.clone();
        for c in Path::new(rel.as_str()).components() {
            match c {
                Component::Normal(s) => out.push(s),
                Component::CurDir => {}
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(PathError::Escape);
                }
            }
        }
        if !out.starts_with(&self.abs) {
            return Err(PathError::OutsideRoot);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn rejects_parent_and_absolute() {
        assert_eq!(RelPath::parse("../secret"), Err(PathError::Escape));
        assert_eq!(RelPath::parse("/etc/passwd"), Err(PathError::Absolute));
        assert_eq!(RelPath::parse(""), Err(PathError::Empty));
        assert_eq!(RelPath::parse("foo/./bar"), Err(PathError::InvalidSegment));
        assert_eq!(RelPath::parse("foo//bar"), Err(PathError::InvalidSegment));
    }

    #[test]
    fn accepts_nested_md() {
        let p = RelPath::parse("10-Inbox/hi.md").unwrap();
        assert_eq!(p.as_str(), "10-Inbox/hi.md");
        assert_eq!(p.file_stem_key(), "hi");
    }

    #[test]
    fn nfc_key_collapses_unicode() {
        let nfd = RelPath::parse("cafe\u{0301}.md").unwrap();
        let nfc = RelPath::parse("caf\u{e9}.md").unwrap();
        assert_eq!(nfd.nfc_key(), nfc.nfc_key());
    }

    #[test]
    fn jail_rejects_escaped_join() {
        assert_eq!(RelPath::parse(".."), Err(PathError::Escape));
        assert_eq!(RelPath::parse("../x.md"), Err(PathError::Escape));
        assert_eq!(RelPath::parse("../outside.md"), Err(PathError::Escape));
        assert_eq!(RelPath::parse("/outside.md"), Err(PathError::Absolute));

        let dir = tempdir().unwrap();
        let root = Root::open(RootId(1), "t", dir.path()).unwrap();
        let rel = RelPath::parse("ok.md").unwrap();
        let resolved = root.resolve(&rel).unwrap();
        assert!(resolved.starts_with(root.abs_path()));
    }
}
