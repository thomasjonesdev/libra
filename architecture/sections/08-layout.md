---
title: "8. Layout"
section: "8"
aliases:
  - "§8"
  - "8"
  - "8. Layout"
tags:
  - architecture
  - section/8
type: architecture
---

# 8. Layout

**taffy** (the flexbox/grid engine from the Dioxus/Bevy ecosystem) computes all geometry.

- Exposed model: flexbox subset + grid — `gap`, `padding`, `margin`, `grow`, `shrink`, `basis`, `main`/`cross` alignment, `wrap`, absolute positioning within a Panel, and `Grid` templates. No floats, no CSS cascade, no stylesheet matching: layout attributes live on the node that owns them.
- Layout is incremental: the component tree marks dirty nodes on prop change; taffy recomputes affected subtrees only.
- Text measurement enters layout through a measure callback into the text stack ([[10-text|§10]]), with a content-hash cache in front of it (measuring is the expensive path).
- Determinism note: all layout math is f32 with a fixed rounding policy (round-to-nearest-half-up at physical-pixel boundaries) applied *once*, at paint time — so layout results are identical across platforms and never accumulate drift.

---

---

← [[Architecture Home]] · [[Map of Content]]
