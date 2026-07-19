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

1. **`.lx` expression whitelist breadth** — how many pure accessors before the grammar stops being "closed"? Start minimal (`len`, `isEmpty`, format specs, generated enum predicates); every addition needs a use case the State-field pattern genuinely can't serve. **Loop-scoped item fields** (`{t.text}`, `key={t.id}`, `onPress={complete(t.id)}`) landed via [[PLAN-ITEM-FIELDS]]; **top-level `{user.name}` + `len`/`isEmpty`** landed via [[PLAN-ACCESSORS]]. Remaining: format specs, enum `is*` predicates.
2. **~~Msg payload ergonomics~~ — resolved by the action layer ([[05-state-and-actions|§5]]).** Payloads are method parameters; the struct-vs-tuple-variant question dissolves into signature style, and markup dispatch follows the reflected signature.
3. **Multi-window model — resolved: one State, per-window view roots.** Ground-control consoles are multi-monitor; a single source of truth across a window wall is the point. Each window declares a root view in `libra.toml`; all windows dispatch into one action queue; damage/paint are per-window; snapshot tests address windows individually. Per-window state fragmentation is rejected for the same reason Context was.
4. **`Frame` cadence off-vsync** — headless/test mode uses injected clocks; should `libra dev` offer a fixed-step mode for reproducing timing bugs live?
5. **Tape format** — bespoke `.ltape` vs. embedding something existing (Arrow IPC?) for interop with the analysis ecosystem.
6. **Markup-declared queries** — should `<Query key={...} into={userLoaded}>` join the [[12.5-declarative-subscriptions|§12.5]] source elements? Same mechanism (declaration diffed on mount), real precedent (Relay), but it moves fetch policy into views and needs dedup/waterfall semantics designed jointly with `libra-query`. Deferred past v0.1; revisit with a real app's fetch graph in hand.
7. **Name — resolved: Libra.** Before first public publish, verify crates.io availability for `libra`/`libra-*` (a `libra` crate may exist from the 2019 Facebook Libra era — if squatted, publish under `libra-ui`/`libraui` and keep the brand) and check domain options (libra.dev, libra-ui.dev).
8. **Typed-value binding for closed-set attrs.** *Numeric binding landed
   2026-07-18* — `<Progress value={ratio}>` binds an `f32` State field: interp
   parses the field's Display (f32 round-trips, so both engines agree) and
   release codegen accesses it **typed** (`state.ratio`), so a non-numeric field
   is a rustc error at the `.lx` line — validity stays at compile time
   ([[06.5-compile-time-validation|§6.5]]). This is the sound pattern; the same
   mechanism extends to other numeric attrs (`height`/`width`) when needed.
   **Still open — enum attrs** (`variant`/`size`/`tone`): the tempting shortcut
   (bind a `String`) is **rejected** (runtime validity). The sound form binds a
   **typed** field whose Rust type *is* the closed set (`State { tone: Tone }` →
   `tone={tone}`); codegen typechecks it, but interp needs the enum to `Display`
   to the canonical markup strings (mapping Display→enum as the literal path
   already does) and the app must hold a `libra_components::Tone`-typed field —
   a coupling to weigh. **Motivate on real app need, not tooling** — the
   workbench ([[PLAN-GALLERY]]) shows every enum value statically, no binding.
   Same "bind a closed value" shape as [[PLAN-TOKENS]] T5; decide together.

---

---

← [[Architecture Home]] · [[Map of Content]]
