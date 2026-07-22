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

## Design direction: Libra UI (Geist-inspired) (2026-07-21)

**The output is *Libra UI* — Libra's own design system.** Vercel's **Geist** is
**inspiration and guidelines, not the target**: we borrow its sensibility
(minimal, high-contrast, restrained color that signals state/hierarchy rather
than decorating) and use its scale structure as a *reference* for our own tokens.
Libra UI is free to deviate wherever Libra's contracts or product (ops/mission-
control, determinism, closed grammar) call for it — the deviations below are
first-class, not compromises. This is **normative design intent**; the code
migration (font swap + palette remap + whole-suite re-validation) is staged in
[[PLAN-GEIST]]. Where we do lean on Geist for concrete numbers, its published
values (`/design.md` light, `/design.dark.md` dark) are the reference to pin from
at implementation time — a starting point for Libra UI's palette, not a spec to
match byte-for-byte.

### Palette (flat `Theme` field → Geist scale step, as reference)

Geist's scale is a useful *reference structure* Libra UI borrows: a 10-scale ×
10-step system (`gray`, `gray-alpha`, `blue`, `red`, `amber`, `green`, `teal`,
`purple`, `pink`, plus a two-value `background` scale) where each step encodes
**intent**, not just lightness (`100` default bg → `400` default border → `700`
solid fill → `900` secondary text → `1000` primary text). Libra UI keeps its
flat `Theme` table and maps each field onto the equivalent step — a starting
point Libra UI tunes to taste:

| Libra `Theme` field | Geist step (reference) | Role |
|---|---|---|
| `bg` | `background-100` | Primary page/card surface |
| `surface` | `background-100` | Component surface (cards use bg) |
| `surface_raised` | `gray-100` | Raised/hover component bg (Color 1) |
| `secondary` | `gray-100` | Secondary control fill |
| `text` | `gray-1000` | Primary text/icons |
| `text_muted` | `gray-900` | Secondary text/icons (disabled → `gray-700`) |
| `text_inverse` | `background-100` | Label on solid fills (primary button) |
| `accent` | `blue-700` | Links, focus, primary action tint (light `#006bff`, dark `blue-900` `#47a8ff`) |
| `accent_text` | `background-100` | Text on the accent fill |
| `border` / `hairline` | `gray-alpha-400` | Default border/divider (translucent — layers over any bg) |
| `selection` | `blue-200` | Text selection wash |
| `scrim` | `gray-alpha` overlay | Modal/drawer backdrop |
| `danger` (+`_bg`/`_fg`) | `red-700` (+`red-200`/`red-1000`) | Errors; error button = `red-800` |
| `warning` (+`_bg`/`_fg`) | `amber-700` (+`amber-200`/`amber-1000`) | Warnings |
| `success` (+`_bg`/`_fg`) | `green-700` (+`green-200`/`green-1000`) | Success. *Geist's convention uses `blue` for success/links; Libra keeps a distinct green for state legibility — a deliberate deviation* |
| `info` (+`_bg`/`_fg`) | `blue-700` (+`blue-200`/`blue-1000`) | Informational |

Hover/active step the scale up (`100`→`200`→`300` fills; `400`→`500`→`600`
borders) — Libra's paint-side hover/selection washes ([[PLAN-NOTES-APP]] M8)
follow this by reading `surface_raised`/`selection`. Wide-gamut Display-P3
variants (Geist's `*-p3` `oklch()` values) are **out of scope**: the CPU raster
path is sRGB-only and byte-deterministic (Appendix A).

### Shape, spacing, elevation

- **Radii** (tight, one family per view): `radius_sm`/`radius_md` = **6px**
  (everyday surfaces + controls), `radius_lg` = **12px** (menus/modals),
  a fullscreen **16px** step, `radius_full` = **9999px** (pills/avatars).
- **Spacing** — Geist's 4px scale (4, 8, 12, 16, 24, 32, 40, 64, 96) maps onto
  the 8-step `space_1…8` ramp; three-step rhythm (8 inside a group, 16 between
  groups, 32–40 between sections); cards pad 24 (16 compact, 32 hero).
- **Elevation** — Geist leans on tonal surfaces + borders first, shadows subtle.
  `elevation.*` stays **blocked** on a shadow `DrawCmd` (see [[PLAN-TOKENS]]);
  until it lands, depth comes from `surface_raised` + `hairline`.
- **Motion** — Geist easing `cubic-bezier(0.175, 0.885, 0.32, 1.1)`, ~150/200/300ms;
  captured here as intent only — animated tokens are §9.6 (unbuilt), and
  snapshots evaluate at target values regardless.

### Typography — Work Sans + Space Mono

Libra diverges from Geist Sans/Mono to two open (OFL) families:

- **Work Sans** — the sans family for all UI and prose (replaces Liberation Sans).
  Renderer bundles **Medium (500)** as [`FontStyle::Regular`] and **Bold (700)** as
  [`FontStyle::Bold`] — the closed `FontStyle` set. Medium reads closer to Geist's
  button weight than Regular 400.
- **Space Mono** — the monospaced family for code/data/tabular figures, wired to
  the future `type.mono.*` tokens. Inert until a mono-consuming component or type
  token lands (no component uses mono today).

Type ramp follows Geist's metrics with `copy-14`/`label-14` (**14px**) as the
common base: `type_xs` 12 · `type_sm` 13 · `type_md` 14 · `type_lg` 16 ·
`type_xl` headings. Fonts stay **fontdue advance-width layout** (no shaping/
kerning — §9, README spike note); Work Sans/Space Mono lay out the same way
Liberation Sans does today.

**Determinism note:** fonts are on the paint path (bundled via `include_bytes!`
in `libra-raster`). The 2026-07-21 [[PLAN-GEIST]] migration re-baselined every
committed snapshot deliberately (`LIBRA_VALIDATE=1`).

**Implementation status:** Libra UI palette + Work Sans / Space Mono shipped
([[PLAN-GEIST]] G1–G3, 2026-07-21). Flat `Theme` struct + TOML override subset;
[[PLAN-TOKENS]] T1–T4 named keys (`surface_raised`, `hairline`, status `*_bg`/`*_fg`,
radius/type/space ramps, `density`). The C3 guard (`libra-components`
`tests/tokens_only.rs`) enforces **tokens-only paint** in the catalog.
Remaining in [[PLAN-TOKENS]]: T5 markup token refs (`bg={color.surface}`, a
grammar change) and `elevation.*` (needs a shadow draw command first).
Space Mono wiring to `type.mono.*` is [[PLAN-GEIST]] G4 (when a mono consumer lands).

## In code

- `crates/libra-draw/src/lib.rs` — `Theme` (the named token table), `tokens`
  (canonical dark values), `Density`, `from_toml_str`
- `crates/libra-raster/src/lib.rs` — bundled fonts (`include_bytes!`); the
  Geist swap re-points these at Work Sans / Space Mono ([[PLAN-GEIST]])
- `assets/fonts/` — Work Sans (Medium + Bold), Space Mono (Regular + Bold)
- `themes/` — TOML theme tables (e.g. `themes/solarized.toml`)
- `crates/libra-components/tests/tokens_only.rs` — the tokens-only guard (T3)

---

← [[Architecture Home]] · [[Map of Content]]
