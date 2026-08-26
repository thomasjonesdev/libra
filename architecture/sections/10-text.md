---
title: "10. Text"
section: "10"
aliases:
  - "§10"
  - "10"
  - "10. Text"
tags:
  - architecture
  - section/10
type: architecture
---

# 10. Text

## In this vault

- [[10.1-text-entry]]

## In code

- `crates/libra-text/` — the pure editing core

---

Text is where custom UI stacks historically die; the strategy is explicit and staged.

- **Shaping & layout**: `parley` (with `swash` underneath) — the Linebender text stack. Real shaping, font fallback chains, bidi, wrapping, and rich spans. This is a deliberate upgrade over Native SDK's single-font/no-kerning subset: Libra targets ops tools that display arbitrary user/log data, so "one Latin font" is not an acceptable ceiling.
- **Rasterization**: `swash` renders glyphs into the atlas; subpixel positioning quantized to a fixed grid (¼-pixel) for determinism.
- **Determinism policy — bundled fonts only.** The app ships its fonts (default set: one UI face, one mono face; overridable in `libra.toml`). System font fallback is *off by default* because it destroys cross-machine reproducibility; a `system-fonts` opt-in exists for apps that prefer nativeness over byte-exact snapshots (their snapshot tests then pin a font manifest).
- **Editing**: fully specced in [[10.1-text-entry|§10.1]]. The buffer is
  a string and the core is a pure state machine. That fence is about the
  model, not the paint. Styled display (markdown spans, italic face)
  already ships. IME and wrap are still the highest-risk *remaining*
  work ([[25-risks|§25]]). Do not reopen an attributed buffer to get
  italic on screen. We already have italic on screen.
- **Monospace fast path**: LogView and Table cells detect pure-ASCII mono runs and skip shaping — the high-volume text path in the target domain is cheap by construction.

---

← [[Architecture Home]] · [[Map of Content]]
