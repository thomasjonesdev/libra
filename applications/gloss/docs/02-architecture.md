---
title: Architecture
type: spec
status: draft
created: 2026-08-24
tags: [gloss, architecture]
---

# Architecture

## Principles

1. **The core is headless.** Every capability lives in a Rust library that knows
   nothing about a UI. The app is one frontend; the MCP server is another; a CLI
   is a third. This is what lets M2 ship before any editor exists.
2. **One write path.** Agent edits, user edits, imports, migrations — all
   mutations go through the patch layer. No exceptions, no direct `fs::write`.
3. **Local by default, remote by escalation.** Indexing and embedding are local
   and free. Only generation costs the user money.
4. **The vault is the source of truth.** The index is a derived cache and must be
   fully rebuildable from the files. If they disagree, the files win.

## Layer model

Each layer depends only on those above it.

```
L0  Vault core      watcher · parser · patch layer · git checkpoints
L1  Index           link graph · frontmatter · hashing · embeddings · search
L2  Review surface  diff generation · apply/reject · run checkpoints
L3  Editor shell    Libra `<Textarea>` · @-mentions · Cmd+K · Tab
L4  Branches        MCP server · audio capture · code-guide staleness
```

L4 branches need only L0/L1, which is why they run in parallel with everything
else. See [[04-roadmap]].

## Process model

```
┌─────────────────────────────────────────────┐
│  Libra app (macOS)                          │
│  views/gloss.lx · <Markdown> · <Textarea>   │
│            │ Effect::Process (stand-in)     │
│            │ Effect::PickFolder             │
│  ┌─────────┴──────────────────────────────┐ │
│  │  gloss-core · gloss-index              │ │
│  │  vault · patch · parse · link graph    │ │
│  └─────────┬──────────────────────────────┘ │
└────────────┼────────────────────────────────┘
             │
    ┌────────┴────────┐
    │ gloss CLI / MCP │  same crates
    └─────────────────┘
```

The core is a library, not a daemon, in v1. Keep it free of `libra-platform`
types so the CLI and a later MCP server stay mechanical. See
[[adr-009-libra-native]].

Vault bytes still leave reduce as `Effect::Process` to this binary's CLI.
`Effect::FileRead` / `FileWrite` are sketched in §12.1 and are not in the
enum. Do not `fs::read` inside an action to "just make it faster." The
split of Gloss work vs Libra work is [[10-libra]].

## Components

### L0 — Vault core

| Concern | Choice | Notes |
| --- | --- | --- |
| File watching | none yet | Open lists notes. A `notify` watcher is still open. |
| Markdown parse | byte scanner in `parse.rs` | Wikilinks, embeds, `%%` comments. Not comrak. |
| Frontmatter | raw YAML span | Unknown keys stay as bytes. No `serde_yaml` rewrite. |
| Path safety | Vault-scoped jail | Reject any resolved path outside a root |
| Undo log | `.gloss/journal.json` | Inverse ops from `apply`. `gix` checkpoints are still open. |

**Multi-root from commit #1.** Every path is `(RootId, RelPath)`, never a bare
`PathBuf`. Retrofitting this means rewriting every query, cache key, and path
assumption. See [[decisions/adr-001-multi-root]].

**The patch layer** is the only thing that touches disk. See
[[03-data-model#Patch and checkpoint format]] and
[[decisions/adr-002-patch-layer]].

### L1 — Index

| Concern | Choice | Notes |
| --- | --- | --- |
| Store | SQLite (`rusqlite`, WAL) | **Started** in `gloss-index`. Notes + links + title-mention suggestions. One DB per workspace, outside the vault |
| Lexical search | `tantivy` | BM25 — still beats embeddings on proper nouns |
| Vectors | `sqlite-vec` | Keeps one store; avoids a second index to corrupt |
| Embeddings | `fastembed-rs`, bge-small-en-v1.5 | Local, CPU, no API cost |
| Invalidation | blake3 content hash per chunk | Re-embed only changed chunks |

**Retrieval is hybrid and the graph is the differentiator.** The vault already
contains an explicit graph in wikilinks, tags, and frontmatter. Pure vector RAG
over notes is a commodity and does poorly on exactly the "keep things together"
job. Rank by: BM25 + cosine similarity + graph proximity (hops from the active
note), reciprocal-rank-fused.

**Budget gate.** Continuous LLM passes over a large vault are a token firehose,
and on BYOK it's the user's money. Cheap local signals — hash changed, embedding
drift beyond threshold, orphan status, link-density outlier — decide *whether*
a note merits a frontier-model pass. Target: fewer than 5% of touched notes
escalate.

### L2 — Review surface

Build this third, not tenth. Every proactive feature is blocked on it, and it's
what converts the propose-only constraint into a feature rather than a tax.

- Patches are structured ops, not raw text diffs — see [[03-data-model]]
- Per-hunk accept/reject, plus accept-all-in-run
- Every agent run is one git checkpoint; undo reverts the whole run
- A rejected patch is a training signal: log the rejection with its context

### L3 — Editor shell

Libra `<Textarea>` for v1 (PLAN-TEXTAREA stage 1: hard newlines, no IME, no
soft wrap). Preview paint is `<Markdown>` (`libra-markdown` already draws
emphasis and wikilinks). Ghost-text Tab is the same kind of paint on a
plain buffer. It is a Textarea overlay, not a hole in §24. See
[[adr-009-libra-native]], [[10-libra]], and [[11-coherence]]. Do not host
a webview to dodge it.

- **Tab** — ghost text; context = current block + retrieved neighbors + open tabs
- **Cmd+K** — inline edit on selection, returns a patch, not a replacement
- **`@`-mentions** — `@note`, `@folder`, `@tag`, `@repo`. This is also the answer
  to "relations without hard syntax": `@` is existing muscle memory, and
  accepting a mention silently emits the wikilink.
- **Quick open (C)** — `<Dialog>` + filter over vault paths; catalog
  `CommandPalette` replaces the shell later. See [[09-app-functions]].
- **Daily note (D)** — open or create `Daily/YYYY-MM-DD.md` (Obsidian-style
  search order). Shipped.
- **Links drawer (G)** — outgoing wikilinks from the open note + full-vault
  backlink scan via `gloss backlinks`. Not a graph viz — that waits on M1.
- **Rules file** — `.gloss/rules.md`, per-workspace, plain Markdown. This is how
  a methodology like Tracer becomes user-authorable.

### L4 — Branches

**MCP server** (`gloss-mcp`). Cheapest high-leverage artifact in the project.
Tools: `search_notes`, `get_note`, `graph_neighbors`, `stale_docs`,
`propose_patch`, `capture_note`. stdio for Claude Code, HTTP for everything else.

**Audio capture** (Swift sidecar). Mic via `AVAudioEngine`; system audio via
Core Audio process taps (macOS 14.2+, `CATapDescription`) with ScreenCaptureKit
as the fallback path. Transcription local via whisper.cpp or Parakeet on MLX.
Diarization is the hard part and is explicitly post-v1.

**Code-guide staleness.** Notes declare `code_refs` in frontmatter pointing at
repo paths or symbols. On repo change, resolve refs; unresolvable or moved refs
flag the note. This is [[Tracer]]'s staleness detection shipped as a product
feature.

### LLM provider abstraction

One trait, three implementations from day one: Anthropic, OpenAI, and a local
OpenAI-compatible endpoint (Ollama / LM Studio / MLX). The local path is what
keeps work notes off a third-party API and makes dogfooding on a sensitive vault
possible at all.

```rust
trait Provider {
    async fn complete(&self, req: CompletionRequest) -> Result<CompletionStream>;
    fn capabilities(&self) -> Capabilities; // caching, tools, ctx window
    fn cost_per_token(&self) -> Option<CostTable>;
}
```

Prompt caching on the vault-context prefix is the single biggest cost lever.
Design the prompt so the stable part comes first.

`Effect::Http` can carry a whole completion. A visible token stream waits on
PLAN-STREAM-DELIVERY. Keys do not go in the vault and should not go on a
Process argv. BYOK needs a Keychain effect before I will point this at a
real vault. See [[06-operations]] and [[10-libra]].
