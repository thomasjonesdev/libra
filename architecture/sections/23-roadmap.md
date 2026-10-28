---
title: "23. Roadmap"
section: "23"
aliases:
  - "§23"
  - "23"
  - "23. Roadmap"
tags:
  - architecture
  - section/23
type: architecture
---

# 23. Roadmap

The phase blurbs below are the historical build order. **Where this stands (2026-10-27):** Phases 0–2 are shipped. Phase 3's release engine, packaging, theming, AccessKit, query, FormState, NavStack, and gallery are shipped; text entry and the catalog are partial. The ergonomics layer and the naming migration, both scheduled in Phase 4, shipped early. [[PLAN-SDK]] S1–S4 shipped: crates.io 0.1.1 as `libra-ui` / `libra-ui-core` / `libra-ui-cli`. The iteration loop (§18.2) ships in that same 0.1.1.

**Remaining, in this order.** Each item is an existing plan. Do not pull Phase 4 DX or Phase 5 forward of it.

1. **First app outside the repo.** The SDK is not the blocker. `libra new` / `dev` / `test` / `package` already work against crates.io.
2. **Iteration loop — shipped in 0.1.1** ([[18.2-iteration-loop|§18.2]]). `.lx` and theme TOML reload in place. Rust saves snapshot, restart, and rehydrate field by field. The published CLI is 0.1.1.
3. **Desktop behavior first-party apps already assume.** Notifications ([[PLAN-NOTIFY]]), tray / close-to-tray / single instance / launch-at-login ([[PLAN-RESIDENCY]]), `<Stream>` event delivery ([[PLAN-STREAM-DELIVERY]]), and three-OS chrome with typed `Unsupported` instead of a silent no-op ([[19.4-desktop-os-matrix|§19.4]], [[PLAN-NATIVE-INTEGRATIONS]] N8).
4. **Text and catalog still open.** Textarea IME / soft-wrap / touch ([[PLAN-TEXTAREA]] stages 2–4). Image is blocked on decode. Table, VirtualList, and LogView remain catalog targets ([[07.1-builtin-catalog|§7.1]]).
5. **Phase 4 DX, after an app is in daily use** ([[PLAN-PHASE4-DX]]). Inspector, `libra lsp`, automation server, `.lreplay`, docs site from gallery fixtures.
6. **Phase 5+ stays later.** Video, the wgpu executor, mobile.

`libra eject` has nothing to eject from. `libra new` already writes a Cargo app.

**Phase 0 — Spike. Shipped.** `libra-core` loop + `libra-markup` parser + interpreter + minimal components (Column/Row/Text/Button) + taffy + tiny-skia + winit window. Counter runs; unit and component tests pass; a snapshot test passes byte-exact. *Go/no-go gate: the determinism claim proven end-to-end.*

**Phase 1 — The testable foundation. Shipped.** Effects runtime + redb KV + Harness with effect interception + snapshot infrastructure with validate/diff + error overlay + `libra check`. `Remote<T,E>` landed. Hot reload of `.lx` preserves state in-process. Rehydration across a Rust restart is the open remainder (item 2 above).

**Phase 2 — The data plane. Shipped** for ring buffers, ingest, tapes, M4, Chart/Sparkline, damage tracking, `Sub::frames`, and the `dashboard` benchmark against [[22-performance-budgets|§22]]. LogView, VirtualList, and Table stayed catalog work (item 4).

**Phase 3 — Release engine & polish. Partial.** `libra-codegen` + conformance (interp≡codegen), compile-time markup errors, Input + Textarea stage 1, AccessKit, theming, `libra package`, `libra-query`, `FormState`, NavStack, and gallery shipped. The full [[10.1-text-entry|§10.1]] acceptance (IME, touch selection, the rest of the catalog) did not.

**Phase 4 — Legibility & documents. Early slice only.** Naming migration and the ergonomics layer (`#[libra::actions]`, `libra::app!`, collection bindings, source elements) shipped ahead of this phase. Still open, and not next: automation server, time-travel inspector, `.lreplay`, `libra lsp`, docs site ([[PLAN-PHASE4-DX]]).

**Phase 5+ — Later.** Video ([[09.5-video|§9.5]]) and, if demanded, the wgpu executor ([[09.4-gpu-executor|§9.4]]) and mobile platform glue ([[24-non-goals|§24]]).

**crates.io 0.1.1 is a snapshot publish, not the "v0.1 after Phase 3" product bar above.** 0.x rendered bytes are not a semver contract. Phase 4 is still what makes the product distinctive in public, and it waits until an app is in daily use. Phases assumed nights-and-weekends solo pace.

## Three-OS desktop parity

The target is **macOS + Windows + Linux** with **identical rendering** and **explicit OS-native chrome** — not a Mac-first shell plus leftovers. Governing note: [[19.4-desktop-os-matrix|§19.4]].

**Content (already the contract).** Same `State` → same display list → same PNG on every OS ([[09.2-canonical-executor|§9.2]]). Snapshots are the proof. Do not add OS-specific paint.

**Chrome (the remaining work).** Native menus, dialogs, clipboard, tray, notifications, frameless titlebars, IME, and the rest of the §19 table are implemented with each OS's API, or they return typed `Unsupported`. Silent no-ops (Linux muda attach, Wayland clipboard drop, frameless ignored off macOS) are gaps to close, not accepted v1 shape.

| Work | Owner |
|---|---|
| Linux native menubar + context popup | [[PLAN-NATIVE-INTEGRATIONS]] N8 |
| Wayland clipboard; frameless CSD / typed refusal on Win+Linux | N8 |
| IME / CJK | [[PLAN-TEXTAREA]] stage 2 |
| Touch + OSK | [[19.1-touch|§19.1]], Textarea stage 3 |
| Notifications (honest `Unsupported` in dev) | [[PLAN-NOTIFY]] |
| Tray, close-to-tray, single instance, launch-at-login | [[PLAN-RESIDENCY]] |
| File/save/alert, `OpenUrl`, `Share` | N1–N4 |
| Host-native packages (already shipped; cross-compile deferred) | [[20-packaging|§20]] |

Packaging is **not** equally cross-capable: `libra package --target` must match the host. That stays deferred. It does not justify a weaker runtime on Windows or Linux.

CI today proves the *paint* path (and should keep doing so on Linux). It does not prove windowed chrome on any OS ([[15.8-ci-shape|§15.8]]). Three-OS chrome lands with harness coverage plus a native check on each host; do not wait for a windowed CI matrix to write the attach paths.

---

← [[Architecture Home]] · [[Map of Content]]
