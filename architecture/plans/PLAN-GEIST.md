---
title: "PLAN-GEIST"
aliases:
  - "PLAN-GEIST"
  - "Geist retheme"
tags:
  - plans
  - architecture
type: plan
---

# Libra UI palette + Work Sans / Space Mono typography (Geist-inspired)

**Status: complete — G1–G3 landed 2026-07-21.** Establish **Libra UI** — Libra's own design system — with a
minimal, high-contrast, restrained-color sensibility **inspired by** Vercel's
**Geist** (guidelines, not a target), and swap the bundled typefaces to
**Work Sans** (sans) and **Space Mono** (mono). The output is Libra UI; Geist is a
reference we tune away from wherever Libra's contracts or product call for it.
Normative design intent lives in [[11-theming|§11]] "Design direction: Libra UI
(Geist-inspired)"; this plan is the staged, determinism-safe execution. Umbrella:
[[PLAN-UI-SURFACE]] / [[PLAN-TOKENS]].

**Reference (external):** Geist `/design.md` (light) and `/design.dark.md`
(dark) — [vercel.com/geist](https://vercel.com/geist/introduction). Where Libra UI
leans on Geist for concrete numbers, per-step hex is pinned from Geist's raw
values (right-click-copy) **at code time** as a starting point — not transcribed
here and not a byte-for-byte spec to match.

**Read before writing code:** [[11-theming|§11]], [[09-rendering|§9]],
[[appendix-a-determinism-checklist|Appendix A]], [[PLAN-TOKENS]] (Invariant 1 — the
byte-identical rule this plan deliberately supersedes once).

## The one deliberate invariant break

[[PLAN-TOKENS]] Invariant 1 holds `Theme::dark()` / `libra-dark` **byte-identical**
to the Phase 0 tokens so no snapshot moves by accident. This plan is the **single
sanctioned exception**: fonts are on the paint path (`include_bytes!` in
`libra-raster`), so changing families re-renders **every glyph in the repo**, and
the Geist palette remaps the flat `Theme`. Both ripple through **every committed
snapshot** plus the counter README hashes. Do it **once, deliberately**, with
`LIBRA_VALIDATE=1 cargo test --workspace` and a README-hash update in the same change.

## Scope decisions (resolved 2026-07-21)

- **In-place retheme** of the built-in `libra-dark` / `libra-light` — Libra UI
  becomes *the* built-in look (not an additive `geist-*` theme pair; the themes
  stay named `libra-*`, since the deliverable is Libra UI, not a Geist clone).
- **Two UI weights only**: Work Sans **Medium (500)** as [`FontStyle::Regular`] + **Bold (700)** — the
  closed UI pair; matches Geist's "≤ two weights per view". [`FontStyle::Italic`] is markdown
  emphasis only (Work Sans Italic), not a third UI weight. *(Regular 400 was not bundled;
  Medium reads closer to Geist button weight.)*
- **sRGB only**: Geist's `*-p3` `oklch()` wide-gamut variants are out of scope — the
  CPU raster path is sRGB and byte-deterministic (Appendix A).
- **Flat `Theme` kept**: fields map onto Geist scale steps (table in [[11-theming|§11]]);
  no `TokenTable` restructure (revisit only if T5 string-keyed lookup needs it).

## Stages

Each stage lands green (`cargo test --workspace`) and updates this status line.

| Stage | Deliverable | Notes |
|---|---|---|
| **G1 — done** | `WorkSans-Medium.ttf` / `WorkSans-Bold.ttf` / `WorkSans-Italic.ttf` + `SpaceMono-Regular.ttf` under `assets/fonts/`; `libra-raster` repointed from Liberation Sans | fontdue advance-layout unchanged; every text snapshot re-validated |
| **G2 — done** | `Theme::dark()` + `Theme::light()` remapped to Libra UI (Geist-inspired) hex; radii 4/6/12; Geist 4px spacing scale; control heights 32/40/48 | Flat `Theme` shape unchanged |
| **G3 — done** | `LIBRA_VALIDATE=1 cargo test --workspace`; whole snapshot set re-recorded; counter README hashes updated | Deliberate whole-suite re-baseline |
| **G4 — mono wiring** *(optional/later)* | Wire Space Mono to `type.mono.*` when a mono-consuming component or type token first needs it (tabular figures, code/log views). Inert until then | Ties [[PLAN-CATALOG]] LogView/Table |
| **G5 — elevation/motion** *(deferred)* | Geist shadows + easing once a shadow `DrawCmd` (blocked, [[PLAN-TOKENS]]) and §9.6 animated tokens exist | Not this plan's critical path |

## Invariants

1. **Determinism holds** — fonts stay **bundled** (never system fonts); PNG output
   remains byte-identical across machines. The bless is a deliberate re-baseline, not
   a loosening of the contract (Appendix A).
2. **fontdue advance-layout only** — no shaping/kerning; Work Sans / Space Mono lay
   out exactly as Liberation Sans does today (§9, README spike note).
3. **Closed `FontStyle`** — Regular + Bold for UI; Italic is markdown emphasis only. A new UI weight would be a new closed-set
   member with its own bundle + measurer arm, not an ad-hoc lookup.
4. **Tokens-only paint survives** — no component gains a raw literal; the C3 guard
   (`tests/tokens_only.rs`) stays green.
5. **Licensing** — Work Sans (OFL) + Space Mono (OFL); license text committed under
   `assets/fonts/`.

## When this change lands (checklist — [[AGENTS|AGENTS §9]])

1. [[11-theming|§11]] "Design direction: Geist" matches the shipped values.
2. Flip this plan + [[Plans Index]] to the reached status; note the retheme in
   [[AGENTS|AGENTS §8]] "built vs remaining".
3. Update **counter README hashes** + the snapshot-truths block (only counter is
   hash-pinned; other apps' PNGs are committed but not hashed).
4. Gates: `cargo test --workspace` (all snapshots re-validated), conformance
   (grammar untouched → should stay green), `tokens_only`.

## Out of scope

- Display-P3 / wide-gamut color (sRGB deterministic path only)
- Elevation shadows + motion (blocked on shadow `DrawCmd` / §9.6 — [[PLAN-TOKENS]])
- Markup token refs `bg={color.surface}` — that is [[PLAN-TOKENS]] T5
- A separate `geist-*` theme pair (this plan reskins the built-ins in place)
- Font shaping/kerning/variable-font axes (fontdue advance-layout stays)

---

← [[Plans Index]] · [[PLAN-UI-SURFACE]] · [[PLAN-TOKENS]] · [[11-theming|§11]]
