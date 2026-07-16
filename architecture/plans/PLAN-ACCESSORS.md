---
title: "PLAN-ACCESSORS"
aliases:
  - "PLAN-ACCESSORS"
tags:
  - plans
  - architecture
type: plan
---

# Top-level paths & accessor whitelist

**Status: complete.** Landed 2026-07-16. Nested `{user.name}` via
ReflectState recursion; `len` / `isEmpty` on strings & collections
(optional `()`); `test={items.isEmpty}` / `test={user.active}` for
`<if>`. Conformance `accessors` fixture; `cargo test --workspace` green.

## v1 scope

| In | Out |
|---|---|
| Top-level one-segment dots `{user.name}` where `user` is a nested ReflectState struct field | Format specs `{count:>6}` (follow-on) |
| Accessors `len`, `isEmpty` on collections / strings (camelCase; optional `()`) | Generated enum `is*` predicates (follow-on) |
| `test={items.isEmpty}` / nested bool dots for `<if>` | Arbitrary methods beyond the whitelist |

## Where

`libra-core` ReflectState path resolution (derive), both engines, conformance, §6.2.

---

← [[Plans Index]] · [[06.2-the-closed-grammar-what-markup-can-do|§6.2]]
