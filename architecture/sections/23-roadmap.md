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

**Phase 0 — Spike (weeks 1–2).** `libra-core` loop + `libra-markup` parser + interpreter + minimal components (Column/Row/Text/Button) + taffy + tiny-skia + winit window. Counter runs; unit and component tests pass; a snapshot test passes byte-exact in a container. *Go/no-go gate: the determinism claim proven end-to-end.*

**Phase 1 — The testable foundation (months 1–2).** Effects runtime + redb KV + Harness with effect interception (integration tests) + snapshot infrastructure with validate/diff + hot reload with state rehydration + error overlay + `libra check`. `Remote<T,E>` + error boundaries land here. The counter-that-fetches-and-persists example, fully tested at every kind.

**Phase 2 — The data plane (months 2–4).** Ring buffers, ingest, tapes, M4, Chart/Sparkline/LogView/VirtualList/Table, damage tracking + layer cache hardened, `Sub::frames`, the `dashboard` benchmark app hitting [[22-performance-budgets|§22]] budgets.

**Phase 3 — Release engine & polish (months 4–6).** `libra-codegen` + conformance suite (interp≡codegen), compile-time markup errors, the full [[10.1-text-entry|§10.1]] text-entry spec to v0.1 acceptance (Input + Textarea, IME, touch selection, undo, validators, large-paste), remaining catalog components, accesskit integration, theming, `libra package`, `libra-query`, `FormState`, screen-stack helper, `libra gallery`.

**Phase 4 — Legibility & documents (months 6–8).** Automation server + skills emitter, time-travel inspector, `.lreplay` replay tests, the [[03.2-terminology-migration|§3.2]] naming migration (aliases → hard rename, snapshot-neutral), the ergonomics layer (`#[libra::actions]` [[05.2-macro-expansion|§5.2]], `libra::app!` [[05.8-app-assembly|§5.8]], collection bindings [[06.2-closed-grammar|§6.2]], source elements [[12.5-declarative-subscriptions|§12.5]] — staged plan in `PLAN-ERGONOMICS.md`), `libra lsp`, docs site generated from the component catalog's own fixtures.

**Phase 5+ — Video** ([[09.5-video|§9.5]]) and, if demanded, the wgpu executor ([[09.4-gpu-executor|§9.4]]) and mobile platform glue ([[24-non-goals|§24]]).

**v0.1 ships after Phase 3; Phase 4 is what makes it distinctive in public.** Phases assume nights-and-weekends solo pace; compress accordingly if staffed.

---

---

← [[Architecture Home]] · [[Map of Content]]
