//! Derived vault index (L1). Rebuildable from files; files win.
//!
//! First slice: notes + resolved wikilinks/embeds, plus title-mention
//! missing-link suggestions. Lexical search, embeddings, and the cost
//! ledger stay later in M1.

mod suggest;

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::Path;

use gloss_core::{
    hash_bytes, list_notes, resolve_link_target, split_frontmatter, wikilinks, ContentHash,
    LinkKind, NoteId, PathError, RelPath, RootId, VaultPath, Workspace,
};
use rusqlite::{params, Connection, OptionalExtension};

pub use suggest::SuggestedLink;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS roots (
  id INTEGER PRIMARY KEY,
  path TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS notes (
  id INTEGER PRIMARY KEY,
  root_id INTEGER NOT NULL REFERENCES roots(id) ON DELETE CASCADE,
  rel_path TEXT NOT NULL,
  title TEXT,
  frontmatter TEXT,
  content_hash BLOB NOT NULL,
  mtime INTEGER NOT NULL,
  indexed_at INTEGER NOT NULL,
  UNIQUE (root_id, rel_path)
);

CREATE TABLE IF NOT EXISTS links (
  src_note INTEGER NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
  dst_note INTEGER REFERENCES notes(id) ON DELETE SET NULL,
  raw_target TEXT NOT NULL,
  kind TEXT NOT NULL,
  byte_start INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS links_src ON links(src_note);
CREATE INDEX IF NOT EXISTS links_dst ON links(dst_note);
";

#[derive(Debug)]
pub enum IndexError {
    Sql(rusqlite::Error),
    Path(PathError),
    Io(String),
}

impl fmt::Display for IndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IndexError::Sql(e) => write!(f, "{e}"),
            IndexError::Path(e) => write!(f, "{e}"),
            IndexError::Io(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for IndexError {}

impl From<rusqlite::Error> for IndexError {
    fn from(e: rusqlite::Error) -> Self {
        IndexError::Sql(e)
    }
}

impl From<PathError> for IndexError {
    fn from(e: PathError) -> Self {
        IndexError::Path(e)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RefreshStats {
    pub scanned: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub deleted: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedNote {
    pub id: NoteId,
    pub title: String,
    pub content_hash: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedLink {
    pub src: NoteId,
    pub dst: Option<NoteId>,
    pub raw_target: String,
    pub kind: LinkKind,
    pub byte_start: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Neighbors {
    pub outgoing: Vec<NoteId>,
    pub incoming: Vec<NoteId>,
}

pub struct Index {
    conn: Connection,
}

impl Index {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, IndexError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| IndexError::Io(e.to_string()))?;
        }
        let conn = Connection::open(path)?;
        Self::from_conn(conn)
    }

    pub fn open_memory() -> Result<Self, IndexError> {
        Self::from_conn(Connection::open_in_memory()?)
    }

    fn from_conn(conn: Connection) -> Result<Self, IndexError> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Index { conn })
    }

    /// Incremental refresh. Unchanged content hashes are left alone.
    pub fn refresh(&mut self, ws: &Workspace, now: i64) -> Result<RefreshStats, IndexError> {
        let tx = self.conn.transaction()?;
        upsert_roots(&tx, ws)?;

        let existing = load_existing(&tx)?;
        let mut keep: HashSet<(u64, String)> = HashSet::new();
        let mut stats = RefreshStats::default();

        for root in ws.roots() {
            let notes = list_notes(root)?;
            for rel in &notes {
                stats.scanned += 1;
                let vault = VaultPath {
                    root: root.id,
                    rel: rel.clone(),
                };
                let bytes = ws.read(&vault)?;
                let hash = hash_bytes(&bytes);
                let key = (root.id.0, rel.as_str().to_string());
                keep.insert(key.clone());
                if existing.get(&key).is_some_and(|h| h == &hash) {
                    stats.unchanged += 1;
                    continue;
                }
                upsert_note(
                    &tx,
                    root.id,
                    rel,
                    &bytes,
                    hash,
                    file_mtime(ws, &vault)?,
                    now,
                )?;
                stats.updated += 1;
            }
        }

        for key in existing.keys() {
            if !keep.contains(key) {
                tx.execute(
                    "DELETE FROM notes WHERE root_id = ?1 AND rel_path = ?2",
                    params![key.0, key.1],
                )?;
                stats.deleted += 1;
            }
        }

        if stats.updated > 0 || stats.deleted > 0 {
            rewrite_links(&tx, ws)?;
        }
        tx.commit()?;
        Ok(stats)
    }

    pub fn notes(&self) -> Result<Vec<IndexedNote>, IndexError> {
        let mut stmt = self.conn.prepare(
            "SELECT root_id, rel_path, title, content_hash FROM notes ORDER BY rel_path",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(IndexedNote {
                id: note_id(row.get(0)?, row.get(1)?),
                title: row
                    .get::<_, Option<String>>(2)?
                    .unwrap_or_else(|| stem_title(&row.get::<_, String>(1).unwrap())),
                content_hash: blob_hash(row.get(3)?),
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(IndexError::from)
    }

    pub fn outgoing(&self, note: &NoteId) -> Result<Vec<IndexedLink>, IndexError> {
        self.links_for(note, true)
    }

    pub fn incoming(&self, note: &NoteId) -> Result<Vec<IndexedLink>, IndexError> {
        self.links_for(note, false)
    }

    pub fn neighbors(&self, note: &NoteId) -> Result<Neighbors, IndexError> {
        Ok(Neighbors {
            outgoing: unique_dsts(self.outgoing(note)?),
            incoming: unique_srcs(self.incoming(note)?),
        })
    }

    fn links_for(&self, note: &NoteId, outgoing: bool) -> Result<Vec<IndexedLink>, IndexError> {
        let sql = if outgoing {
            "SELECT s.root_id, s.rel_path, d.root_id, d.rel_path, l.raw_target, l.kind, l.byte_start
             FROM links l
             JOIN notes s ON s.id = l.src_note
             LEFT JOIN notes d ON d.id = l.dst_note
             WHERE s.root_id = ?1 AND s.rel_path = ?2
             ORDER BY l.byte_start"
        } else {
            "SELECT s.root_id, s.rel_path, d.root_id, d.rel_path, l.raw_target, l.kind, l.byte_start
             FROM links l
             JOIN notes d ON d.id = l.dst_note
             JOIN notes s ON s.id = l.src_note
             WHERE d.root_id = ?1 AND d.rel_path = ?2
             ORDER BY l.byte_start"
        };
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(params![note.root.0, note.path.as_str()], |row| {
            let dst_root: Option<u64> = row.get(2)?;
            let dst_path: Option<String> = row.get(3)?;
            Ok(IndexedLink {
                src: note_id(row.get(0)?, row.get(1)?),
                dst: match (dst_root, dst_path) {
                    (Some(r), Some(p)) => Some(note_id(r, p)),
                    _ => None,
                },
                raw_target: row.get(4)?,
                kind: parse_kind(&row.get::<_, String>(5)?),
                byte_start: row.get::<_, i64>(6)? as usize,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(IndexError::from)
    }
}

fn upsert_roots(tx: &rusqlite::Transaction<'_>, ws: &Workspace) -> Result<(), IndexError> {
    let live: HashSet<u64> = ws.roots().map(|r| r.id.0).collect();
    tx.execute(
        &format!(
            "DELETE FROM roots WHERE id NOT IN ({})",
            if live.is_empty() {
                "NULL".into()
            } else {
                live.iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            }
        ),
        [],
    )?;
    for root in ws.roots() {
        tx.execute(
            "INSERT INTO roots (id, path, name) VALUES (?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET path = excluded.path, name = excluded.name",
            params![
                root.id.0,
                root.abs_path().to_string_lossy().as_ref(),
                root.name.as_str()
            ],
        )?;
    }
    Ok(())
}

fn load_existing(
    tx: &rusqlite::Transaction<'_>,
) -> Result<HashMap<(u64, String), ContentHash>, IndexError> {
    let mut stmt = tx.prepare("SELECT root_id, rel_path, content_hash FROM notes")?;
    let rows = stmt.query_map([], |row| {
        Ok((
            (row.get::<_, u64>(0)?, row.get::<_, String>(1)?),
            blob_hash(row.get(2)?),
        ))
    })?;
    rows.collect::<Result<HashMap<_, _>, _>>()
        .map_err(IndexError::from)
}

fn upsert_note(
    tx: &rusqlite::Transaction<'_>,
    root: RootId,
    rel: &RelPath,
    bytes: &[u8],
    hash: ContentHash,
    mtime: i64,
    now: i64,
) -> Result<(), IndexError> {
    let src = String::from_utf8_lossy(bytes);
    let split = split_frontmatter(&src);
    let yaml = split.as_ref().map(|s| s.yaml);
    let title = yaml
        .and_then(title_from_yaml)
        .unwrap_or_else(|| stem_title(rel.as_str()));
    tx.execute(
        "INSERT INTO notes (root_id, rel_path, title, frontmatter, content_hash, mtime, indexed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(root_id, rel_path) DO UPDATE SET
           title = excluded.title,
           frontmatter = excluded.frontmatter,
           content_hash = excluded.content_hash,
           mtime = excluded.mtime,
           indexed_at = excluded.indexed_at",
        params![
            root.0,
            rel.as_str(),
            title,
            yaml,
            hash.0.as_slice(),
            mtime,
            now
        ],
    )?;
    Ok(())
}

fn rewrite_links(tx: &rusqlite::Transaction<'_>, ws: &Workspace) -> Result<(), IndexError> {
    tx.execute("DELETE FROM links", [])?;
    for root in ws.roots() {
        let notes = list_notes(root)?;
        for rel in &notes {
            let src_id = note_row_id(tx, root.id, rel.as_str())?.expect("note just indexed");
            let bytes = ws.read(&VaultPath {
                root: root.id,
                rel: rel.clone(),
            })?;
            let src = String::from_utf8_lossy(&bytes);
            for link in wikilinks(&src) {
                let dst = resolve_link_target(&link.raw_target, &notes)
                    .and_then(|p| note_row_id(tx, root.id, p.as_str()).ok().flatten());
                tx.execute(
                    "INSERT INTO links (src_note, dst_note, raw_target, kind, byte_start)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        src_id,
                        dst,
                        link.raw_target,
                        kind_name(link.kind),
                        link.range.start as i64
                    ],
                )?;
            }
        }
    }
    Ok(())
}

fn note_row_id(
    tx: &rusqlite::Transaction<'_>,
    root: RootId,
    rel: &str,
) -> Result<Option<i64>, IndexError> {
    tx.query_row(
        "SELECT id FROM notes WHERE root_id = ?1 AND rel_path = ?2",
        params![root.0, rel],
        |row| row.get(0),
    )
    .optional()
    .map_err(IndexError::from)
}

fn file_mtime(ws: &Workspace, path: &VaultPath) -> Result<i64, IndexError> {
    let abs = ws.resolve(path)?;
    mtime_of(&abs)
}

fn mtime_of(path: &Path) -> Result<i64, IndexError> {
    let meta = std::fs::metadata(path).map_err(|e| IndexError::Io(e.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(meta.mtime())
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        Ok(0)
    }
}

fn title_from_yaml(yaml: &str) -> Option<String> {
    for line in yaml.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("title:") else {
            continue;
        };
        let rest = rest.trim();
        if rest.is_empty() {
            continue;
        }
        let unquoted = rest
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .or_else(|| rest.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
            .unwrap_or(rest);
        if !unquoted.is_empty() {
            return Some(unquoted.to_string());
        }
    }
    None
}

pub(crate) fn stem_title(rel: &str) -> String {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    name.strip_suffix(".md")
        .or_else(|| name.strip_suffix(".MD"))
        .unwrap_or(name)
        .to_string()
}

fn note_id(root: u64, rel: String) -> NoteId {
    NoteId::new(RootId(root), RelPath::parse(&rel).expect("stored path"))
}

fn blob_hash(bytes: Vec<u8>) -> ContentHash {
    let arr: [u8; 32] = bytes.try_into().expect("blake3 hash is 32 bytes");
    ContentHash(arr)
}

fn kind_name(kind: LinkKind) -> &'static str {
    match kind {
        LinkKind::Wikilink => "wikilink",
        LinkKind::Embed => "embed",
    }
}

fn parse_kind(raw: &str) -> LinkKind {
    match raw {
        "embed" => LinkKind::Embed,
        _ => LinkKind::Wikilink,
    }
}

fn unique_dsts(links: Vec<IndexedLink>) -> Vec<NoteId> {
    let mut out = Vec::new();
    for link in links {
        if let Some(dst) = link.dst {
            if !out.contains(&dst) {
                out.push(dst);
            }
        }
    }
    out
}

fn unique_srcs(links: Vec<IndexedLink>) -> Vec<NoteId> {
    let mut out = Vec::new();
    for link in links {
        if !out.contains(&link.src) {
            out.push(link.src);
        }
    }
    out
}

#[cfg(test)]
mod title_tests {
    use super::{stem_title, title_from_yaml};

    #[test]
    fn reads_plain_and_quoted_title() {
        assert_eq!(title_from_yaml("title: Atlas\n"), Some("Atlas".into()));
        assert_eq!(title_from_yaml("title: \"Hello\"\n"), Some("Hello".into()));
        assert_eq!(stem_title("10-Inbox/Welcome.md"), "Welcome");
    }
}
