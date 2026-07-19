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
shipped (`libra-dark` byte-identical to legacy tokens). [[PLAN-TOKENS]] T1
(2026-07-16) added the first named keys — `surface_raised`, `hairline`, and
semantic status colors. **T2/T4 (2026-07-17)** expanded the table to the full
named set — `text_inverse`, `border`, status `*_bg`/`*_fg` pairs, the radius
ramp (`radius_sm/md/lg/full`), the type ramp (`type_xs…xl`), an 8-step spacing
scale (`space_1…8`), control heights, and a **`density`** (compact/normal/
comfortable) resolved through `Theme::space`/`Theme::dense` — all overridable
in TOML, all keeping `libra-dark`/`libra-light` byte-identical (new keys are
read only by post-T1 components). The C3 guard (`libra-components`
`tests/tokens_only.rs`) enforces **tokens-only paint** in the catalog.
Remaining in [[PLAN-TOKENS]]: T5 markup token refs (`bg={color.surface}`, a
grammar change) and `elevation.*` (needs a shadow draw command first).

## In code

- `crates/libra-draw/src/lib.rs` — `Theme` (the named token table), `tokens`
  (canonical dark values), `Density`, `from_toml_str`
- `themes/` — TOML theme tables (e.g. `themes/solarized.toml`)
- `crates/libra-components/tests/tokens_only.rs` — the tokens-only guard (T3)

---

← [[Architecture Home]] · [[Map of Content]]
