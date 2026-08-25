---
title: Roadmap
type: plan
status: draft
created: 2026-08-24
tags: [gloss, planning]
---

# Roadmap

Sequenced for an audience of one, building nights and weekends around a
demanding day job. Every milestone has an exit criterion that is a *thing I use*,
not a thing I finished.

## Dependency spine

```
M0 Vault core ──┬── M1 Index ──┬── M2 MCP server        (ship first)
                │              ├── M3 Review surface
                │              │      └── M4 Editor shell ── M5 Tab
                │              └── R1 Code-guide staleness
                └── R2 Audio capture  (parallel, starts early)
```

Three items carry real schedule risk and none are on the critical path, so they
start early and run in parallel: **audio capture** (per-OS, Core Audio taps are
finicky), **a custom Tab model** (deferred entirely — hosted-era problem), and
**sync** (inherited the moment you go standalone; deferred to the user's
existing solution in v1).

## M0 — Vault core

**Landed (usable).** Watcher and git checkpoints are still open. Parser, patch
layer, and multi-root path types are in `gloss-core`.

- `notify` watcher with debounce and rename coalescing
- comrak + wikilink extension; frontmatter round-trips losslessly
- Patch layer with `expect_hash`, inverse-patch generation, run checkpoints
- Path jail; `(RootId, RelPath)` is the only path type

**Exit:** a CLI can apply and undo a multi-op patch against my real vault
without losing a byte. Property tests prove round-trip and inverse correctness.

**Trap:** the temptation to `fs::write` "just for the importer." Don't. Once
there are two write paths there are five.

## M1 — Index

Link graph, frontmatter, chunk hashing, local embeddings, hybrid retrieval.

- SQLite + tantivy + sqlite-vec, one store per workspace
- fastembed-rs with bge-small; incremental re-embed on chunk-hash change
- Hybrid ranker: BM25 + cosine + graph proximity, RRF-fused
- Cost ledger and the local budget gate — before anything calls a frontier model

**Exit:** full index of my vault rebuilds from cold in under two minutes, and
incremental update after a single-note edit is under 200ms.

## M2 — MCP server — *first real value*

`gloss-mcp` over stdio. Claude Code reads my vault as its guide.

- `search_notes`, `get_note`, `graph_neighbors`, `stale_docs`, `capture_note`
- No UI. No editor. No audio. Only L0 + L1.

**Exit:** I stop pasting context into Claude Code manually, for a full week.

This is the honest milestone-one: daily-used, proves the relational engine under
load, and validates L0/L1 before any UI exists to hide their problems.

## R1 — Code-guide staleness *(parallel with M2)*

`code_refs` in frontmatter; resolve against repo git state; flag moved/missing.

**Exit:** a stale-docs report on a real repo that catches something I'd missed.

## M3 — Review surface

Diff generation and apply/reject. The unlock for every proactive feature.

- Per-hunk accept/reject; accept-all-in-run; undo-whole-run via checkpoint
- Rejection logging with context
- **Then, immediately on top of it:** retroactive link pass, inbox amnesty,
  near-duplicate merge — the [[01-overview#First-session value (the launch demo)|launch demo]] features

**Exit:** the retroactive link pass runs on my vault and I accept more hunks than
I reject. If that ratio is upside down, the ranker in M1 is wrong — stop and fix
it rather than proceeding to M4.

## R2 — Audio capture *(started during M0, lands whenever it lands)*

Swift sidecar: `AVAudioEngine` mic, Core Audio process taps for system audio,
whisper.cpp or MLX Parakeet for local transcription, then structuring into a
filed note via M3's review surface. Diarization is post-v1.

**Exit:** a half-hour call becomes a filed, linked, structured note I keep.

## M4 — Editor shell

**Started.** Libra app with vault path, note list, `<Textarea>`, save through
the patch layer. Still missing `@`-mentions, Cmd+K, and a rules file.

**Exit:** I write in Gloss instead of Obsidian for a week without missing
anything badly enough to switch back.

## M5 — Tab

Ghost-text inline completion using a fast frontier model with prompt caching.
Context = current block + retrieved neighbors + open tabs.

**Exit:** acceptance rate above 25% on my own writing.

## Deliberately after v1

Mobile capture · sync · diarization · custom Tab model · hosted subscription ·
teams and shared vaults · Windows/Linux · plugin API.

## Reassessment gate

After M2 + R1, stop and ask whether anyone other than me wants the MCP server and
the stale-docs report. That answer decides whether Gloss is a product with the
[[01-overview#The wedge vault-as-codebase-guide|engineering wedge]] or a very
good personal tool. Both are acceptable outcomes; conflating them is not.
