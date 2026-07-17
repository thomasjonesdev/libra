---
title: "PLAN-IF"
aliases:
  - "PLAN-IF"
  - "§6.2 if/else"
tags:
  - plans
  - architecture
type: plan
---

# `<if>` / `<else>` Plan

**Status: complete.** Landed 2026-07-16. `cargo test --workspace` green;
`if_else` conformance fixture; dashboard uses `<if test={running}><Frames/></if>`
(no `subscriptions:`); source elements excluded from flex layout so snapshots
stay byte-identical.

**Read before writing code:** [[06.2-the-closed-grammar-what-markup-can-do|§6.2]], [[06.4-one-grammar-two-engines|§6.4]], [[12.5-declarative-subscriptions-effects-as-mount-scoped-data|§12.5]], [[PLAN-ERGONOMICS]] E3 acceptance (dashboard `<if>` + `<Frames>`).

## v1 scope (this plan)

```lx
<if test={running}>
  <Frames onTick={frame} />
  <else>
    <Text muted>paused</Text>
  </else>
</if>
```

| In | Out (later plans) |
|---|---|
| `test={ident}` — top-level bool (Display `"true"` / `"false"`) | `test={path.isLive()}`, zero-arg Call attrs |
| Optional single nested `<else>…</else>` | `<else if>` chains |
| Expand away in both engines (no `ViewNode::If`) | Expressions, `!`, `&&` |
| Conformance fixture | Generated enum `is*` predicates |
| Dashboard: markup `<if>` replaces `subscriptions:` | Loop-scoped `test={t.done}` (needs item bool reflect — follow-on) |

Truthiness: field Display equals `"true"` (case-sensitive). Codegen emits `if state.flag { … } else { … }` when the field is `bool` (typed); interp uses the Display rule so engines agree for bool fields.

## Execution

1. Vault: §6.2 Implemented today
2. Interp `resolve_if` + Codegen `gen_if` (both in `resolve_children` / `gen_children`)
3. Conformance `if_else.lx`
4. Dashboard port
5. Hygiene

## Acceptance

- [x] `interp_and_codegen_produce_identical_trees` for `if_else` across `running: true/false`
- [x] Dashboard: no `subscriptions:` field; `<if test={running}><Frames …/></if>` in view; snapshots byte-identical (invisible source)
- [x] Nested `<if>` under `<for>` works; bare `<else>` outside `<if>` is a span error
- [x] `cargo test --workspace` green

---

← [[Architecture Home]] · [[Plans Index]] · governing [[06.2-the-closed-grammar-what-markup-can-do|§6.2]]
