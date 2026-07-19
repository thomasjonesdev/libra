---
title: "PLAN-COMPONENTS"
aliases:
  - "PLAN-COMPONENTS"
  - "component crate structure"
  - "component authoring experience"
tags:
  - plans
  - architecture
type: plan
---

# Component crate structure & authoring experience

**Status: in progress — C0–C3 landed 2026-07-17.** Splits the single 1040-line
`crates/libra-components/src/lib.rs` into a per-component module layout and
defines the **authoring surface** — the clean, obvious place a designer or
component author goes to change how the UI looks. Umbrella:
[[PLAN-UI-SURFACE]].

**Landed:** `lib.rs` (1040 lines) → `node.rs` (vocabulary) + `layout.rs` /
`paint.rs` (thin dispatchers) + `catalog/` (one module per component, each
owning `build` + `paint` via a `BuildCx` / `PaintCx`). Public surface
unchanged (re-exports); all 270 tests green with **byte-identical** snapshots
(no re-record). `catalog/mod.rs` carries the authoring guide, `catalog/text.rs`
is the copy-me template, and `tests/tokens_only.rs` enforces tokens-only paint
(C3 = [[PLAN-TOKENS]] T3 enforcement). **Remaining:** C4 (optional `Component`
trait) only if boilerplate warrants.

Sibling plans, kept distinct:

| Plan | Owns |
|---|---|
| [[PLAN-CATALOG]] | *What* components exist + the beauty-first build order |
| [[PLAN-TOKENS]] | *Token data* — named ramps, TOML tables, density, hot reload |
| **PLAN-COMPONENTS** (this) | *Where a component lives* + *how you author/tweak one* |

## Why now

The catalog is about to grow from ~9 shipped components to the ~50 in
[[PLAN-CATALOG]] across 10 waves. Today every component is spread across
**two big central `match`es** in one file:

- `build()` — one arm per `ViewNode` variant (measure → taffy leaf → semantics).
- `paint_full()` — one arm per `ItemKind` variant (emit `DrawCmd`s), plus
  free helpers `paint_input` / `paint_textarea` / `paint_chart`.

So "everything about the Button" is in two places 350 lines apart, and adding
a component means editing the middle of a growing file. That does not scale to
the catalog, and it is the opposite of *approachable*. There is no single file
a designer can open to see (or safely change) one component.

**This is a structure + DX plan, not a behavior change.** The closed grammar
([[06.2-closed-grammar|§6.2]]), the two engines ([[06.4-one-grammar-two-engines|§6.4]]),
and every committed snapshot stay **byte-identical** through the mechanical
stages — `ViewNode` and `ItemKind` remain the same closed enums, only their
*arm bodies* move.

## Target module layout

One crate (not a new crate — [[21-crate-layout|§21]] is unchanged), split
into concern-core + a per-component catalog:

```
crates/libra-components/src/
  lib.rs          — crate root: re-exports (stable public surface) + wiring
  node.rs         — shared vocabulary: ViewNode, ItemKind, BoxStyle, Variant,
                    TextSize, Role, Semantics, Item, LaidTree, InputFocus,
                    SourceDecl, TextMeasurer, LayoutError
  layout.rs       — layout(): taffy build + flatten; box_style(); build dispatch
  paint.rs        — paint()/paint_full()/hit(): the ItemKind dispatch loop
  catalog/
    mod.rs        — the one list of components; dispatch tables
    container.rs  — Column / Row / Box            (build only; paints nothing)
    text.rs       — Text
    button.rs     — Button
    input.rs      — Input       (build + paint)
    textarea.rs   — Textarea    (build + paint)
    list_item.rs  — ListItem    (build + paint, incl. hover wash)
    separator.rs  — Separator
    spacer.rs     — Spacer
    chart.rs      — Chart / Sparkline (build + paint, reads SeriesStore)
    source.rs     — Frames / Interval / Stream   (mount-only, §12.5)
```

**Each `catalog/<component>.rs` owns that component end to end:** its `build`
(measure + taffy + semantics) and its `paint` (`DrawCmd` emission), reading
**only theme tokens** — never raw hex ([[PLAN-TOKENS]] T3). The central
`build`/`paint` matches shrink to one-line delegations
(`ItemKind::Button => catalog::button::paint(item, cx)`), so the closed enums
and dual-engine contract are untouched while each component gains a home.

Free functions over a `Component` trait to start — the enums are closed, so a
trait object buys nothing yet. Revisit a trait only if the dispatch boilerplate
proves worse than the match (S3, optional).

## The authoring surface (the DX this unlocks)

Three clearly separated jobs, each with **one obvious place**:

| I want to… | Go to | Feedback loop |
|---|---|---|
| Change the **global look** (color, spacing, radius, type) | `themes/*.toml` — no Rust ([[PLAN-TOKENS]]) | Hot reload in the running app / gallery |
| Change **one component's** structure or paint | exactly one file: `catalog/<component>.rs` (tokens only) | Gallery snapshot diff |
| **Add** a component | copy the template `catalog/` module + wire the grammar (both engines + conformance) + a gallery cell | [[PLAN-CATALOG]] acceptance checklist |
| **Preview** everything | `examples/gallery` — every component × theme, hot-reloadable | The designer's live canvas |

Deliverables that make this real:

- **A documented component template** (`catalog/_template.rs` or a doc block
  in `catalog/mod.rs`) showing the minimal `build` + `paint` + semantics a new
  component implements — so "add a component" is copy-one-file, not
  reverse-engineer-the-match.
- **A one-screen authoring guide** (module doc on `catalog/mod.rs`, linked from
  [[07.1-builtin-catalog|§7.1]] and [[PLAN-CATALOG]]) that states the rules:
  tokens only, emit semantics, no wall-clock/env in paint (Appendix A), gallery
  cell + snapshot per component.
- **Tokens-only enforcement is now trivial**: with paint localized per file, a
  simple check ("no `Color::rgb`/hex literal under `catalog/`") makes
  [[PLAN-TOKENS]] T3's "no hex in components" mechanically verifiable.

## Stages

| Stage | Deliverable | Behavior change |
|---|---|---|
| **C0** | Extract `node.rs` (types) + `layout.rs` + `paint.rs` out of `lib.rs`; `lib.rs` re-exports the same public surface | **None** — pure move; snapshots byte-identical |
| **C1** | Introduce `catalog/`; move each component's `build`+`paint` arm into its own module behind thin dispatch | **None** — pure move |
| **C2** | Add the component template + authoring guide (§7.1 / PLAN-CATALOG cross-links) | Docs only |
| **C3** | Tokens-only check under `catalog/` (CI or `cargo test`), completing [[PLAN-TOKENS]] T3 enforcement | Lint only |
| **C4** *(opt.)* | A `Component` trait / registry **only if** it cuts boilerplate | Refactor, still no snapshot change |

Land C0–C1 as pure refactors first (safest, unblocks everyone); C2–C3 make the
surface approachable; C4 stays optional.

## Invariants

1. **Public surface unchanged.** `libra_components::{layout, LaidTree, ViewNode,
   ItemKind, Semantics, Role, …}` keep resolving at the crate root via
   `lib.rs` re-exports — **no downstream import changes** (the crate-level
   analogue of [[05.9-module-layout|§5.9]]'s "a split must never change a test
   import").
2. **Byte-identical snapshots** through C0–C1. The refactor ships with **no**
   `LIBRA_VALIDATE` re-record; if any snapshot moves, the move was not pure —
   stop and find why.
3. **Closed enums stay closed.** `ViewNode`/`ItemKind` live in `node.rs`
   unchanged; the two engines still emit `ViewNode`, so
   [[06.4-one-grammar-two-engines|§6.4]] and conformance are unaffected.
4. **Every component still emits `Semantics`** (a11y + harness queries) and
   obeys Appendix A on the paint path (no env/wall-clock).
5. Tokens remain **data** on `libra_draw::Theme`; components resolve names,
   never literals ([[11-theming|§11]], [[PLAN-TOKENS]]).

## Acceptance

- `cargo test --workspace` green; `examples/gallery` snapshots unchanged (no
  re-record) after C0–C1.
- No public path of `libra-components` changed (grep of downstream `use`s is
  identical).
- After C2: a new component can be added by copying one `catalog/` file +
  grammar wiring + a gallery cell, following one linked guide.

## Out of scope

- New components — that is [[PLAN-CATALOG]] (this plan only relocates the ones
  that exist).
- Token ramp/density/markup-ref work — that is [[PLAN-TOKENS]].
- The reconciler / retained component state — [[PLAN-RECONCILER]].
- Any grammar change; any new crate.

## In code

- `crates/libra-components/src/lib.rs` — the single file this plan splits
- `crates/libra-draw/src/lib.rs` — `Theme` + `tokens` (the styling vocabulary)
- `themes/` — designer-editable TOML theme tables
- `examples/gallery/` — the component × theme preview + snapshot surface

---

← [[Plans Index]] · [[PLAN-UI-SURFACE]] · [[PLAN-CATALOG]] · [[PLAN-TOKENS]] · [[07.1-builtin-catalog|§7.1]]
