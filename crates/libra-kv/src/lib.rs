//! libra-kv — the persistence store (ARCHITECTURE.md §13.1):
//! localStorage, but honest. redb (pure-Rust, ACID, single-file B-tree)
//! in the platform app-data directory. String-ish keys, byte values,
//! prefix scans. All access goes through `Cmd::Kv*` on effects workers —
//! apps never touch this crate directly.

use redb::{Database, TableDefinition};
use std::path::{Path, PathBuf};

const TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("kv");

pub struct Kv {
    db: Database,
}

impl Kv {
    /// Open (creating if needed) a store at an explicit path. Tests use
    /// this with a temp dir.
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let db = Database::create(path).map_err(|e| e.to_string())?;
        Ok(Kv { db })
    }

    /// The production location: `<platform app-data dir>/<app_id>/kv.redb`
    /// (`%APPDATA%`, `~/Library/Application Support`, `~/.local/share`).
    pub fn open_default(app_id: &str) -> Result<Self, String> {
        let dirs = directories::ProjectDirs::from("", "", app_id)
            .ok_or("no home directory available")?;
        Self::open(&dirs.data_dir().join("kv.redb"))
    }

    pub fn default_path(app_id: &str) -> Option<PathBuf> {
        directories::ProjectDirs::from("", "", app_id)
            .map(|d| d.data_dir().join("kv.redb"))
    }

    pub fn set(&self, key: &str, value: &[u8]) -> Result<(), String> {
        let tx = self.db.begin_write().map_err(|e| e.to_string())?;
        {
            let mut t = tx.open_table(TABLE).map_err(|e| e.to_string())?;
            t.insert(key, value).map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())
    }

    pub fn get(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        let tx = self.db.begin_read().map_err(|e| e.to_string())?;
        let t = match tx.open_table(TABLE) {
            Ok(t) => t,
            // A store nothing was ever written to has no table yet.
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(e) => return Err(e.to_string()),
        };
        Ok(t.get(key).map_err(|e| e.to_string())?.map(|g| g.value().to_vec()))
    }

    pub fn delete(&self, key: &str) -> Result<(), String> {
        let tx = self.db.begin_write().map_err(|e| e.to_string())?;
        {
            let mut t = tx.open_table(TABLE).map_err(|e| e.to_string())?;
            t.remove(key).map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())
    }

    /// All `(key, value)` pairs whose key starts with `prefix`, in key order.
    pub fn scan_prefix(&self, prefix: &str) -> Result<Vec<(String, Vec<u8>)>, String> {
        let tx = self.db.begin_read().map_err(|e| e.to_string())?;
        let t = match tx.open_table(TABLE) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(vec![]),
            Err(e) => return Err(e.to_string()),
        };
        let mut out = Vec::new();
        for entry in t.range(prefix..).map_err(|e| e.to_string())? {
            let (k, v) = entry.map_err(|e| e.to_string())?;
            let key = k.value();
            if !key.starts_with(prefix) {
                break;
            }
            out.push((key.to_string(), v.value().to_vec()));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_kv() -> (tempfile::TempDir, Kv) {
        let dir = tempfile::tempdir().expect("tempdir");
        let kv = Kv::open(&dir.path().join("kv.redb")).expect("open");
        (dir, kv)
    }

    #[test]
    fn set_get_delete_roundtrip() {
        let (_dir, kv) = temp_kv();
        assert_eq!(kv.get("session/current").unwrap(), None);
        kv.set("session/current", b"abc").unwrap();
        assert_eq!(kv.get("session/current").unwrap().as_deref(), Some(&b"abc"[..]));
        kv.delete("session/current").unwrap();
        assert_eq!(kv.get("session/current").unwrap(), None);
    }

    #[test]
    fn scan_returns_prefix_matches_in_key_order() {
        let (_dir, kv) = temp_kv();
        kv.set("settings/theme", b"dark").unwrap();
        kv.set("settings/density", b"compact").unwrap();
        kv.set("session/current", b"x").unwrap();
        let hits = kv.scan_prefix("settings/").unwrap();
        assert_eq!(
            hits,
            vec![
                ("settings/density".to_string(), b"compact".to_vec()),
                ("settings/theme".to_string(), b"dark".to_vec()),
            ]
        );
    }

    #[test]
    fn values_survive_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kv.redb");
        Kv::open(&path).unwrap().set("k", b"v").unwrap();
        assert_eq!(Kv::open(&path).unwrap().get("k").unwrap().as_deref(), Some(&b"v"[..]));
    }

    #[test]
    fn empty_store_scans_and_gets_cleanly() {
        let (_dir, kv) = temp_kv();
        assert_eq!(kv.scan_prefix("any/").unwrap(), vec![]);
        assert_eq!(kv.get("any").unwrap(), None);
    }
}
