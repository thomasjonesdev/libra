---
title: "PLAN-TOKENS"
aliases:
  - "PLAN-TOKENS"
  - "design tokens plan"
tags:
  - plans
  - architecture
type: plan
---

# Design tokens — customizable by default

**Status: in progress — T1 landed 2026-07-16 (T2 partially).** Makes
[[11-theming-and-design-tokens|§11]] real enough that the expanded catalog
([[PLAN-CATALOG]]) can stay beautiful **and** themeable without
per-component forks. Umbrella: [[PLAN-UI-SURFACE]].

**T1 decision:** extended the existing flat `Theme` struct with named
fields (`surface_raised`, `hairline`, `danger`/`warning`/`success`/`info`)
rather than introducing a separate `TokenTable` — one lookup path, TOML
override + hot reload keep working unchanged, and dark stayed
byte-identical because new keys are only read by post-T1 components
(`Separator` is the first consumer). The TOML parser gained the new keys
(T2's key-set growth; unknown keys already error with line numbers —
suggestions still open). Revisit `TokenTable` only if/when markup token
refs (T5) need string-keyed lookup.

## Why now

Today `libra_draw::Theme` is a **flat** struct (bg/surface/text/accent/…
radius/type/button padding). That shipped Phase 3 theming and kept
`libra-dark` byte-identical to old snapshots — keep that invariant.

The json-render RN catalog (and any serious design system) expects
**named ramps**: surfaces, semantic status colors, spacing scale, type
scale, radii, elevation, control sizes. §11 already describes this;
implementation has not caught up.

## Target shape (normative intent → §11)

```
color.bg / color.surface / color.surface.raised
color.text / color.text.muted / color.text.inverse
color.accent / color.accent.text
color.border / color.selection
color.danger / color.warning / color.success / color.info   (+ .bg / .fg)
space.1 … space.8                                          (density-scaled)
radius.sm / radius.md / radius.lg / radius.full
type.xs … type.xl (+ type.mono.*)
elevation.none / elevation.panel / elevation.modal
control.height.sm|md|lg / control.pad.x|y
```

- **No hex in components or `.lx`.** Markup may eventually bind
  `bg={color.warning.bg}` (grammar follow-on); until then components
  resolve names in Rust via `Theme`.
- **Semantic aliases:** `color.telemetry.nominal → color.success` so
  domain themes remap meaning without touching catalog code.
- **Density:** global `compact | normal | comfortable` multiplies
  `space.*` and control metrics.
- **Snapshots** include theme identity (name + hash of token table).

## Stages

| Stage | Deliverable | Notes |
|---|---|---|
| **T0** | Document token names in §11 + this plan; map current flat fields → names | No byte change |
| **T1** | Internal `TokenTable` (or expanded `Theme`) with named lookup; `libra-dark`/`libra-light` fill all keys | Components still use today's fields via aliases |
| **T2** | TOML theme files gain the full key set; unknown keys error with suggestions | Hot reload already exists for files |
| **T3** | Migrate Button/Text/Input/Chart paints to named tokens only | Gallery re-bless if needed; dark values stay identical |
| **T4** | Density multiplier + semantic status tokens used by Badge/Progress/Switch | Unblocks catalog waves 2–3 |
| **T5** | Optional markup token refs (`bg={color.surface}`) — closed ident path, not expressions | Dual-engine + conformance |

## Customization contract

| Audience | Mechanism |
|---|---|
| App author | `themes/*.toml` + `Options::theme_file` / `Effect::SetTheme` |
| Catalog author | Resolve `theme.token("color.accent")` — never literals |
| Snapshot tests | `.theme("libra-dark")` (default); matrix for light |
| Designers | Edit TOML; hot reload in shell |

## Invariants

1. **`Theme::dark()` / `libra-dark` values for existing keys stay
   bit-identical** until an intentional bless of the whole snapshot set.
2. Tokens are data — switching themes is an effect / harness option, not
   ambient process env on the paint path.
3. Missing token at paint = bug (debug assert / overlay), not silent
   fallback to magenta — prefer load-time theme validation.

## Out of scope

- Runtime CSS / Tailwind class strings
- Per-instance style objects that bypass tokens
- Animated token interpolation (later)

---

← [[Plans Index]] · [[PLAN-UI-SURFACE]] · [[11-theming-and-design-tokens|§11]]
