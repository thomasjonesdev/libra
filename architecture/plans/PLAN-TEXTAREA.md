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

**Status: in progress — stage 1 landed 2026-07-17.** Completes [[10.1-text-entry|§10.1]] beyond single-line Input.

## Stages

1. ~~`Textarea` component (multi-line EditState, same pure core)~~ **done**: `EditOp::Up`/`Down` (column-preserving line nav; `Home`/`End` became line-scoped — identical for single-line values) in libra-text; `<Textarea>` in both engines (`list_textarea.lx` fixture); line-aware paint (per-line selection bands, line-positioned caret); shell Enter-inserts-newline / Up/Down / newline-preserving paste when the focused control is a Textarea; gallery + notes (`editor-body`) adopted. Hard newlines only — soft wrap + sticky column are stage-2 concerns.
2. IME composition events through shell → EditOp
3. Touch selection / long-press
4. Soft wrap + sticky-column Up/Down; large-paste rope backing

---

← [[Plans Index]] · [[10.1-text-entry|§10.1]]
