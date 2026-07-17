---
title: "AGENTS"
aliases:
  - "Agent Brief"
  - "AGENTS.md"
  - "End-to-end"
tags:
  - architecture
  - agents
  - moc
type: brief
---

# Libra — end-to-end brief (agents & humans)

**Read this first.** It is the short path to understanding the whole system. Deep detail lives in linked vault notes; this page is the map, not the territory.

| If you need… | Go here next |
|---|---|
| Full outline | [[Architecture Home]] · [[Map of Content]] |
| Resolve `§N.N` → file | [[SECTION-INDEX]] |
| Plans / staging history | [[Plans Index]] |
| Repo commands & crate lore | repo-root `CLAUDE.md` |

---

## 1. What Libra is

A **native desktop UI framework in Rust** for operational, data-dense tools (dashboards, control surfaces, ops consoles). The product is not “another React-in-Rust” — it is a stack where **logic, views, effects, and rendered pixels are deterministic and machine-verifiable**.

One-sentence pitch → [[01.1-the-one-sentence-pitch|§1.1]]

---

## 2. The four bets (everything else follows)

1. **One `State`, closed `Action` set** — changes go through pure `reduce(state, action) → Vec<Effect>`.
2. **Views are data** — `.lx` markup has a *closed grammar* (bindings + dispatch, no logic/side effects).
3. **Effects are data** — actions return `Effect` descriptions; only the runtime performs I/O; tests intercept, never mock.
4. **Rendering is deterministic** — CPU rasterizer; same display list → same bytes on every machine; screenshots are byte-exact.

→ [[01.3-the-bet|§1.3]] · checklist [[appendix-a-the-determinism-contract-as-a-checklist|Appendix A]]

---

## 3. Five words (authoring vocabulary)

| Word | Meaning |
|---|---|
| **State** | Single app-state struct — source of truth |
| **Action** | One recorded change (authored as `&mut self` methods; recorded as a serializable enum) |
| **Effect** | Description of I/O returned by an action |
| **Component** | Anything that renders (catalog, `.lx` composition, or Rust) |
| **View** | A `.lx` file — declarative projection of State |

React→Libra translation → [[03-mental-model-react-to-libra|§3]] · naming → [[03.3-naming-conventions-reacts-verbatim|§3.3]]

Typical app shape:

```rust
#[libra::state]
struct State { /* fields */ }

#[libra::actions]
impl State {
    fn increment(&mut self) { /* … */ }           // → Action::Increment
    fn loaded(&mut self, r: HttpResult) { /* … */ } // effect continuation
}

libra::app! {
    state: State,
    view: "src/view.lx",
    // …
}
```

→ [[05-state-and-actions|§5]] · [[05.8-app-assembly-one-declaration|§5.8]]

---

## 4. Pipeline (one-way data flow)

Every stage is a pure function of the previous:

```
.lx  →  markup (AST + spans)
     →  interp (dev) / codegen (release)  →  ViewTree
     →  components + taffy layout         →  LaidTree (+ semantics + handlers)
     →  paint                             →  Vec<DrawCmd>
     →  libra-raster (tiny-skia)          →  deterministic RGBA / PNG
```

Layer diagram → [[04.1-layer-diagram|§4.1]] · frame lifecycle → [[04.2-the-frame-lifecycle|§4.2]]

**Runtime loop (main thread):** action queue → `reduce` → Effects → rebuild view → layout → paint.

| Side path | Role |
|---|---|
| **Effects runtime** | Workers run Http/Kv/Timer/Random/…; results re-enter as Actions ([[12-the-effects-runtime|§12]]) |
| **Data plane** | High-rate rings/tapes/M4; State holds handles only ([[14-the-high-rate-data-plane|§14]]) |
| **Platform shell** | winit/softbuffer presentation only — **not** on the determinism paint path ([[21-workspace-and-crate-layout|§21]]) |
| **Harness** | Same pipeline headless for tests ([[15-testing-architecture|§15]]) |

**Two engines, one grammar:** `libra-interp` (dev, hot reload) and `libra-codegen` (release). Both depend on `libra-markup` only. Grammar changes must update **both** + a `conformance/` fixture — `interp_and_codegen_produce_identical_trees` is the drift alarm ([[06.4-one-grammar-two-engines|§6.4]]).

---

## 5. Closed sets (what you may not invent casually)

| Closed set | Rule of thumb |
|---|---|
| **Markup grammar** | No expressions that compute, mutate, or perform I/O. Need compute? Put it on State. ([[06.2-the-closed-grammar-what-markup-can-do|§6.2]], [[06.3-what-markup-cannot-do-and-why-each-restriction-pays|§6.3]]) |
| **Effect vocabulary** | Versioned enum; apps don't invent ad-hoc I/O calls. ([[12.1-the-effect-vocabulary-closed-versioned|§12.1]]) |
| **Component catalog** | Curated built-ins; custom Rust/Canvas must still emit semantics and stay deterministic. ([[07-components|§7]]) |
| **Purity of actions** | No I/O, clocks, randomness, threads, or blocking inside reduce — time/entropy arrive *inside* action payloads. ([[05.3-purity-rules-for-actions-enforced-by-convention-checked-by-lint|§5.3]]) |

Mount-scoped ambient work uses **source elements** (`<Frames/>`, `<Interval every={ms} onTick={…}/>`, `<Stream id={…} onEvent={…}/>`) or `subscriptions:` in `app!` — declarations, not `useEffect` code ([[12.5-declarative-subscriptions-effects-as-mount-scoped-data|§12.5]]; [[PLAN-SOURCES]]). Interval fires Instant actions via shell recurring timers; Stream mounts/queries in v1 (Ws event delivery later). **`<if test={bool}>`** ([[PLAN-IF]]) mounts sources conditionally — see `examples/dashboard`.

---

## 6. Crate map (where to edit)

| Crate | Job |
|---|---|
| `libra-core` | State/Action/Effect/`Remote`/`App` contract |
| `libra-reflect` | `#[libra::state]` / `#[libra::actions]` / `app!` |
| `libra-markup` | `.lx` parser (shared grammar) |
| `libra-interp` | Dev view engine |
| `libra-codegen` | Release view engine |
| `libra-components` | Layout + semantics + handlers |
| `libra-draw` | `DrawCmd` + theme tokens |
| `libra-raster` | Deterministic CPU raster |
| `libra-effects` | Only I/O actor |
| `libra-data` | High-rate data plane |
| `libra-kv` | redb behind `Effect::Kv*` |
| `libra-text` | Pure text-edit state machine |
| `libra-test` | In-process `Harness` |
| `libra-platform` | Windowed shell (presentation) |
| `libra` | Facade re-exports for macros |

Full layout → [[21-workspace-and-crate-layout|§21]]

---

## 7. Testing ladder (five kinds)

| Kind | Asserts |
|---|---|
| **Unit** | `reduce` / actions — pure, no runtime |
| **Component** | Semantics tree queries (`test_id` / text / role) via Harness |
| **Integration** | Effect descriptions + hand-fed results (no mocks) |
| **Snapshot** | Byte-exact PNG |
| **Replay** | Recorded action sessions (planned / partial) |

Determinism contract that makes them exact → [[15.1-the-determinism-contract-what-makes-every-kind-exact|§15.1]]

Example apps split tests as `tests/unit.rs`, `components.rs`, `integration.rs`, `snapshots.rs` — one file per kind.

---

## 8. What is built vs remaining

**Done (through Phase 3 core + ergonomics + item fields + `<if>` + sources + accessors):** Phases 0–2; codegen + conformance; text-entry core (`libra-text` + Input + keyboard/clipboard); theming; AccessKit; `libra-query`; FormState/`Field`; NavStack; gallery; `#[libra::state]`/`#[libra::actions]`/`app!`; `<for>` collections; loop-scoped item fields; `<if test={bool}>` / `<else>` ([[PLAN-IF]]); `<Frames>` / `<Interval>` / `<Stream>` sources ([[PLAN-SOURCES]]); top-level `{user.name}` + `len`/`isEmpty` accessors ([[PLAN-ACCESSORS]]). Naming migration complete.

**Still open / do not build ahead:** expanded catalog (Scroll next; token ramps T2+; platform effects) — [[PLAN-UI-SURFACE]], demand-ordered by the notes reference app ([[PLAN-NOTES-APP]]); Textarea IME/soft-wrap/touch ([[PLAN-TEXTAREA]] stages 2–4); packaging; automation server polish; full Phase 4 DX (LSP, time-travel UI). Tracked in [[23-roadmap|§23]] and [[26-open-questions|§26]]. *Shipped along the way (2026-07-16/17): PLAN-TOKENS T1, Spacer/Separator/`width`/weighted `grow`, ListItem (+hover-as-paint-input), Textarea stage 1; snapshot verb renamed bless→validate (`LIBRA_VALIDATE=1`).*

**Non-goals** (scope fence) → [[24-non-goals|§24]]

---

## 9. How to change the system safely

1. **Find the governing §** via [[SECTION-INDEX]] — read that note before designing.
2. **Respect Appendix A** — if a change breaks determinism, purity, closed grammar, or interceptable effects, it is wrong unless the vault is deliberately updated in the same change.
3. **Cite § in doc comments** the way existing crates do (`ARCHITECTURE.md §N.N`).
4. **Grammar change** → update interp *and* codegen *and* add/adjust a `conformance/` fixture; keep `interp_and_codegen_produce_identical_trees` green.
5. **Visual change** → expect snapshot diffs; validate deliberately (`LIBRA_VALIDATE=1`); update README hashes with PNGs.
6. **Edit the vault note**, not a revived root mega-doc. New subsections need: note + parent links + `SECTION-INDEX` row + Home/MOC outline entries.
7. **Don't invent ahead of §23/§26**; keep shapes compatible with later phases.

### When this change lands (light checklist)

Do this in the **same change** as the code — only the notes that actually moved, not a full vault audit:

1. Governing § note matches the new behavior/contract (or you explicitly decided code follows an existing note).
2. If “built vs remaining” / roadmap status shifted, update **this page** (§8) and/or [[23-roadmap|§23]] / [[26-open-questions|§26]].
3. New subsection? Wire Home + MOC + `SECTION-INDEX` (step 6 above).
4. Run the relevant gates: `cargo test` (scoped or workspace), conformance if grammar, snapshots if pixels.

Skip rewriting unrelated § notes. Stale *adjacent* prose is fine; stale *governing* prose is not.

Repo operational detail (commands, gotchas) stays in `CLAUDE.md`; this brief is the architectural on-ramp.

---

## 10. Suggested reading order (first hour)

1. This page (done)
2. [[01.3-the-bet|§1.3 The bet]] + [[appendix-a-the-determinism-contract-as-a-checklist|Appendix A]]
3. [[04.1-layer-diagram|§4.1 Layer diagram]]
4. [[05-state-and-actions|§5]] (skim [[05.2-what-the-macro-generates-one-expansion-no-magic|§5.2]], [[05.8-app-assembly-one-declaration|§5.8]])
5. [[06-the-view-layer-and-markup|§6]] (especially [[06.4-one-grammar-two-engines|§6.4]])
6. [[12-the-effects-runtime|§12]] + [[12.5-declarative-subscriptions-effects-as-mount-scoped-data|§12.5]]
7. [[15-testing-architecture|§15]]
8. Then dive by task (rendering [[09-rendering|§9]], data plane [[14-the-high-rate-data-plane|§14]], text [[10.1-text-entry-the-full-spec-in-scope-committed|§10.1]], …)

---

← [[Architecture Home]] · [[Map of Content]] · [[Plans Index]]
