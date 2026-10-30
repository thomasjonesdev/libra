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

The phase blurbs below are the historical build order. **Where this stands (2026-10-27):** Phases 0–2 are shipped. Phase 3's release engine, packaging, theming, AccessKit, query, FormState, NavStack, and gallery are shipped; text entry and the catalog are partial. The ergonomics layer and the naming migration, both scheduled in Phase 4, shipped early. [[PLAN-SDK]] S1–S4 shipped: crates.io 0.1.1 as `libra-ui` / `libra-ui-core` / `libra-ui-cli`. The iteration loop (§18.2) ships in that same 0.1.1. **0.1.2** publishes the rest of the catalog that is unblocked (Textarea, VirtualList, Table, LogView, framework keyboard, SafeArea), native integrations N1–N8, and Phase 4 DX stages 1–5.

**Remaining, in this order.** Each item is an existing plan. Do not pull Phase 4 DX or Phase 5 forward of it.

1. **First app outside the repo.** The SDK is not the blocker. `libra new` / `dev` / `test` / `package` already work against crates.io.
2. **Iteration loop — shipped in 0.1.1** ([[18.2-iteration-loop|§18.2]]). `.lx` and theme TOML reload in place. Rust saves snapshot, restart, and rehydrate field by field. The published CLI is 0.1.1.
3. **Desktop behavior first-party apps already assume. Shipped.** Notifications ([[PLAN-NOTIFY]] NT1–NT4), tray / close-to-tray / single instance / launch-at-login ([[PLAN-RESIDENCY]] R1–R6), `<Stream>` delivery plus `Sub::Stream` and the aws-login device-code screen ([[PLAN-STREAM-DELIVERY]] D5–D6), and the §19.4 chrome gaps ([[PLAN-NATIVE-INTEGRATIONS]] N8: Linux menus are a typed refusal, Wayland clipboard is enabled, frameless drops decorations on Windows and Linux). Windows toast replace is limited by notify-rust 4, which does not read the tag id.
4. **Text and catalog. Shipped**, except Image. Textarea stages 2–4 ([[PLAN-TEXTAREA]]). `<VirtualList>`, `<Table>`, and `<LogView>` window by the scroll offset ([[07.1-builtin-catalog|§7.1]], [[PLAN-CATALOG]] wave 8). The framework on-screen keyboard summons on touch focus (`keyboard="text|numeric|hex|none"`, [[19.1-touch|§19.1]]). Image stays blocked on a deterministic decode path. Tree is still wave 8. System-OSK integration, fling, and pinch stay in §19.1.
5. **Phase 4 DX** ([[PLAN-PHASE4-DX]]). Automation server, inspector window, `.lreplay` action replay, `libra lsp` diagnostics, and `libra docs` shipped. Still open: `input` / `screenshot` / `perf`, effect-result bodies inside `.lreplay`, LSP completion/hover/go-to-definition, and inspector time-travel scrubbing.
6. **Phase 5+ stays later.** Video, the wgpu executor, mobile.

`libra eject` has nothing to eject from. `libra new` already writes a Cargo app.

**Phase 0 — Spike. Shipped.** `libra-core` loop + `libra-markup` parser + interpreter + minimal components (Column/Row/Text/Button) + taffy + tiny-skia + winit window. Counter runs; unit and component tests pass; a snapshot test passes byte-exact. *Go/no-go gate: the determinism claim proven end-to-end.*

**Phase 1 — The testable foundation. Shipped.** Effects runtime + redb KV + Harness with effect interception + snapshot infrastructure with validate/diff + error overlay + `libra check`. `Remote<T,E>` landed. Hot reload of `.lx` preserves state in-process. Rehydration across a Rust restart is the open remainder (item 2 above).

**Phase 2 — The data plane. Shipped** for ring buffers, ingest, tapes, M4, Chart/Sparkline, damage tracking, `Sub::frames`, and the `dashboard` benchmark against [[22-performance-budgets|§22]]. LogView, VirtualList, and Table landed with catalog item 4.

**Phase 3 — Release engine & polish. Partial.** `libra-codegen` + conformance (interp≡codegen), compile-time markup errors, Input + Textarea, AccessKit, theming, `libra package`, `libra-query`, `FormState`, NavStack, and gallery shipped. IME, long-press selection, the framework OSK, and the data catalog (VirtualList, Table, LogView) shipped with item 4. Image and Tree did not.

**Phase 4 — Legibility & documents. Early slice only.** Naming migration and the ergonomics layer (`#[libra::actions]`, `libra::app!`, collection bindings, source elements) shipped ahead of this phase. The semantic automation server ([[16-automation-server|§16]]) and the inspector window ([[18.4-debugging|§18.4]]) shipped. Still open: time-travel scrubbing, `.lreplay`, `libra lsp`, docs site ([[PLAN-PHASE4-DX]]).

**Phase 5+ — Later.** Video ([[09.5-video|§9.5]]) and, if demanded, the wgpu executor ([[09.4-gpu-executor|§9.4]]) and mobile platform glue ([[24-non-goals|§24]]).

**crates.io 0.1.1 is a snapshot publish, not the "v0.1 after Phase 3" product bar above.** 0.x rendered bytes are not a semver contract. Phase 4 is still what makes the product distinctive in public, and it waits until an app is in daily use. Phases assumed nights-and-weekends solo pace.

## Three-OS desktop parity

The target is **macOS + Windows + Linux** with **identical rendering** and **explicit OS-native chrome** — not a Mac-first shell plus leftovers. Governing note: [[19.4-desktop-os-matrix|§19.4]].

**Content (already the contract).** Same `State` → same display list → same PNG on every OS ([[09.2-canonical-executor|§9.2]]). Snapshots are the proof. Do not add OS-specific paint.

**Chrome.** Native menus, clipboard, tray, notifications, frameless titlebars, `OpenUrl`, OS alert/confirm, clipboard share, file open/save, and window title/fullscreen/resize are implemented per OS or return typed `Unsupported`. Linux menubars are a typed refusal (no GTK window). `<SafeArea>` pads by shell insets (zero in the canonical frame). OS appearance follow remains later.

| Work | Owner |
|---|---|
| Linux native menubar | N8 shipped as a typed refusal (no GTK window) |
| Wayland clipboard; frameless on Win+Linux | N8 shipped |
| Notifications, tray, close-to-tray, single instance, launch-at-login | [[PLAN-NOTIFY]], [[PLAN-RESIDENCY]] shipped |
| IME composition | [[PLAN-TEXTAREA]] stage 2 shipped |
| Touch long-press + framework OSK | stage 3 and the touch keyboard shipped. System OSK, fling, and pinch stay [[19.1-touch|§19.1]] |
| `OpenUrl`, OS alert/confirm, clipboard `Share`, file open/save | N1, N2, N4 shipped. `PickFile` / `SaveFile` shipped |
| Window title / fullscreen / resize | N6 shipped |
| Host-native packages (already shipped; cross-compile deferred) | [[20-packaging|§20]] |

Packaging is **not** equally cross-capable: `libra package --target` must match the host. That stays deferred. It does not justify a weaker runtime on Windows or Linux.

CI today proves the *paint* path (and should keep doing so on Linux). It does not prove windowed chrome on any OS ([[15.8-ci-shape|§15.8]]). Three-OS chrome lands with harness coverage plus a native check on each host; do not wait for a windowed CI matrix to write the attach paths.

---

← [[Architecture Home]] · [[Map of Content]]
