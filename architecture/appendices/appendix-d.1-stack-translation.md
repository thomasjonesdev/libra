---
title: "D.1 Stack translation"
section: "D.1"
aliases:
  - "D.1"
  - "D.1 Stack translation"
tags:
  - architecture
  - appendix
type: architecture
---

# D.1 Stack translation

## In this vault

- Parent: [[appendix-d-team-and-use-case-fit-ground-crew-operations-reference-case]]

---

| Existing | Libra equivalent | Transfer quality |
|---|---|---|
| React components / JSX | `.lx` markup, typed props, composition, slots | Near 1:1 (surface preserved deliberately, [[03-mental-model-react-to-libra|§3]]) |
| Redux Toolkit / reducer patterns | Actions + the generated reducer ([[05-state-and-actions|§5]]) | 1:1 — it is this |
| TanStack Query | `libra-query` (keys, staleness, invalidation, retry) | Near 1:1 vocabulary |
| TanStack Table | Table component's headless column model ([[07.1-the-built-in-catalog-closed-curated|§7.1]]) | Near 1:1 |
| TanStack Router | `Screen` enum + nav stack; `libra://` deep links later | Conceptual |
| vanilla-extract (typed tokens/sprinkles) | Design tokens ([[11-theming-and-design-tokens|§11]]): same discipline, TOML + typed resolution | Near 1:1 — design-system practice transfers whole |
| Jest + Testing Library | Unit / component / integration tests, same selector philosophy | Near 1:1, minus all mocking |
| Storybook | `libra gallery` | 1:1 intent |
| Playwright/e2e | Automation server ([[16-the-automation-server|§16]]) + replay tests | Better: typed dispatch, byte-exact |
| Rust services / sidecars | In-process custom effect executors (NATS/DDS, [[13.3-beyond-kv|§13.3]]) | Sidecar layer deleted |
| Node + Postgres backend | **Unchanged.** Libra is client-only; service topology stays | N/A |
| Tauri 2 tablet apps | Coexist during pilot; Linux+touch is a native Libra target ([[19.1-touch-first-class-not-an-afterthought|§19.1]]) | Migration, not big-bang |

Type sharing across the boundary: if services are (or become) Rust, serde structs are shared directly between service and UI crates — deleting the codegen'd-TS-types pipeline. With a Node backend retained, schema-first types (JSON Schema/OpenAPI → serde derive) fill the same role.

---

← [[Architecture Home]] · [[Map of Content]] · parent [[appendix-d-team-and-use-case-fit-ground-crew-operations-reference-case]]
