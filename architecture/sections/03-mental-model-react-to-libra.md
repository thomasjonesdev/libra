---
title: "3. Mental Model: React → Libra"
section: "3"
aliases:
  - "§3"
  - "3"
  - "3. Mental Model: React → Libra"
tags:
  - architecture
  - section/3
type: architecture
---

# 3. Mental Model: React → Libra

## In this vault

- [[03.1-the-vocabulary-five-words-all-of-them-already-yours]]
- [[03.2-terminology-migration-complete-this-document-and-the-code-now-agree]]
- [[03.3-naming-conventions-reacts-verbatim]]

---

For a developer fluent in React, the translation table:

| React concept | Libra equivalent | Notes |
|---|---|---|
| Component | A `.lx` markup file + optional Rust component fn | Same composition model, same word ([[07-components|§7]]) |
| JSX | `.lx` markup (JSX-flavored, closed grammar) | See [[06-the-view-layer-and-markup|§6]] |
| Props | Typed attributes / a props struct | Validated at compile time in release |
| `useState` | Fields on the single `State` struct | State is centralized, not tree-resident |
| `setState` / reducers | **Actions** — methods on `State` in `#[libra::actions]` impls ([[05-state-and-actions|§5]]) | Redux Toolkit's `createSlice`, with real types |
| `useEffect` / thunks & sagas | `Effect` values returned by actions; `Sub` for subscriptions | Effects are data ([[12-the-effects-runtime|§12]]) |
| `useEffect` for lifecycle ("while mounted…") | Source elements in markup: `<Frames/>`, `<Interval/>`, `<Stream/>` ([[12.5-declarative-subscriptions-effects-as-mount-scoped-data|§12.5]]) | Declarations diffed on mount/unmount — data, never code |
| `fetch()` | `Effect::http(...)` | Runtime executes; tests assert |
| `localStorage` | `Effect::kv_set` / `Effect::kv_get` → `redb` | Better than localStorage ([[13-persistence|§13]]) |
| `key` prop | `key` attribute on `<for>` | Same purpose (identity across reorder) |
| Conditional render `{cond && <X/>}` | `<if test={cond}>` | Closed grammar form |
| List render `.map()` | `<for each={items} as="item" key="item.id">` | Closed grammar form |
| Context | State slices ([[05.7-scaling-state-slices-not-fragmentation|§5.7]]) | No hidden ambient state |
| `useRef` + imperative escape | The data plane ([[14-the-high-rate-data-plane|§14]]) + Canvas component ([[07.4-canvas-advanced-custom-ui|§7.4]]) | Purity for logic, throughput escape hatch |
| React DevTools | Time-travel inspector ([[18.5-time-travel-and-session-capture|§18.5]]) | Every action logged, scrubbing, session export |
| Jest + Testing Library | Unit / component / integration tests ([[15-testing-architecture|§15]]) | No mocks, no fake DOM |
| Storybook + Chromatic | `libra gallery` + snapshot tests ([[15.5-snapshot-tests-byte-exact-pixels|§15.5]]) | No fuzz tolerance, no browser farm |
| Playwright | Replay tests ([[15.6-replay-tests-recorded-sessions|§15.6]]) + automation server ([[16-the-automation-server|§16]]) | Typed dispatch, byte-exact assertions |

The intended feeling: a React developer reads a Libra app and knows where everything is within minutes. The differences (centralized state, effects-as-data) are the *same* differences a React team adopts voluntarily when it brings in Redux Toolkit + MSW — here they are the defaults, and the tooling is built around them.

---

← [[Architecture Home]] · [[Map of Content]]
