---
title: "PLAN-SCROLL"
aliases:
  - "PLAN-SCROLL"
  - "scroll"
  - "clipping"
type: plan
tags:
  - plans
  - architecture
---

# Scroll (and the clip primitive it needs)

**Status: shipped — 2026-07-18 (S1–S4).** A `<Scroll>` viewport that clips its
content and offsets it by a scroll position. Immediate need: the workbench
([[PLAN-GALLERY]]) sidebar is taller than the window. Ties:
[[07.1-builtin-catalog|§7.1]] (the component), [[09.1-display-list|§9.1]] /
[[09.2-canonical-executor|§9.2]] (the clip primitive), [[19-platform-integration|§19]]
(wheel), [[appendix-a-determinism-checklist|Appendix A]] (the paint path).

## Where it fits (three pieces, three layers)

Scroll isn't one thing — it lands cleanly once split across the layers it
actually touches:

| Piece | Layer | Why there |
|---|---|---|
| **Clip primitive** `DrawCmd::PushClip{rect}` / `PopClip` | display list ([[09.1-display-list|§9.1]]) + raster ([[09.2-canonical-executor|§9.2]]) | You can't scroll without clipping the overflow; today the display list has no clip. A rect clip is deterministic (same bytes), so it's contract-safe. |
| **`<Scroll>` component** | catalog ([[07.1-builtin-catalog|§7.1]]) | A viewport of fixed/grown height whose content lays out at natural size and paints offset + clipped. |
| **Scroll offset** | a **deterministic paint/layout input**, like focus/hover | It is *view* state, not app State — the same category as `InputFocus` and `hover`. The shell owns it (fed by wheel, §19); the harness feeds it explicitly; default `0`, so snapshots are unaffected. **Not** on `State` (that would put view scrolling in the action log for no reason). |

The guiding rule: **scroll position is an input to paint, exactly like hover and
focus** — deterministic, shell-provided live, harness-provided in tests. The
canonical frame (snapshot) is always at offset 0.

## Design

- **Clip** (`DrawCmd::PushClip{rect}` / `PopClip`): the raster keeps a clip
  stack; the active clip is the intersection. tiny-skia fills take a rect
  `Mask`; the manual blits (text, strips, glyph coverage) gain a clip-rect
  bounds check. `render_damage` applies the same clip during replay, so the
  **byte-equivalence property test stays green** — a clip only restricts which
  pixels are written, identically in both paint paths.
- **`<Scroll>`**: `ViewNode::Scroll { height?, test_id }` wrapping one content
  child. Build makes a fixed-height (or grown) viewport node; the content lays
  out at its natural height inside (taffy `overflow`). `flatten` applies
  `(0, -scroll_y)` to the content subtree's rects and tags each descendant
  `Item` with the viewport clip rect; paint emits `PushClip(viewport)` around
  the scrolled items. `scroll_y` is clamped to `[0, content_h − viewport_h]`.
- **Offset input**: `LaidTree.paint*` already threads `focus` and `hover`; add a
  `scroll: &ScrollState` (test_id → offset px). The shell updates it from
  `MouseWheel` over a scroll viewport and `request_redraw`s; the harness gets
  `h.scroll(by::test_id(id), dy)`; default empty → offset 0.
- **Semantics**: the viewport is a `Role::Container` (scrollable); content items
  keep their own roles. A dedicated scrollable a11y affordance is later polish.

## Stages

| Stage | Deliverable |
|---|---|
| **S1** ✅ | Clip primitive: `DrawCmd::PushClip`/`PopClip` + raster clip stack (path fills via `Mask`, text/strips via bounds check) shared by `render` + `render_damage`; clip-confines + **byte-equivalence-with-clips** tests green; HiDPI scaler handles clips. *(2026-07-18)* |
| **S2** ✅ | `<Scroll testId height?>` component (node + build + `flatten` clip/scroll tags + `LaidTree::apply_scroll` + paint/hit clip), both engines + conformance. The flexbox `min-height: 0` reset in `box_style` was required so a definite outer height constrains the viewport (else content inflates the whole chain); `taffy` `overflow.y: Hidden` stops the viewport growing to its content. |
| **S3** ✅ | Scroll offset as a deterministic input: `ScrollState` (by test_id); harness `scroll(id, dy)`; shell routes `MouseWheel` to the viewport under the cursor + re-lays-out. |
| **S4** ✅ | Workbench sidebar wrapped in `<Scroll testId="sidebar">`; a short-viewport test verifies it clips + offsets + round-trips (byte-exact scrolled snapshot). |

## Invariants

1. **Determinism** — clip is a pure rect operation; `render_damage` clips
   identically to `render`, so `damage_tracked_render_is_byte_equivalent_to_full`
   stays green (Appendix A). Snapshots render at scroll offset 0.
2. **Scroll is an input, not State** — no scroll action, no entry in the action
   log; the shell/harness own it like focus/hover.
3. **Closed grammar** — `<Scroll>` is a normal catalog element; its content is
   ordinary markup. No new expression power.
4. **Both engines + conformance** for the grammar addition (§6.4).

## Out of scope (v1)

- Horizontal scroll (design is symmetric; add when needed), scrollbars (a paint
  affordance later), momentum/rubber-band, nested-scroll chaining, scroll-into-view.

## In code

- `crates/libra-draw/src/lib.rs` — `DrawCmd::PushClip`/`PopClip`
- `crates/libra-raster/src/lib.rs` — clip stack in `render` / `render_damage`
- `crates/libra-components/src/catalog/scroll.rs` — the component; `layout.rs` `flatten` (offset + clip tag)
- `crates/libra-platform/src/lib.rs` — `MouseWheel` → offsets; `crates/libra-test/src/lib.rs` — `scroll()`

---

← [[Plans Index]] · [[07.1-builtin-catalog|§7.1]] · [[09.1-display-list|§9.1]] · [[PLAN-GALLERY]]
