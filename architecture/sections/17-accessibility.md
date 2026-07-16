---
title: "17. Accessibility"
section: "17"
aliases:
  - "§17"
  - "17"
  - "17. Accessibility"
tags:
  - architecture
  - section/17
type: architecture
---

# 17. Accessibility

Accessibility is load-bearing, not charitable: the semantics tree is simultaneously the a11y surface and the test-query index, so it cannot rot.

- Every component emits `Semantics { role, label, value, state, actions }`; composed views inherit/override via markup attributes (`label=`, `testId=` lives here too).
- The tree feeds **accesskit**, which bridges to VoiceOver / Narrator / AT-SPI — real screen-reader support on all three platforms from one tree.
- Keyboard: full tab-order/focus-ring system in the component layer; CommandPalette and hotkey subs make ops tools keyboard-first.
- `libra check --a11y` audits statically (labels on interactive components, contrast against theme tokens, focus reachability), and the audit runs in CI.


---

---

← [[Architecture Home]] · [[Map of Content]]
