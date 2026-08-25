---
title: Data model
type: spec
status: draft
created: 2026-08-24
tags: [gloss, architecture]
---

# Data model

## Where things live

```
~/Library/Application Support/com.gloss.app/
  workspaces/<workspace-id>/
    index.db          SQLite: notes, chunks, links, vectors, FTS
    journal.db        SQLite: patch journal, cost ledger, rejections
    models/           downloaded embedding + whisper weights
  settings.json       workspace list, provider prefs (no secrets)

<vault-root>/.gloss/
  rules.md            user-authored agent rules — committed
  ignore              gitignore syntax — committed
```

Nothing derived is stored in the vault. The index must be rebuildable from the
files alone; if index and files disagree, the files win. API keys live in the
macOS Keychain and never touch either location — see [[06-operations]].

## Index schema (sketch)

```sql
CREATE TABLE roots (
  id INTEGER PRIMARY KEY, path TEXT NOT NULL UNIQUE, name TEXT NOT NULL
);

CREATE TABLE notes (
  id INTEGER PRIMARY KEY,
  root_id INTEGER NOT NULL REFERENCES roots(id),
  rel_path TEXT NOT NULL,
  title TEXT,
  frontmatter TEXT,            -- JSON, unknown keys preserved
  content_hash BLOB NOT NULL,  -- blake3 of file bytes
  mtime INTEGER NOT NULL,
  indexed_at INTEGER NOT NULL,
  UNIQUE (root_id, rel_path)
);

CREATE TABLE chunks (
  id INTEGER PRIMARY KEY,
  note_id INTEGER NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
  ordinal INTEGER NOT NULL,
  heading_path TEXT,           -- 'Architecture > Layer model'
  byte_start INTEGER NOT NULL,
  byte_end INTEGER NOT NULL,
  chunk_hash BLOB NOT NULL     -- re-embed only when this changes
);

CREATE TABLE links (
  src_note INTEGER NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
  dst_note INTEGER REFERENCES notes(id),  -- NULL = unresolved
  raw_target TEXT NOT NULL,
  kind TEXT NOT NULL,          -- wikilink | markdown | tag | embed | code_ref
  byte_start INTEGER NOT NULL
);

CREATE TABLE code_refs (
  note_id INTEGER NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
  repo_root TEXT NOT NULL,
  target TEXT NOT NULL,        -- path, or path#symbol
  last_seen_rev TEXT,          -- git oid where it last resolved
  status TEXT NOT NULL         -- ok | moved | missing | stale
);

-- sqlite-vec virtual table for chunk embeddings
-- tantivy index lives on disk beside index.db
```

`(root_id, rel_path)` everywhere. There is no bare path type in the codebase.

## Patch and checkpoint format

Patches are **structured ops, not raw text diffs**. Text diffs can't express
"rename this note and rewrite every inbound link," which is most of what the
agent actually does.

```rust
enum Op {
    CreateNote  { root: RootId, path: RelPath, content: String },
    ReplaceSpan { note: NoteId, range: ByteRange, text: String, expect_hash: Hash },
    SetFrontmatter { note: NoteId, key: String, value: Value },
    RenameNote  { note: NoteId, to: RelPath, rewrite_backlinks: bool },
    DeleteNote  { note: NoteId },
    MergeNotes  { from: Vec<NoteId>, into: NoteId, strategy: MergeStrategy },
}

struct Patch { id: PatchId, run: RunId, ops: Vec<Op>, rationale: String,
               provenance: Provenance, cost: TokenCost }
```

Rules that make this safe:

- **`expect_hash` on every mutating op.** If the note changed since the patch was
  generated, the op fails rather than clobbering. This is the whole answer to
  concurrent human/agent editing, and it must be there from the first commit.
- **Ops are individually reversible.** Apply produces an inverse patch, stored in
  the journal.
- **A run is a checkpoint.** One git commit per agent run, tagged with the
  `RunId`. Undo reverts the run, not the file.
- **Provenance travels with the patch** — which model, which prompt version,
  which retrieved notes. When a suggestion is wrong six months later you need to
  know why it was made.

## Rejections are data

```sql
CREATE TABLE rejections (
  patch_id TEXT PRIMARY KEY, run_id TEXT NOT NULL,
  op_kind TEXT NOT NULL, context TEXT NOT NULL,  -- JSON
  rejected_at INTEGER NOT NULL
);
```

Every rejected hunk is a labelled negative example. On BYOK this is the only
training signal available, and it's the corpus a custom Tab model would
eventually be built from.

## Cost ledger

```sql
CREATE TABLE spend (
  id INTEGER PRIMARY KEY, run_id TEXT, feature TEXT NOT NULL,
  provider TEXT NOT NULL, model TEXT NOT NULL,
  input_tokens INTEGER, cached_tokens INTEGER, output_tokens INTEGER,
  usd_estimate REAL, at INTEGER NOT NULL
);
```

When it's the user's key, spend has to be visible per operation. This forces the
budget-gate problem to be solved in M1 instead of discovered at scale.

## Config

`.gloss/rules.md` — plain Markdown, committed to the vault, read into the system
prompt. Sections are conventions, not schema:

```markdown
## Filing
Meeting notes go in `10-Meetings/YYYY/`. Filename `YYYY-MM-DD-<slug>`.

## Linking
Prefer linking to an existing note over creating a new one.
Never create a note with fewer than three sentences of substance.

## Never touch
`90-Archive/**`, `**/*.excalidraw.md`
```
