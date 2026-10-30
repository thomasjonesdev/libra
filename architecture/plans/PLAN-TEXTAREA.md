---
title: "PLAN-TEXTAREA"
aliases:
  - "PLAN-TEXTAREA"
tags:
  - plans
  - architecture
type: plan
---

# Textarea / IME / touch (§10.1 remainder)

**Status: stages 1–4 shipped.** Completes [[10.1-text-entry|§10.1]] beyond single-line Input. The framework on-screen keyboard ([[19.1-touch|§19.1]]) shipped with the catalog: touch focus summons `text` / `numeric` / `hex`; `none` suppresses it.

## Stages

1. ~~`Textarea` component (multi-line EditState, same pure core)~~ **done**: `EditOp::Up`/`Down` (column-preserving line nav; `Home`/`End` became line-scoped — identical for single-line values) in libra-text; `<Textarea>` in both engines (`list_textarea.lx` fixture); line-aware paint (per-line selection bands, line-positioned caret); shell Enter-inserts-newline / Up/Down / newline-preserving paste when the focused control is a Textarea; gallery + notes (`editor-body`) adopted. Hard newlines only — soft wrap + sticky column are stage-2 concerns.
2. **Shipped.** `EditOp::Preedit` / `Commit`. The composition stays out of the buffer until commit, paints with an accent underline, and the shell positions the candidate window with `set_ime_cursor_area`. A following edit commits first.
3. **Shipped.** A 500ms touch dwell selects the word and paints selection grips. The harness verb is `long_press`. winit does not deliver touch events on macOS; Linux and Windows do.
4. **Shipped.** Soft wrap was already the paint path. Arrow Up/Down in a textarea follow visual rows and keep a pixel column. `EditOp::Up`/`Down` stay hard-line (the core has no font metrics) and remember a character column. Pastes at 64KiB and above keep a `ropey` rope so undo clones stay cheap.

---

← [[Plans Index]] · [[10.1-text-entry|§10.1]]
