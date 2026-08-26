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

What and why: [[11-coherence]]. That page is the product. This page is
the order we build it.

Gloss lives in the Libra repo. M1 and M2 are almost entirely Gloss
crates. Split framework work in [[10-libra]] before adding a catalog
node or a new `Effect` "so Gloss can ship."

**Order vs reality.** We already built a usable editor (M4) before the
review surface (M3). Do not spend the next pass making that editor look
like Obsidian. The first-session proof is 340 links and inbox amnesty.
Those need M1 and M3. Daily use is MCP (M2). The window is how you
review the bundle.

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

**Landed (usable).** Watcher and git checkpoints are still open. The byte
scanner, patch layer, and multi-root path types are in `gloss-core`.

- Byte scanner for wikilinks, embeds, comments, and a raw YAML split
- Patch layer with `expect_hash`, inverse ops, and `.gloss/journal.json`
- Path jail; `(RootId, RelPath)` is the only path type
- Still open: `notify` watcher, `gix` checkpoints

**Exit:** a CLI can apply and undo a multi-op patch against my real vault
without losing a byte. Property tests prove round-trip and inverse correctness.

**Libra:** none. Watcher as a new effect is optional; poll if the app is open.

**Trap:** the temptation to `fs::write` "just for the importer." Don't. Once
there are two write paths there are five.

## M1 — Index

**Started.** `gloss-index` persists notes + resolved wikilinks/embeds in SQLite
(WAL), refresh is incremental on blake3, and `gloss index` / `gloss neighbors`
read it. Title-mention missing links are queryable (`gloss suggest-links`) —
the first "340 connections" signal, no embeddings yet. Still open: chunks,
tantivy, sqlite-vec / embeddings, hybrid rank, cost ledger.

- SQLite + tantivy + sqlite-vec, one store per workspace
- fastembed-rs with bge-small; incremental re-embed on chunk-hash change
- Hybrid ranker: BM25 + cosine + graph proximity, RRF-fused
- Cost ledger and the local budget gate — before anything calls a frontier model

**Exit:** full index of my vault rebuilds from cold in under two minutes, and
incremental update after a single-note edit is under 200ms.

**Libra:** none for the crate or the CLI. A live watcher would make
incremental refresh while the window is open less dumb. Not a gate.

## M2 — MCP server — *first real value*

`gloss-mcp` over stdio. Claude Code reads my vault as its guide.

- `search_notes`, `get_note`, `graph_neighbors`, `stale_docs`, `capture_note`
- No UI. No editor. No audio. Only L0 + L1.

**Exit:** I stop pasting context into Claude Code manually, for a full week.

This is the honest milestone-one: daily-used, proves the relational engine under
load, and validates L0/L1 before any UI exists to hide their problems.

**Libra:** none. If a framework task is in the way of `gloss-mcp`, drop the
framework task.

## R1 — Code-guide staleness *(parallel with M2)*

`code_refs` in frontmatter; resolve against repo git state; flag moved/missing.

**Exit:** a stale-docs report on a real repo that catches something I'd missed.

**Libra:** none.

## M3 — Review surface

Diff generation and apply/reject. The unlock for every proactive feature.

- Per-hunk accept/reject; accept-all-in-run; undo-whole-run via checkpoint
- Rejection logging with context
- **Then, immediately on top of it:** the 340-link pass and inbox
  amnesty from [[11-coherence]]

**Exit:** the retroactive link pass runs on my vault and I accept more hunks than
I reject. If that ratio is upside down, the ranker in M1 is wrong. Stop and fix
it. The editor already exists, so "proceeding to M4" is not the threat. The
threat is decorating the shell instead of fixing the ranker.

**Libra:** none for a CLI review. In-window hunks are State rows first
(`hunk_rows` + ListItem). A `Diff` catalog node only if that repeats badly.
Cmd+K inline edit also needs the current Textarea selection in the action
payload. See [[10-libra]].

## R2 — Audio capture *(started during M0, lands whenever it lands)*

Swift sidecar: `AVAudioEngine` mic, Core Audio process taps for system audio,
whisper.cpp or MLX Parakeet for local transcription, then structuring into a
filed note via M3's review surface. Diarization is post-v1.

**Exit:** a half-hour call becomes a filed, linked, structured note I keep.

**Libra:** none. Sidecar plus a patch. Filing goes through M3.

## M4 — Editor shell

**Started.** Libra app with Browse (native folder sheet), note list, preview-first
`<Markdown>` (display-only), soft-wrapped `<Textarea>` behind Edit, save and undo
through the patch journal. Rail functions: quick open, daily note, link/backlink
drawer — see [[09-app-functions]]. **Landed:** `@`-mentions, `.gloss/rules.md`
(read + settings excerpt), keyboard shortcuts (§19.2), catalog chrome. Still
missing: Cmd+K inline edit (blocked on M3 review surface).

**Exit:** I write in Gloss instead of Obsidian for a week without missing
anything badly enough to switch back.

**Libra (this is the gate):** PLAN-TEXTAREA stages 2–4, especially soft wrap.
`Effect::FileRead` / `FileWrite` so open/save is not a subprocess per note.
Cmd+K inline edit is M3 plus selection, not more rail chrome. Graph viz is
not on the exit list. See [[09-app-functions]] and [[10-libra]].

## M4.1 — Markdown display crate

**Started.** `libra-markdown` parses CommonMark (frontmatter skip, block AST,
inline spans). Preview is catalog `<Markdown>` — real `Heading` / `Text` /
`Separator` children, never a webview, never WYSIWYG (§24). Inlines paint as
Text runs: strong → bold, emphasis → italic face, strike → hairline,
code/math → muted source, links and `[[wikilinks]]` → accent (press
dispatches `onWikilink`). GFM tables lay out as rows. The same crate is
the substrate for M5 Tab paint later — do not start a second parser.

**Exit:** opening a real vault note shows headings, paragraphs, lists, fenced
code, bold, italic, strike, tables, math source, and wikilinks as
components. Edit still writes through `<Textarea>`. Soft wrap is PLAN-TEXTAREA
stage 4, not a second markdown crate.

**Libra:** remaining misses are `libra-markdown` parser gaps. Do not start a
second parser for Tab later.

## M5 — Tab

Ghost-text inline completion using a fast frontier model with prompt caching.
Context = current block + retrieved neighbors + open tabs.

**Exit:** acceptance rate above 25% on my own writing.

**Libra:** ghost-run paint on Textarea when M1 can retrieve neighbors.
Streaming tokens if we want them before HTTP returns. §24 does not
block this. The buffer stays a string. Until the paint exists, Tab
means the model and the prompt. See [[11-coherence]].

## Deliberately after v1

Mobile capture · sync · diarization · custom Tab model · hosted subscription ·
teams and shared vaults · Windows/Linux · plugin API.

## Reassessment gate

After M2 + R1, stop and ask whether anyone other than me wants the MCP
server and the stale-docs report. That answer decides whether Gloss is
a product or a very good personal tool. Both are acceptable. Conflating
them is not. The claims themselves stay [[11-coherence]].
