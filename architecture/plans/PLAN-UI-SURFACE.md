---
title: "PLAN-UI-SURFACE"
aliases:
  - "PLAN-UI-SURFACE"
  - "UI surface"
tags:
  - plans
  - architecture
type: plan
---

# UI surface — catalog, tokens, native integrations

**Status: planned (documentation).** Umbrella for expanding Libra's
built-in UI toward the breadth of Vercel's **json-render React Native**
standard catalog (`@json-render/react-native`), while keeping Libra's
contracts: closed grammar, token-only styling, deterministic paint, and
effects-as-data for OS work.

**Mirror source (external):**
[vercel-labs/json-render](https://github.com/vercel-labs/json-render)
`packages/react-native` — `standardComponentDefinitions` +
`standardActionDefinitions`. Libra is **desktop-native (winit)**, not
React Native; we mirror *capability and catalog shape*, not RN APIs.

**How to use this set**

1. Land [[PLAN-TOKENS]] early enough that new components never hard-code
   hex/metrics — beauty comes from tokens, not one-off paint.
2. Implement catalog entries **one component at a time** from
   [[PLAN-CATALOG]] (gallery + snapshots before the next).
3. Land [[PLAN-NATIVE-INTEGRATIONS]] when a component or example needs
   OS chrome (alert, share, open URL, safe insets) — never as silent
   I/O inside `update`.

| Plan | Owns |
|---|---|
| [[PLAN-TOKENS]] | Design-token tables, semantic aliases, density, markup token refs |
| [[PLAN-CATALOG]] | Full component inventory + beauty-first build order |
| [[PLAN-NATIVE-INTEGRATIONS]] | Platform effects & shell glue (Alert/Share/OpenUrl/…) |
| [[PLAN-TEXTAREA]] | Multi-line text / IME / touch (already separate; §10.1) |

**Non-goals here:** implementing any of the above; AI chat elements
(Vercel AI Elements) — different product surface; track separately if
ever needed.

---

← [[Plans Index]] · [[07.1-the-built-in-catalog-closed-curated|§7.1]] · [[11-theming-and-design-tokens|§11]] · [[19-platform-integration|§19]]
