use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use crate::path::{PathError, RelPath, Root, RootId, VaultPath};

/// Directory names that are never walked or written.
pub const OPAQUE_DIR_NAMES: &[&str] = &[".obsidian", ".trash", ".git", ".gloss"];

#[derive(Debug, Default)]
pub struct Workspace {
    roots: BTreeMap<RootId, Root>,
    next_id: u64,
}

impl Workspace {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_root(
        &mut self,
        abs: impl Into<PathBuf>,
        name: impl Into<String>,
    ) -> Result<RootId, PathError> {
        self.next_id += 1;
        let id = RootId(self.next_id);
        let root = Root::open(id, name, abs)?;
        self.roots.insert(id, root);
        Ok(id)
    }

    pub fn root(&self, id: RootId) -> Option<&Root> {
        self.roots.get(&id)
    }

    pub fn roots(&self) -> impl Iterator<Item = &Root> {
        self.roots.values()
    }

    pub fn resolve(&self, path: &VaultPath) -> Result<PathBuf, PathError> {
        let root = self.roots.get(&path.root).ok_or(PathError::OutsideRoot)?;
        root.resolve(&path.rel)
    }

    pub fn read(&self, path: &VaultPath) -> Result<Vec<u8>, PathError> {
        let abs = self.resolve(path)?;
        fs::read(&abs).map_err(|e| PathError::Io(e.to_string()))
    }

    pub fn write(&self, path: &VaultPath, bytes: &[u8]) -> Result<(), PathError> {
        let abs = self.resolve(path)?;
        if let Some(parent) = abs.parent() {
            fs::create_dir_all(parent).map_err(|e| PathError::Io(e.to_string()))?;
        }
        fs::write(&abs, bytes).map_err(|e| PathError::Io(e.to_string()))
    }

    pub fn remove(&self, path: &VaultPath) -> Result<(), PathError> {
        let abs = self.resolve(path)?;
        fs::remove_file(&abs).map_err(|e| PathError::Io(e.to_string()))
    }

    pub fn rename(&self, from: &VaultPath, to: &RelPath) -> Result<(), PathError> {
        let src = self.resolve(from)?;
        let dest_path = VaultPath {
            root: from.root,
            rel: to.clone(),
        };
        let dest = self.resolve(&dest_path)?;
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| PathError::Io(e.to_string()))?;
        }
        fs::rename(&src, &dest).map_err(|e| PathError::Io(e.to_string()))
    }
}

pub fn list_notes(root: &Root) -> Result<Vec<RelPath>, PathError> {
    let mut out = Vec::new();
    walk(root.abs_path(), root.abs_path(), &mut out)?;
    out.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    Ok(out)
}

fn walk(
    root: &std::path::Path,
    dir: &std::path::Path,
    out: &mut Vec<RelPath>,
) -> Result<(), PathError> {
    let entries = fs::read_dir(dir).map_err(|e| PathError::Io(e.to_string()))?;
    for entry in entries {
        let entry = entry.map_err(|e| PathError::Io(e.to_string()))?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if OPAQUE_DIR_NAMES.contains(&name_str.as_ref()) {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            walk(root, &path, out)?;
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .map_err(|_| PathError::OutsideRoot)?
            .to_string_lossy()
            .replace('\\', "/");
        out.push(RelPath::parse(&rel)?);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn lists_md_and_skips_obsidian() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("10-Inbox")).unwrap();
        fs::create_dir_all(dir.path().join(".obsidian")).unwrap();
        fs::write(dir.path().join("10-Inbox/a.md"), "hi").unwrap();
        fs::write(dir.path().join(".obsidian/app.json"), "{}").unwrap();
        fs::write(dir.path().join("skip.txt"), "no").unwrap();
        let mut ws = Workspace::new();
        let id = ws.add_root(dir.path(), "mini").unwrap();
        let notes = list_notes(ws.root(id).unwrap()).unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].as_str(), "10-Inbox/a.md");
    }
}
