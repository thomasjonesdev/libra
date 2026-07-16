---
title: "11. Theming & Design Tokens"
section: "11"
aliases:
  - "§11"
  - "11"
  - "11. Theming & Design Tokens"
tags:
  - architecture
  - section/11
type: architecture
---

# 11. Theming & Design Tokens

- All component styling resolves through **named tokens**: `color.surface`, `color.accent`, `radius.md`, `space.3`, `type.mono.sm`, `elevation.panel`. No hex codes in components or markup (markup may reference tokens: `bg={color.warning.bg}`).
- Themes are token tables (TOML), hot-reloadable in dev, switchable at runtime (`Effect::SetTheme`); the renderer re-resolves live. Two ship built in: `libra-dark` (mission-control default) and `libra-light`.
- Semantic aliasing: `color.telemetry.nominal` → `color.green.400`, so domain themes re-map meaning without touching components.
- Density: a global scale token (`compact`/`normal`) multiplies the space ramp — ops screens want compact.
- Tokens are part of the determinism contract: a snapshot test's identity includes the theme hash.

**Implementation status:** a flat `Theme` struct + TOML override subset is
shipped (`libra-dark` byte-identical to legacy tokens). Full named ramps,
density, and semantic status colors are staged in [[PLAN-TOKENS]] (under
[[PLAN-UI-SURFACE]]). Catalog growth must not invent paint literals —
extend the token table instead.

---

← [[Architecture Home]] · [[Map of Content]]
