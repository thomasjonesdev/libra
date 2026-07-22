---
title: "PLAN-GALLERY"
aliases:
  - "PLAN-GALLERY"
  - "component workbench"
  - "libra storybook"
tags:
  - plans
  - architecture
type: plan
---

# Gallery → component workbench (Libra Storybook)

**Status: in progress — G0–G3 landed 2026-07-18; M8 (2026-07-21) completed the
per-component × dark/light snapshot matrix** (every shipped workbench cell has
both themes; Textarea/ListItem resting canvases added). Grew `examples/gallery`
from a single flat matrix into a **Storybook-style workbench**: a sidebar of
every catalog component, click one to load it *in isolation* with its variants
and a live text control. Modeled on the `examples/notes` three-pane layout
(§5.9), 1200×760. Ties: [[07.1-builtin-catalog|§7.1]] (the catalog it browses),
[[18-developer-experience|§18]] + Appendix C (Storybook is the borrowing),
[[PLAN-COMPONENTS]] / [[PLAN-CATALOG]] (what it renders).

**Landed:** the §5.9 split (`model`/`state`/`actions`/`lib`), the three-pane
shell (grouped sidebar via `<for>`, canvas, controls), selection via
`#[project]` `is_*` bools, per-component canvas blocks (variants matrices +
playground) for all 12 shipped visible components, and a shared `label` text
control. Snapshots are per-component (`workbench_button/badge/chart` × themes).
Charts auto-run off the data plane (no `<Frames>`). **No grammar change.**
Remaining: add a canvas block whenever a new catalog component lands.

## Today

`examples/gallery` is **one static screen** — every component × state laid out
at once, snapshot-covered under both themes (the PR visual-review surface). It
proves rendering but doesn't *isolate*: a Button change and a Chart change land
in the same 640×360 image, and there's nowhere to poke a component's props.

## The constraint that shapes everything (read first)

Storybook's "controls" live-set arbitrary props via JS. **Libra's grammar is
closed** — an attribute is either a literal or a `{state}` binding, and only
*some* attributes bind today:

| Binds to state today (`{field}`) | Static-only (literal in markup) |
|---|---|
| text content, `Input`/`Textarea` `value`, `ListItem` `title`/`subtitle`/`meta`, `ListItem` `selected` (bool), `if test` (bool), `Chart` `series` | `variant`, `size`, `tone` (**enums**); `bold`/`muted`/`grow` (**flags**); `gap`/`padding`/`height`/`width` (**numbers**) |

So a control can drive a **text** or **bool** prop directly, but it **cannot**
live-set `variant`/`tone`/`size`. The workbench lives entirely within that:

- **Text props** bind to an `Input` (a Button's label, a Text's content, a
  Badge's label) — real controls, on the grammar.
- **Enum props** (variant/tone/size) are shown as a **static variants matrix** —
  all discrete values side by side. This *is* "show its variants," and it needs
  no live picker.
- **Bool props** toggle a small `<if>` (or bind a bindable bool like `selected`).

**Decoupled from any grammar change.** An earlier draft proposed a "bind
enum/flag attrs" grammar feature to get live enum pickers. It is deliberately
**not** a stage here: for closed-enum attrs, a stringly binding would move
validity from **compile-time to runtime** (eroding [[06.5-compile-time-validation|§6.5]]),
and the only sound form — binding a *typed* enum field — is a reflection change
that must be justified by real app needs, **not** by this tool. The variants
matrix already shows every value, so the workbench doesn't need it. That idea
now lives as an open question ([[26-open-questions|§26]] — typed-value binding),
independent of this plan.

## Target layout (modeled on `examples/notes`)

Three panes in a `Row grow`, `Separator vertical` between — the notes shape,
one pane wider:

```
┌───────────────────────────────────────────────────────────────┐
│ Libra Workbench                                    {status} [◧] │  header row
├───────────┬───────────────────────────────────┬───────────────┤
│ Components│  Button                            │ Controls      │
│ Layout    │  ─────────────────────────────     │  label [____] │
│   Column  │  Variants                          │  variant  [▸] │
│   Row     │  [Primary][Secondary][Ghost]       │  bold     [ ] │
│ Display   │                                    │               │
│ ▶ Button  │  Playground                        │               │
│   Badge   │  [ Your Label ]                    │               │
│   Text    │                                    │               │
│ Data      │                                    │               │
│   Chart   │  (live: reads control state)       │               │
└───────────┴───────────────────────────────────┴───────────────┘
  sidebar (fixed width)    canvas (selected story)   controls (fixed width)
```

- **Sidebar** — grouped component list. `<for each={component_rows} as="c"
  key={c.id}><ListItem title={c.name} selected={c.selected}
  onPress={select(c.id)}/></for>`, one `<for>` per group (Layout / Display /
  Controls / Data) under a `<Text bold>` header. Fixed `width={200}`.
- **Canvas** — the selected component's story. A stack of per-component
  `<if test={is_button}>…</if>` blocks (one per catalog entry): a title, a
  **Variants** matrix (all static values), and a **Playground** instance whose
  bindable props read the control fields.
- **Controls** — `<if>`-gated per component: an `Input` per text prop, a cycle
  `Button` per enum, a toggle for each bool. Fixed `width={240}`.
- **Window**: bump from 640×360 to ~**1200×760** (fits three panes; notes uses
  a similarly generous frame).

### Selecting is a bool per component (closed-grammar idiom)

`<if test={…}>` takes a **bool**, not an equality expression (expressions are
forbidden, §6.3). So selection is `selected: String` on State plus one derived
bool per component (`is_button`, `is_badge`, …) rebuilt in the **`#[project]`**
hook (§6.3) — the same pattern `examples/notes` uses for its row projections.
~15 bools is the honest cost; a future **`<switch>`/`<match>` element** (or
`test={selected == "button"}`) would fold them away — noted, not built.

## State & module layout (§5.9)

Past ~200 lines, split like `examples/notes`:

```
examples/gallery/src/
  model.rs    — ComponentEntry { id, name, group }; the static catalog registry
                as data; control defaults
  state.rs    — Gallery { selected, component_rows, controls…, per-component
                is_* bools } + inherent helpers
  actions.rs  — select(id), control edits (labelChanged, cycleVariant, toggleX),
                #[project] rebuilding component_rows + the is_* bools
  lib.rs      — re-exports + app!  (window 1200×760)
```

Controls are a **shared, reused set of fields** interpreted per component
(`ctl_text`, `ctl_variant_ix`, `ctl_bool`, …) so the state stays small; the
canvas's per-component block reads whichever it needs.

## Stages

| Stage | Deliverable |
|---|---|
| **G0 + G1** | `examples/gallery` rebuilt in the §5.9 module layout as a three-pane shell: grouped sidebar (`<for>` per group), canvas, controls panel, header; `select(id)` + `#[project]` `is_*` bools; window 1200×760 |
| **G2** | Per-component canvas blocks = **Variants matrices** (migrate today's static rows, one component per `<if>` block) |
| **G3** | Controls panel: text `Input` → a Playground instance per text-bearing component, bound to a shared `ctl_*` field |
| **G4** *(optional polish)* | Bool toggles via `<if>` where a component has a bindable bool; skip enum cycles (the matrix covers variants) |

Each catalog component gets a canvas block + (where it has text props) a
control, one at a time — the same "one component at a time" cadence as
[[PLAN-CATALOG]], and adding a component to the catalog now includes adding its
workbench block. **No stage depends on a grammar change.**

(G0 and G1 merge in practice: the flat view is replaced wholesale, so a
"pure split first" buys nothing — the split lands with the new shell.)

## Snapshots

The flat matrix is **replaced** by per-component isolation — a better
regression surface (a Button change fails only Button's snapshot, not a shared
image):

- Landing (default `selected`) × dark/light.
- One canvas snapshot **per component** (select it, snapshot) as blocks land.
- A couple of **controls-interaction** snapshots (typed label, toggled bool,
  cycled variant) driven deterministically (`type_text`/`click`).

Existing `gallery_*` snapshots are re-recorded once, deliberately (§15.5). No
`README` hash change (gallery isn't hash-pinned there; only counter is).

## Invariants

1. **Closed grammar holds** — no dynamic "render component by name"; each story
   is static markup gated by an `is_*` bool. Controls only bind what the grammar
   already binds (text/bool); no grammar change is introduced by this plan.
2. **Determinism** — control fields have fixed defaults, so default snapshots
   are stable; interactive states are exercised with explicit input, never
   ambient (Appendix A).
3. **Tokens only** — the workbench chrome styles through theme tokens
   ([[PLAN-TOKENS]]); it dogfoods the catalog it browses.
4. **No new determinism/purity surface** — this is an example app; it adds no
   crate contract. (G5's grammar feature is the only core change, and lands as
   its own dual-engine + conformance plan.)
5. **§5.9 split never changes a test import** — public `gallery::{…}` stays flat.

## Decisions (resolved 2026-07-17)

- **Controls depth**: text controls + static variants matrices (G2–G3); enum
  controls are the matrix, not a live picker. No grammar change.
- **Panes**: 3-pane (sidebar | canvas | controls), matching Storybook and notes.
- **Snapshots**: the flat matrix is **replaced** by per-component isolation —
  the better regression surface.

## Out of scope

- MDX/docs pages, a11y/measure addons, args tables autogenerated from types.
- Rendering a component from a string name (needs open dispatch — against the
  closed grammar).
- Live enum/flag controls and the grammar change they'd require — see
  [[26-open-questions|§26]] (typed-value binding); explicitly **not** a gallery
  dependency.

## In code

- `examples/gallery/` — the app this plan reshapes (today one `lib.rs` + `gallery.lx`)
- `examples/notes/` — the three-pane §5.9 layout reference to model on
- `crates/libra-components/src/catalog/` — the components it browses ([[PLAN-COMPONENTS]])

---

← [[Plans Index]] · [[07.1-builtin-catalog|§7.1]] · [[18-developer-experience|§18]] · [[PLAN-CATALOG]] · [[PLAN-COMPONENTS]]
