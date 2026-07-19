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

**Status: in progress — T1 landed 2026-07-16; T2/T3/T4 landed 2026-07-17.**
Makes [[11-theming|§11]] real enough that the expanded catalog
([[PLAN-CATALOG]]) can stay beautiful **and** themeable without
per-component forks. Umbrella: [[PLAN-UI-SURFACE]].

**T2/T4 (2026-07-17):** expanded the flat `Theme` to the full named set —
`text_inverse`, `border`, status `*_bg`/`*_fg` pairs, radius ramp
(`radius_sm/lg/full`), type ramp (`type_xs/lg`), 8-step `space` scale, control
heights, and a `Density` (compact/normal/comfortable) resolved via
`Theme::space`/`Theme::dense`. All TOML-overridable; `libra-dark`/`libra-light`
stayed byte-identical (new keys read only by post-T1 components). **T3
enforcement** shipped with [[PLAN-COMPONENTS]] C3: `libra-components`
`tests/tokens_only.rs` fails the build on any raw color literal under
`catalog/`. **Remaining:** T5 (markup token refs — a grammar change) and
`elevation.*` (blocked on a shadow draw command).

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
| **T1** ✅ | Internal `TokenTable` (or expanded `Theme`) with named lookup; `libra-dark`/`libra-light` fill all keys | Done 2026-07-16 |
| **T2** ✅ | TOML theme files gain the full key set (`space_*`, ramps, status pairs, `density`); unknown keys error with line numbers | Done 2026-07-17 (suggestions still open) |
| **T3** ✅ | Catalog paints use named tokens only — enforced by `tests/tokens_only.rs` ([[PLAN-COMPONENTS]] C3) | Done 2026-07-17; dark values identical |
| **T4** ✅ | `Density` multiplier (`Theme::space`/`dense`) + semantic status `*_bg`/`*_fg` tokens | Done 2026-07-17; consumed as Badge/Progress/Switch land |
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

← [[Plans Index]] · [[PLAN-UI-SURFACE]] · [[11-theming|§11]]
