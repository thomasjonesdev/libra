---
title: "15. Testing Architecture"
section: "15"
aliases:
  - "§15"
  - "15"
  - "15. Testing Architecture"
tags:
  - architecture
  - section/15
type: architecture
---

# 15. Testing Architecture

## In this vault

- [[15.1-determinism-contract]]
- [[15.2-unit-tests]]
- [[15.3-component-tests]]
- [[15.4-integration-tests]]
- [[15.5-snapshot-tests]]
- [[15.6-replay-tests]]
- [[15.7-fuzzing-property-tests]]
- [[15.8-ci-shape]]

## In code

- `crates/libra-test/src/lib.rs` — the in-process `Harness<A: App>`
- `examples/*/tests/` — unit / components / integration / snapshots per app

---

The reason the framework exists. Five kinds of test — named for what a frontend developer already calls them, not for a numbering scheme — all headless, all runnable in a bare CI container, all deterministic. Target: an app's full suite in **seconds**.

| Kind | Exercises | Lives in | JS-world analog | What's different here |
|---|---|---|---|---|
| **Unit** | Actions: pure state transitions + returned effects | `#[cfg(test)]` mod next to the slice | Jest reducer tests | Actions are plain methods — no store setup, no dispatch boilerplate |
| **Component** | The real view tree: queries, clicks, typing | `tests/components.rs` | Testing Library | Real tree, real hit testing — no jsdom, no `act()` |
| **Integration** | Full flows across effects | `tests/integration.rs` | MSW / saga tests | No mocks at all: effects intercepted as data, results hand-fed, fully synchronous |
| **Snapshot** | Rendered pixels | `tests/snapshots.rs` | Chromatic / jest-image-snapshot | Byte-exact PNG equality, zero tolerance thresholds |
| **Replay** | Recorded sessions end-to-end | `tests/replays/` | Playwright traces | Exact re-execution: final state hash + final frame bytes |

Test names are plain descriptions (`fn connect_requests_session()`); the kind lives in the file, never in a prefix.

---

← [[Architecture Home]] · [[Map of Content]]
