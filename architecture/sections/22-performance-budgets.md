---
title: "22. Performance Budgets"
section: "22"
aliases:
  - "§22"
  - "22"
  - "22. Performance Budgets"
tags:
  - architecture
  - section/22
type: architecture
---

# 22. Performance Budgets

Budgets are CI-enforced (criterion benches + frame-timing assertions in snapshot-harness runs on a pinned reference machine class):

| Metric | Budget |
|---|---|
| `.lx` reparse + revalidate (dev) | < 5 ms per file |
| Hot-reload edit → visible | < 500 ms |
| App-crate incremental rebuild (logic edit) | < 5 s |
| `reduce` dispatch overhead (runtime, excl. app logic) | < 5 µs |
| Reconcile + layout, 1k-component tree, small delta | < 1 ms |
| Full repaint 1440p (cold) | < 8 ms |
| Damage-tracked dashboard frame, 12 live charts @60fps | < 2 ms |
| M4 downsample, 1M points → 1200 px | < 3 ms (cached: ~0) |
| Unit+component+integration suite, 500 tests | < 2 s |
| Snapshot test, 1280×800 | < 50 ms each |
| Cold start to first frame | < 150 ms |
| Release binary (counter) | < 8 MB |

Two standing benchmark apps: `dashboard` (data-plane stress: 16 series × 10 kHz tape) and `log-explorer` (1M-line corpus, live filter).

---

---

← [[Architecture Home]] · [[Map of Content]]
