---
title: "25. Risks & Mitigations"
section: "25"
aliases:
  - "§25"
  - "25"
  - "25. Risks & Mitigations"
tags:
  - architecture
  - section/25
type: architecture
---

# 25. Risks & Mitigations

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | **Text editing & IME** — the historically fatal subsystem | High | Ride the text stack; do not invent shaping. The *buffer* stays a plain string ([[10.1-text-entry|§10.1]]). Attributed editing and formatting toolbars stay out. Styled *display* (`<Markdown>`, emphasis faces) already ships and is not the death trap. Pure `EditState` tested without rendering; CJK IME still a v0.1 bar. |
| 2 | **CPU rendering ceiling** — a real app outgrows the raster budget | Medium | Damage+layers first (usually sufficient); per-component GPU surfaces second; wgpu executor third — architecture already decoupled, decision deferred to profiling evidence |
| 3 | **Interpreter/codegen drift** | Medium | Structural prevention (one parser crate), golden conformance suite in CI, fuzzed equivalence |
| 4 | **Determinism erosion** (a dependency update changes rasterization) | Medium | Pin tiny-skia/swash exactly; snapshot corpus is the canary; version-bump PRs re-validate deliberately, reviewed as diffs |
| 5 | **Solo-scope overrun** | High | [[24-non-goals|§24]] exists; Phase-0 gate is honest; the catalog is 30 components not 50; DX polish (LSP) staged behind shipping the test ladder |
| 6 | **Ecosystem overlap** (Dioxus/Iced ship a determinism story) | Low-Med | Libra's moat is the *combined* contract ([[15.1-determinism-contract|§15.1]]) + data plane + automation; if an incumbent gets there first, Libra's test/automation layers were designed as separable crates — pivot to riding them |
| 7 | **winit gaps** (IME quirks, platform behaviors) | Medium | winit is the ecosystem's shared battlefield — issues are known and workarounds documented; libra-platform isolates all of it |
| 8 | **Serde rehydration friction** as State evolves in dev | Low | Per-field fallback + defaults + explicit report; worst case is a clean restart, never corruption |
| 9 | **Linux touch/OSK fragmentation** on tablet targets | High | Framework-owned OSK component as the guaranteed path ([[19.1-touch|§19.1]]); system-OSK strictly opt-in; gesture layer rides winit touch events which are mature on Linux; touch test synthesis in the harness from Phase 2 |
| 10 | **Team adoption risk** (theoretical → production) | Medium | Adoption path in [[appendix-d-use-case-fit|Appendix D]]: internal ops tool pilot first, coexistence with existing Tauri surfaces, closed markup grammar keeps non-Rust contributors productive and safe |

---

---

← [[Architecture Home]] · [[Map of Content]]
