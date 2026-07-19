---
title: "PLAN-ITEM-FIELDS"
aliases:
  - "PLAN-ITEM-FIELDS"
  - "§6.2 item fields"
tags:
  - plans
  - architecture
type: plan
---

# §6.2 Item Fields Plan

**Status: complete.** Landed 2026-07-16. `cargo test --workspace` green;
`for_item_fields` conformance fixture guards interp≡codegen; `examples/todos`
ports to `{t.text}` / `key={t.id}` / `onPress={complete(t.id)}` (structured
`active`/`done: Vec<Todo>`); todos mixed-list snapshots re-blessed (labels no
longer encode `"id. text"`). Opaque `Vec<SeriesHandle>` / `Vec<String>` keep
whole-value iteration.

**Read before writing code:** [[06.2-closed-grammar|§6.2]]
(implemented-today vs vision), [[06.3-grammar-restrictions|§6.3]]
(`#[project]` projections; filtered lists until `<if>`),
[[06.4-one-grammar-two-engines|§6.4]] (dual-engine + conformance),
[[06.5-compile-time-validation|§6.5]],
[[05.2-macro-expansion|§5.2]],
[[appendix-a-determinism-checklist|Appendix A]].

Design is settled below; this plan is execution order, acceptance criteria,
and traps. Architecture notes stay normative — Phase 0 updates §6.2's
"Implemented today" in the **same change** as the first code PR that lands
behavior.

## Why

Today both engines treat `<for>` items as opaque `Display` strings
(`ReflectState::collection` → `Vec<String>`). That is enough for dashboard
charts (`key={s}`) and for the first item-scoped handlers
(`onPress={complete(t)}`), but it forces todos to:

1. Encode identity + label into one line (`"3. Buy milk"`).
2. Project `active` / `done` as `Vec<String>`, not structured rows.
3. Parse that line back in actions to find the real `Todo`.

§6.2's vision already names the fix: `{item.field}`, `key={item.id}`,
`onPress={select(item.id)}`. This plan ships **only that closed loop-scoped
slice** — not the rest of the vision.

## Scope fence

### In (this plan)

| Capability | Example |
|---|---|
| Loop-scoped field bind | `{t.text}` inside `<for each={…} as="t">` |
| Field key | `key={t.id}` |
| Field action arg | `onPress={complete(t.id)}` |
| Structured `Vec<T>` reflection | `T` with named `Display` fields → per-row field map |
| Todos port | Drop the `"id. text"` hack; actions take a stable id string |

### Out (do not build in this plan)

- Top-level / non-loop dot-paths (`{user.name}`)
- Accessor whitelist (`{items.len()}`, `isEmpty`, format specs)
- Nested paths beyond one segment (`item.a.b`)
- `<if>` / `<else>` ([[26-open-questions|§26]])
- `<Interval>` / `<Stream>` ([[12.5-declarative-subscriptions|§12.5]])
- Composition / slots
- A real reconciler (keys remain identity *data* for later; layout still rebuilds fully)

Universal ground rules (same as [[PLAN-ERGONOMICS]]): one PR per phase when
practical, `cargo test --workspace` green at every boundary, grammar changes
always land in **both** engines with a `conformance/` fixture
(`interp_and_codegen_produce_identical_trees` is the drift alarm), bless
snapshots only when pixels intentionally change.

---

## Decision locked: filtered lists until `<if>`

Until `<if>` exists, todos keeps **two projected collections** (`active` /
`done`) rather than one `items` list with conditional row chrome.

**Choice A (required):** projections become structured rows (or keep
`Vec<Todo>`-shaped display lists), and each `<for>` uses `{t.text}` /
`complete(t.id)` / `key={t.id}`.

**Rejected — Choice B:** single `items` list mixing active and done in one
visual band (worse UX, not worth waiting on).

`#[project]` still rebuilds the filtered collections after every action
([[06.3-grammar-restrictions|§6.3]]); actions
only mutate the source-of-truth `items`.

Target markup after the port:

```lx
<for each={active} as="t" key={t.id}>
  <Row gap={8} cross="center">
    <Button variant="secondary" onPress={complete(t.id)}>{t.text}</Button>
    <Button variant="ghost" onPress={remove(t.id)}>×</Button>
  </Row>
</for>
```

(and the analogous `done` / `uncomplete` block).

---

## Execution order

| # | Phase | Where |
|---|---|---|
| 0 | Lock the contract in the vault | [[06.2-closed-grammar|§6.2]] |
| 1 | Structured collection reflection | `libra-core`, `libra-reflect` |
| 2 | Loop field resolution in both engines | `libra-markup` (if needed), `libra-interp`, `libra-codegen`, `conformance/` |
| 3 | Dispatch uses resolved field strings | harness + shell (likely already fine) |
| 4 | Port `examples/todos` | example + tests + snapshots |
| 5 | Brief / roadmap hygiene | [[AGENTS]], CLAUDE.md, this plan status |

Prefer **0+1** as the first PR (API without grammar consumers), then **2+3**
(grammar + conformance), then **4+5** (example + docs). Combining 0 into the
first code PR that changes observable behavior is also fine — never leave
§6.2's "Implemented today" lying.

---

## Phase 0 — Lock the contract ([[06.2-closed-grammar|§6.2]])

Update **Implemented today** (not only the vision bullets) to state:

1. Inside `<for>`, `{loopVar.field}` binds when `field` is a named field on
   the item element type.
2. `key={loopVar.field}` and `onPress={action(loopVar.field)}` are allowed;
   the arg must still target a `String`-payload action. Field values reach
   the action via `Display` → `String` (same channel as today's whole-item
   Display payload).
3. One path segment only — `item.a.b` is a span-diagnosed error.
4. Whole-value items (`Vec<String>`, `Vec<SeriesHandle>`) keep current
   behavior: `key={x}`, `onPress={toggle(x)}`, no `.field`.
5. Conformance fixtures are the drift alarm ([[06.4-one-grammar-two-engines|§6.4]]).

**Acceptance:**

- [ ] §6.2 "Implemented today" / "Not built" lines match the fence above.
- [ ] This plan stays linked from Plans Index; status stays `planned` until
      Phase 4 lands.

---

## Phase 1 — Reflection (`libra-core` + `libra-reflect`)

Today:

```text
ReflectState::collection(path) -> Option<Vec<String>>
```

That forces the Display encoding. Extend the reflection surface so engines
can resolve `loopVar.field` without inventing per-engine type knowledge.

**Preferred shape (implementer's choice, keep it small and shared):**

- Per-row field map: e.g. `collection_rows(path) -> Option<Vec<Row>>`
  where `Row` is an ordered map of field name → Display string, **or**
- Keep `collection()` for whole-value / key fallback and add
  `collection_field(path, index, field) -> Option<String>`.

Derive / `#[libra::state]` rules:

- `Vec<T>` where `T` has named fields that are `Display` → structured rows.
- `Vec<String>` / other whole-value Display element types → current
  whole-value behavior (dashboard `SeriesHandle` must not regress).
- `#[libra(skip)]` on an item field excludes it from markup (same as state).

**Acceptance:**

- [ ] `libra-reflect` unit tests: Todo-like struct fields appear; skipped
      fields do not; `Vec<String>` / `Vec<SeriesHandle>` still expose
      whole-value iteration.
- [ ] No grammar change yet — existing todos / dashboard / conformance stay
      green without markup edits.

**Traps:** macros can't resolve real types — syntactic `Vec<…>` detection
already exists; structured-item detection must follow the same tradeoff
(false positives fail at compile time). Do not route scalar state fields
through the new row API.

---

## Phase 2 — Markup path resolution (both engines + conformance)

In **both** `libra-interp` and `libra-codegen` (and markup only if the AST
needs a richer path node):

1. Resolve `loopVar.field` in text binds, `key=`, and
   `onPress={action(...)}` args when `loopVar` is the in-scope `<for>`
   binding.
2. Validate: field exists on the item (interp: load-time diagnostic with
   span + suggestion; codegen: direct field access so rustc enforces it).
3. Reject nested paths and unknown names with the same diagnostic spirit in
   both engines.
4. Add conformance fixtures, e.g. `for_item_fields.lx` (+ actions companion
   if needed). Keep `for_loop` / `for_actions` green (whole-value regression
   guards).

**Acceptance:**

- [ ] `interp_and_codegen_produce_identical_trees` green for the new
      fixture(s).
- [ ] Unknown field / nested path → span error in interp; codegen fails
      closed (rustc or explicit diagnose — pick one and document in §6.2).
- [ ] Dashboard-style `key={s}` whole-value fixtures unchanged.

**Traps:** dual-engine drift is the failure mode this whole stack exists to
catch — never land interp-only. Codegen today assumes loop roots are
Display strings ([[06.4-one-grammar-two-engines|§6.4]]); structured rows
must not break `SeriesHandle` chart props inside `<for>`.

---

## Phase 3 — Dispatch path

Harness and shell already dispatch item-scoped handlers via
`ReflectAction::action_with_str`. Confirm the resolved **field** string
(e.g. id) is what gets carried on `on_press_arg` — same channel as today's
whole-item Display, different source.

**Acceptance:**

- [ ] Component-level click on a row still hits the correct action payload
      (covered by todos component tests in Phase 4; add a conformance-level
      or harness unit if dispatch wiring changes).
- [ ] No new action-payload kinds — still `String` only for markup-built
      handlers.

---

## Phase 4 — Port `examples/todos`

1. Source of truth stays `items: Vec<Todo>` (`#[libra(skip)]` or reflected —
   only one collection strategy; prefer skip + projected structured
   `active`/`done` lists so the view never iterates unfiltered items).
2. Drop `line()` / `"id. text"` identity. Actions take the id (Display →
   `String`) from `complete(t.id)` / `remove(t.id)`.
3. `#[project]` rebuilds structured `active` / `done` (and `summary`) from
   `items`.
4. Update unit, component, and snapshot tests. Bless deliberately
   (`LIBRA_BLESS=1`) only if pixels change; update README hashes with the
   PNGs.
5. Module docs cite §6.2 item fields (this plan) and §6.3 `#[project]`.

**Acceptance:**

- [ ] No Display-encoded row identity remains in `examples/todos`.
- [ ] `cargo test -p todos` green; component tests still prove
      per-row payload specificity.
- [ ] Snapshots blessed only if visual output changed, and hashes updated
      together.

---

## Phase 5 — Hygiene

Per [[AGENTS]] §9 (light checklist, same change as the landing PR):

- [ ] §6.2 "Implemented today" matches shipped behavior (from Phase 0).
- [ ] [[AGENTS]] §8 / repo `CLAUDE.md`: move item-field bullet from
      "still future" to built; keep `<if>` / accessors / top-level dots as
      remaining.
- [ ] This plan → **Status: complete** with a one-paragraph landing note
      (commands run, any deliberate snapshot bless).
- [ ] Optional: one-line pointer from [[26-open-questions|§26]] open
      question #1 if the whitelist breadth note still mentions item
      sub-bindings as unresolved — clarify loop-scoped fields are done,
      accessors are not.

---

## Verification (after Phase 4)

```sh
cargo test -p libra-reflect
cargo test -p libra-interp
cargo test -p libra-codegen
cargo test -p conformance
cargo test -p todos
cargo test --workspace
```

If snapshots moved:

```sh
LIBRA_BLESS=1 cargo test -p todos --test snapshots
# update README SHA-256 hashes with the blessed PNGs
```

---

## Risks / traps (summary)

| Risk | Mitigation |
|---|---|
| Interp≠codegen drift | Conformance fixture + both engines in one PR |
| Dashboard `Vec<SeriesHandle>` regression | Whole-value path remains; existing fixtures must stay byte/tree identical |
| Inventing a reconciler | Keys are data only; full rebuild stays |
| Scope creep into `<if>` / `len()` / `{user.name}` | Fence above; say no |
| Stale vault | Phase 0 + Phase 5 in the landing changes |

---

← [[Architecture Home]] · [[Plans Index]] · [[Map of Content]] · governing [[06.2-closed-grammar|§6.2]]
