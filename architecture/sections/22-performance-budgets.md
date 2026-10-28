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

## Per-app budgets in the harness

The frame-timing half of the above is available to any app test through the
autopilot ([[PLAN-AUTOPILOT]]): `.budget(Budget::new().p95_ms(150.0).max_ms(1_500.0))`
asserts over a whole run, and a failure names the interaction behind the worst
frame. Because these are wall-clock they are opt-in per test and belong at the
regression-in-kind threshold, not at a tight one — the clip-mask regression put
single frames past a second, which no reasonable machine variance reaches.

**Growth guards are the machine-independent half.** `.bounded("history", 500, …)`
measures app state rather than time, so it is deterministic everywhere and is
the leak check that belongs in a committed test. Process RSS is not: it reports
allocator behavior as much as app behavior. A run that holds every collection
bounded across hundreds of interactions is the property "memory climbs while I
use it" actually reduces to.

## In code

- `examples/dashboard` — `cargo run -p dashboard --release -- bench` (the §22 budget check)
- `crates/libra-test/src/autopilot.rs` — `Budget` + bounded metrics for per-app runs
- Root `Cargo.toml` `[profile.dev.package.*]` — paint-path crates (tiny-skia / fontdue / taffy + `libra-raster` / `libra-draw` / `libra-data` / `libra-components` / `libra-platform`) stay at `opt-level = 2` in plain debug so live windows remain within the damage-tracked budget without a special profile. App logic stays opt-level 0.

---

---

← [[Architecture Home]] · [[Map of Content]]
