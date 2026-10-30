---
title: "PLAN-PHASE4-DX"
aliases:
  - "PLAN-PHASE4-DX"
tags:
  - plans
  - architecture
type: plan
---

# Phase 4 DX foundations

**Status: stages 1–5 shipped** (each with the slice named below). [[23-roadmap|§23]] Phase 4.

1. **Shipped.** Automation server (§16) — `snapshot`, `query`, `dispatch`, `state`, `schema`, `log` as JSON lines on a mode-`0600` unix socket. `libra dev` sets `LIBRA_AUTOMATION=1` (debug builds only). `#[libra(redact)]` values stay off `state` and `schema.fields`. Screenshot, synthetic input, and perf verbs are still open
2. **Shipped.** Inspector (§18.4) — `libra dev` sets `LIBRA_INSPECTOR=1`. A debug build opens a second window beside the app with the action log and state. Repeated actions collapse (`Ticked × N`). Redacted fields show as `(redacted)` in that window only. Damage rects, the layer cache, and a frame-timing view are still open
3. **Shipped.** `.lreplay` (§15.6) — action steps, `discard` / `preludeDiscard`, blake3 state hash and frame hash. `libra test --replay file.lreplay` sets `LIBRA_REPLAY` and runs tests named `lreplay`. Effect result bodies and tapes are still open
4. **Shipped.** `libra lsp` — stdio server. Diagnostics are markup parse errors and interpreter structural errors (unknown elements, bad attributes, PascalCase handlers), with 1-based spans converted to LSP positions. Completion, hover, and go-to-definition need app reflection and are still open
5. **Shipped.** `libra docs [out]` writes a static site from the gallery catalog (`examples/gallery/src/model.rs`) and copies `workbench_<id>_<theme>.png` snapshots beside each page

Ship incrementally; each stage keeps `cargo test --workspace` green.

---

← [[Plans Index]] · [[23-roadmap|§23]]
