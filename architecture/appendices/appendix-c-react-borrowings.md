---
title: "Appendix C — React Ecosystem Borrowings"
section: "Appendix C"
aliases:
  - "Appendix C"
  - "Appendix C — React Ecosystem Borrowings"
tags:
  - architecture
  - appendix
type: architecture
---

# Appendix C — React Ecosystem Borrowings

| Borrow | Source | Libra form | Phase |
|---|---|---|---|
| Query cache | TanStack Query | `libra-query`: `Effect::Query { key, req, policy }` — dedup, stale-while-revalidate, retry/backoff, invalidation (`Effect::Invalidate(key)`); cache state arrives as actions; fully interceptable in integration tests | 2–3 |
| Async-state type | Suspense (via Elm's RemoteData) | `Remote<T, E> = NotAsked \| Loading \| Success(T) \| Failure(E)` in `libra-core`; markup `<match>` renders per-variant; makes loading/error impossible-states unrepresentable | 1 |
| Component gallery | Storybook | `libra gallery`: generated app rendering every catalog component × state × theme from the components' own snapshot fixtures; doubles as PR visual-review surface | 3 |
| Error boundaries | React error boundaries | `catch_unwind` at the Rust-component/Canvas paint boundary → error placeholder component + dev-overlay backtrace; interpreter errors already contained by design | 1 |
| Form state | React Hook Form | `FormState<T>` model helper: per-field dirty/touched/error, pure validators, submit gating — plain state machinery, unit-testable | 3 |
| Screens/routing | React Router | `Screen` enum + navigation-stack helper (push/pop/replace as actions); `libra://` deep-link protocol maps URLs → screen state (post-v0.1) | 3 |
| Mount-scoped effects | `useEffect` (lifecycle core) | Source elements `<Frames/>`/`<Interval/>`/`<Stream/>` ([[12.5-declarative-subscriptions|§12.5]]): declarations diffed on mount/unmount; results arrive as recorded actions — never code in the tree | 4 |
| Zero-boilerplate app entry | `create-react-app` / Vite scaffold ethos | `libra::app!` ([[05.8-app-assembly|§5.8]]): state + actions + one wiring declaration is a complete app | 4 |

Evaluated and rejected: Context (ambient hidden state), hooks as *code* in any form — state in the tree and effects executed at render stay excluded; the [[12.5-declarative-subscriptions|§12.5]] source elements are the data-shaped answer to `useEffect`'s legitimate core — concurrent rendering/transitions (damage tracking + the data plane already address the underlying contention), portals (the overlay layer in `libra-components` covers dialogs/toasts/menus), CSS-in-JS (tokens only).

---

← [[Architecture Home]] · [[Map of Content]]
