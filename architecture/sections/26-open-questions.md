---
title: "26. Open Questions"
section: "26"
aliases:
  - "§26"
  - "26"
  - "26. Open Questions"
tags:
  - architecture
  - section/26
type: architecture
---

# 26. Open Questions

1. **`.lx` expression whitelist breadth** — how many pure accessors before the grammar stops being "closed"? Start minimal (`len`, `isEmpty`, format specs, generated enum predicates); every addition needs a use case the State-field pattern genuinely can't serve.
2. **~~Msg payload ergonomics~~ — resolved by the action layer ([[05-state-and-actions|§5]]).** Payloads are method parameters; the struct-vs-tuple-variant question dissolves into signature style, and markup dispatch follows the reflected signature.
3. **Multi-window model — resolved: one State, per-window view roots.** Ground-control consoles are multi-monitor; a single source of truth across a window wall is the point. Each window declares a root view in `libra.toml`; all windows dispatch into one action queue; damage/paint are per-window; snapshot tests address windows individually. Per-window state fragmentation is rejected for the same reason Context was.
4. **`Frame` cadence off-vsync** — headless/test mode uses injected clocks; should `libra dev` offer a fixed-step mode for reproducing timing bugs live?
5. **Tape format** — bespoke `.ltape` vs. embedding something existing (Arrow IPC?) for interop with the analysis ecosystem.
6. **Markup-declared queries** — should `<Query key={...} into={userLoaded}>` join the [[12.5-declarative-subscriptions-effects-as-mount-scoped-data|§12.5]] source elements? Same mechanism (declaration diffed on mount), real precedent (Relay), but it moves fetch policy into views and needs dedup/waterfall semantics designed jointly with `libra-query`. Deferred past v0.1; revisit with a real app's fetch graph in hand.
7. **Name — resolved: Libra.** Before first public publish, verify crates.io availability for `libra`/`libra-*` (a `libra` crate may exist from the 2019 Facebook Libra era — if squatted, publish under `libra-ui`/`libraui` and keep the brand) and check domain options (libra.dev, libra-ui.dev).

---

---

← [[Architecture Home]] · [[Map of Content]]
