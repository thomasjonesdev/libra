---
title: "PLAN-COMPOSE"
aliases:
  - "PLAN-COMPOSE"
  - "§7.2 composed components"
  - "composable lx"
tags:
  - plans
  - architecture
type: plan
---

# Composable `.lx` components Plan

**Status: done (2026-07-21).** Implements the [[07.2-composed-components|§7.2]] vision (a
`.lx` file with a `<props>` declaration *is* a component) so views stop growing
into single monoliths (the workbench `gallery.lx` is 389 lines with ~30 inlined
canvas blocks). React-familiar ergonomics — file = component, typed props,
`{children}` slots — expressed **entirely within the closed grammar**: no
expressions, no logic, no new runtime capability.

**Read before writing code:** [[07.2-composed-components|§7.2]] (the target
shape), [[06.2-closed-grammar|§6.2]] + [[06.3-grammar-restrictions|§6.3]] (what
markup may not do), [[06.4-one-grammar-two-engines|§6.4]] (both engines +
conformance are non-negotiable), [[06.5-compile-time-validation|§6.5]] (how
codegen turns call-site errors into rustc errors), [[appendix-a-determinism-checklist|Appendix A]].

## The problem (why now)

Views are data with a closed grammar — no expressions ([[06.2-closed-grammar|§6.2]]).
That is the right bet, but it has a side effect: **everything a view needs must
be present in the view or precomputed on State**, and there is no way to name a
recurring markup shape and reuse it. Consequence in a real app:

- `examples/gallery/views/gallery.lx` inlines a canvas block per component
  (`<if test={is_button}>…</if>`, ×30) → one 389-line file.
- Any repeated shape (a labelled field row, a metric card, a form group) is
  copy-pasted, and a change means editing every copy.

Composition is the missing **reuse** primitive. It is orthogonal to two other
closed-grammar costs that it does **not** solve (do not conflate them):

- The `is_*: bool` explosion + one `<if>` per branch is the *no-`<match>`/switch*
  problem — deferred with [[26-open-questions|§26]] #8 (typed enum-attr binding)
  and a future `<match>` element. Composition shrinks each block into a
  `<ButtonDemo/>` call but leaves the N booleans in place.
- Binding a closed-set attr (`variant`/`tone`) is [[26-open-questions|§26]] #8.
  Prop **passing** of enums is deferred to the same decision (see Out, below).

## Design tenets (Appendix A gate)

1. **Composition is pure tree substitution, before layout.** A composed view
   produces **byte-identical `ViewNode`s** to the hand-inlined equivalent — no
   determinism, purity, or paint-path impact. This is the load-bearing property
   test: `composed_tree_equals_inlined`.
2. **Props are the encapsulation boundary — a closed, typed interface.** Inside
   a component `.lx`, bindings and handlers resolve **only against its props**,
   never against app State. This is what makes a component reusable (as in
   React) and keeps the **closed action set**: a component dispatches only the
   handler props passed to it, inventing no actions of its own.
3. **Two engines, one grammar.** Interp expands component refs at resolve time;
   codegen emits one Rust fn per component and lowers each call site to a typed
   fn call, so rustc delivers [[06.5-compile-time-validation|§6.5]] validation
   for free. A `composed` conformance fixture is the drift alarm
   ([[06.4-one-grammar-two-engines|§6.4]]).
4. **File = component.** A `.lx` file whose first node is `<props …/>` is a
   component named by the PascalCase of its filename (`metric_card.lx` →
   `<MetricCard/>`). No registration boilerplate — the most React-familiar rule,
   and the interp already reads a PascalCase element as "a component reference"
   ([interp `require_camel_case` doc, lib.rs:33-36](../../crates/libra-interp/src/lib.rs#L33)).

## Grammar additions

Component file:

```lx
<props label: String, value: String, ratio: Num, onPick: Action />

<Card padding={12}>
  <Text muted size="sm">{label}</Text>
  <Row gap={8} cross="baseline">
    <Text size="xl">{value}</Text>
    <Progress value={ratio} />
  </Row>
  <Button variant="ghost" onPress={onPick}>details</Button>
</Card>
```

Call site (in any other view):

```lx
<MetricCard label="Revenue" value={total} ratio={pct} onPick={openRevenue} />
```

Prop types in v1 mirror what the grammar already binds today, so no new binding
machinery is invented:

| Prop type | Backed by | Binds like |
|---|---|---|
| `String` | Display path | `{label}`, string-literal attr |
| `Num` | typed `f32`/`u32` field (§6.5, §26 #8) | `value={ratio}` |
| `Bool` | Display `"true"`/`"false"` | `<if test={flag}>` |
| `Action` | unit handler | `onPress={onPick}` |
| `Action(String)` | String-payload handler | `onChange={onEdit}` |
| `slot` (children) | call-site children → `<slot/>` | see C6 |

## v1 scope

| In (this plan) | Out (later / other plans) |
|---|---|
| `<props name: Type, …/>` as the component file's first node | Default prop values; optional props |
| PascalCase call sites resolving to a component file by filename | Explicit registration / import paths |
| Prop-only scope inside a component (no ambient State) | Components reaching into State (rejected — see tenet 2) |
| `String` / `Num` / `Bool` / `Action` / `Action(String)` props | **Enum props** (`tone`/`variant`) — ties to [[26-open-questions|§26]] #8 |
| A single `{children}` `<slot/>` (C6) | Named/multiple slots |
| Nested composition (a component using another) | — |
| Cycle / arity / type / unknown-prop diagnostics with spans | — |
| Passing scalars + handlers as props | **Passing a collection** as a prop (`<for>` over a prop) |
| Both engines + `composed` conformance fixture | `<match>`/switch (kills the `is_*` set — separate) |

Everything in the right column keeps shapes compatible with [[26-open-questions|§26]];
do not build ahead of it.

## Diagnostics (span-carrying, both engines)

Interp errors at reload; codegen turns the same into rustc errors at the `.lx`
span ([[06.5-compile-time-validation|§6.5]]):

- `<MetricCard/>` missing required prop `value` → error at the call span.
- Unknown prop `<MetricCard colour="…"/>` → "no prop 'colour' on MetricCard — did you mean 'label'?"
- Type mismatch: `ratio={name}` where `name: String` but prop is `Num` → typed error at span.
- Recursion cycle `A → B → A` → load/compile error naming the cycle.
- `<props>` not the first node, or a call site to a non-existent component → span error.

## Wiring (registry threading)

Today the platform parses one `lx_source` into `self.ast` and resolves it each
frame ([platform lib.rs:217, :443](../../crates/libra-platform/src/lib.rs#L217)).
Composition needs a **component registry** (name → parsed AST + prop decl)
threaded into `resolve`:

- **Discovery:** every `.lx` in the view directory whose first node is `<props>`
  registers as a component (PascalCase filename). The `app!` `view:` field still
  names the *root*; siblings are auto-discovered. (Alternative considered:
  explicit `components:` list in `app!` — rejected as boilerplate; auto-discovery
  matches the file=component rule. Revisit only if a namespacing need appears.)
- **Interp:** `resolve` gains the registry; a PascalCase ref builds a fresh
  prop-only `Scope` from the passed attrs and expands the component's AST into
  the tree.
- **Codegen:** `build.rs` enumerates component files, emits `fn metric_card(props…) -> ViewNode`,
  lowers call sites to calls. Add a `composed` fixture to `conformance/`.
- **Harness:** `libra-test` loads the same registry so component/snapshot tests
  see composed views identically.

## Stages

- **C1 — Parser.** `<props …/>` decl (typed param list) + treat a PascalCase
  element as a component ref (name + attrs + children). `libra-markup` only.
- **C2 — Interp.** Registry param on `resolve`; prop-only `Scope` construction;
  expansion; cycle/arity/type/unknown-prop diagnostics with spans.
- **C3 — Codegen.** One fn per component (typed params); call sites → typed fn
  calls; cycle detection at build; §6.5 error trailers.
- **C4 — Conformance.** `composed.lx` host + `metric_card.lx` component fixtures;
  extend `interp_and_codegen_produce_identical_trees`; add
  `composed_tree_equals_inlined` (the tenet-1 guardrail).
- **C5 — Wiring.** Auto-discovery in `app!`/platform/harness; `libra check`
  validates the whole registry.
- **C6 — Slots.** Single `{children}`/`<slot/>` so wrapper components work
  (Card-like layout shells).
- **C7 — Dogfood / acceptance.** Refactor `gallery.lx` canvas blocks into
  per-component demo components (`<ButtonDemo/>`, `<BadgeDemo/>`, …). Snapshots
  must stay byte-identical (proves tenet 1 end-to-end); the file shrinks from a
  monolith to a thin `<if>`-dispatch shell over component calls.

## Acceptance

- [x] `interp_and_codegen_produce_identical_trees` green for the `composed` fixture.
- [x] `composed_tree_equals_inlined`: a composed view and its hand-inlined twin
      produce identical `ViewNode`s (and therefore identical snapshots).
- [x] Missing/unknown/mistyped prop and recursion cycle are span-carrying errors
      in interp **and** rustc errors at the `.lx` line in codegen.
- [x] Component bindings/handlers resolve only against props (an app-State path
      referenced inside a component is an error, not a silent capture).
- [x] `gallery.lx` refactored onto components; every `workbench_*` snapshot
      byte-identical; README hashes unchanged.
- [x] `cargo test --workspace` + `cargo run -p libra-cli -- check .` green.

## When this lands (vault checklist — [[AGENTS#9 How to change the system safely|AGENTS §9]])

- [[07.2-composed-components|§7.2]] gains an "Implemented" note + link here.
- [[AGENTS]] §8 "still open" → move composition to done; [[Plans Index]] status.
- `SECTION-INDEX` unchanged (no new subsection — §7.2 already exists).

---

← [[Architecture Home]] · [[Plans Index]] · governing [[07.2-composed-components|§7.2]] · grammar gate [[06.4-one-grammar-two-engines|§6.4]]
