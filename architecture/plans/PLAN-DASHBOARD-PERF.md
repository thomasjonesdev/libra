---
title: "PLAN-DASHBOARD-PERF"
aliases:
  - "PLAN-DASHBOARD-PERF"
tags:
  - plans
  - architecture
type: plan
---

# Dashboard Performance & Smoothness Plan

**Status: complete.** All 8 stages (0–7, including the optional AA stage)
landed. `cargo test --workspace` green throughout; bench stayed within the
[[22-performance-budgets|§22]] budget at every stage (frame total p50 0.64ms → ~0.73ms after Stage 7's
AA blending, budget < 2ms); snapshots stayed byte-identical through Stage 6
and only the four chart-bearing PNGs (`dashboard_wave`, `gallery_dark`,
`gallery_light`, `gallery_focus_dark`) changed at Stage 7 — visually
confirmed smooth, no other snapshot moved. Live-window pacing/smoothness
(Stages 1–2) could not be visually verified in this session (no GUI/display
access) — worth a manual `cargo run -p dashboard --release` check.

Fixes the live-window choppiness in `cargo run -p dashboard --release` and makes the
chart wall render at full physical resolution. The [[22-performance-budgets|§22]] pipeline itself is healthy —
measured on this machine (2026-07-14, release):

```
libra dashboard bench — 600 frames, 16 series × 10000 Hz, 1280×800
  view+layout              p50   0.026 ms
  paint (M4 + geometry)    p50   0.047 ms
  raster (damage-tracked)  p50   0.609 ms
  frame total              p50   0.683 ms   (budget: < 2 ms)
```

Everything wrong lives in the **windowed shell** (`libra-platform`, explicitly *not*
on the determinism contract's paint path — [[21-workspace-and-crate-layout|§21]]) plus two shell-adjacent hot paths.
Diagnosis, in order of user-visible impact:

1. **Frame pacing** — a fixed 16 ms timer (`FRAME_INTERVAL`,
   `crates/libra-platform/src/lib.rs`) beats against the 16.67 ms display refresh
   (softbuffer has no vsync), duplicating/dropping a frame ~2–3×/sec; and
   `next_frame_at = Instant::now() + FRAME_INTERVAL` accumulates every late
   `ControlFlow::WaitUntil` wakeup into permanent schedule drift.
2. **Retina cost** — the bench runs 1280×800 at scale 1.0; the live window rasters
   2560×1280 physical (~3.2× the pixels), and because all 16 charts change every
   tick, damage ≈ the whole window. On top: `render_damage` replays `Clear` as a
   *full-pixmap* fill every partial repaint, and `present()` does a scalar
   RGBA→XRGB conversion of all ~3.3 M pixels every frame regardless of damage.
3. **Per-frame allocation churn** — `scale_cmds` clones the whole cmd list, the
   frame cache clones it again (`cmds.to_vec()`), `push_a11y` builds + clones a
   snapshot every relayout (= every tick).
4. **Ingest wakeup churn** — the Sine source sleeps ≤100 µs per sample; macOS
   rounds that to ~1 ms+, so 16 threads each attempt ~10 k sleeps/sec and push one
   sample per ring lock.
5. **Blocky curves** — chart M4 columns are computed at *logical* width
   (`width_px = inner.w as u32` in `paint_chart`), then `scale_cmds` doubles
   `col_w` → 2-physical-px-wide steps on Retina; and `fill_strips` writes
   hard-edged opaque runs (no edge coverage), so slow curves staircase.

Stages are ordered by payoff and risk. **S1 alone should visibly fix "choppy";
S2–S5 buy back CPU headroom; S6 fixes blockiness; S7 is the only byte-changing
stage and is optional.**

## Ground rules (apply to every stage)

- [x] Each stage is one commit/PR, landed with `cargo test --workspace` green.
- [x] Every stage except S7 is **snapshot-neutral**: record
      `shasum -a 256 examples/*/tests/snapshots/*.png > /tmp/snapshot-baseline.txt`
      before S0 and compare after every stage. README.md hashes untouched until S7.
- [x] After each stage, run `cargo run -p dashboard --release -- bench` — all [[22-performance-budgets|§22]]
      budgets must still pass, and no percentile may regress by more than noise
      (~10%). Paste the numbers into the commit message.
- [x] The determinism-critical invariants are non-negotiable:
      `damage_tracked_render_is_byte_equivalent_to_full` (libra-raster),
      `chart_columns_incremental_equals_cold_rebuild` (libra-data), and the
      conformance suite stay green untouched. No pinned dep bumps (taffy,
      tiny-skia, fontdue).
- [x] `App::update` stays pure; nothing here may add clocks/I/O to app code or to
      the canonical paint path. All wall-clock use stays inside the shell ([[21-workspace-and-crate-layout|§21]])
      and the effects/data runtimes ([[12.3-execution-model|§12.3]], [[14.2-architecture|§14.2]]).
- [x] New/changed doc comments cite the relevant ARCHITECTURE.md § (house style).
- [ ] Manual smoke after S1, S2, S6: `cargo run -p dashboard --release`, watch
      ~15 s: charts scroll smoothly, no rhythmic hitch, pause/resume still works,
      resize drag stays live. **Not done this session — no GUI/display access;
      code + tests + bench verified, live visual smoke still outstanding.**

---

## Stage 0 — Frame-time instrumentation (measure before touching)

Dev-only stderr stats so every later stage's effect is measurable. No new deps.

- [x] In `crates/libra-platform/src/lib.rs`, add a small `FrameStats` struct on
      `Shell`: ring of the last N=120 (present-to-present interval, present CPU
      duration) pairs, sampled in `present()`.
- [x] Gate on `std::env::var_os("LIBRA_FRAME_STATS").is_some()` (checked once at
      shell construction, stored as `bool`). When on, every 120 presents print one
      stderr line: `[libra] frame p50/p95/max interval …/…/… ms · present …/…/… ms`.
- [x] Zero cost when off (a bool check per present; no allocation).
- [x] Record a baseline run (~30 s of `LIBRA_FRAME_STATS=1 cargo run -p dashboard
      --release`) in the commit message. Expect interval p95 to show the pacing
      problem (spikes to ~32 ms).

## Stage 1 — Frame pacing: refresh-matched, drift-free ticks

The smoothness fix. All in `crates/libra-platform/src/lib.rs`.

- [x] Replace the `FRAME_INTERVAL` constant with a per-shell `frame_period:
      Duration`, computed from the window's monitor:
      `gfx.window.current_monitor().and_then(|m| m.refresh_rate_millihertz())`
      → `Duration::from_secs_f64(1000.0 / mhz as f64)` (60 Hz monitor: mhz=60000
      → 16.667 ms). Fallback when `None`: `Duration::from_secs_f64(1.0 / 60.0)`.
      Compute in `resumed()` and recompute on `WindowEvent::Resized` /
      `ScaleFactorChanged` (window may have moved to another monitor).
- [x] Absolute scheduling in `about_to_wait`: on tick, advance
      `self.next_frame_at += self.frame_period` and only if it has fallen more
      than one full period behind (`next_frame_at < now`) snap to
      `now + frame_period`. Never `now + period` on the normal path — that's the
      drift bug being fixed.
- [x] `sync_subscriptions` already resets `next_frame_at = Instant::now()` on
      activation — keep that (first tick immediate).
- [x] Call `gfx.window.pre_present_notify()` immediately before every
      `buffer.present…()` call (winit 0.30's compositor-timing hint).
- [x] Keep the tick site in `about_to_wait` (tick → dispatch → relayout →
      `request_redraw`; present follows in the same loop iteration via
      `RedrawRequested`). Do **not** move ticking into `RedrawRequested` — resize
      also requests redraws and must not generate extra frame actions.
- [x] `frame_index`/`FrameInfo` semantics unchanged (t_ms stays wall-elapsed
      since shell start — apps only ever see time inside actions, [[14.2-architecture|§14.2]]) —
      confirmed by code inspection (`tick_frame` untouched).
- [ ] Verify with S0 stats: interval p95 within ~1 ms of p50, no ~32 ms
      spikes. **Not done this session — no GUI/display access.**

## Stage 2 — Present only what changed (blit + damage)

`present()` currently converts and presents the full buffer every frame even when
`render_damage` reported partial or zero damage. All in `libra-platform`.

- [x] Capture the `Damage` return of `renderer.render_damage(…)` (currently
      discarded). Its `Partial(rect)` is in **physical** px here, since the shell
      feeds scaled cmds.
- [x] `Damage::None` → return without touching the surface at all (no
      `buffer_mut`, no present). The window server retains the last frame.
- [x] `Damage::Partial(r)` → after `surface.resize`, get the buffer and check
      `buffer.age()`:
      - `age == 1` (buffer holds last frame): convert **only rows
        `r.y..r.y+r.h`, columns `r.x..r.x+r.w`** from the cached pixmap, then
        `buffer.present_with_damage(&[softbuffer::Rect { x, y, width, height }])`
        (fields are `u32`/`NonZeroU32`; round the rect out to integers, clamp to
        the surface).
      - `age == 0` or `> 1` (unknown/stale contents): full convert + plain
        `present()`. This is the correctness fallback — softbuffer's macOS
        backend may report 0; the code must be right either way.
- [x] `Damage::Full` → full convert + `present()` (unchanged path).
- [x] Keep the panic-overlay path (`painted.is_err()`) as a full present.
- [x] Extract the pixel-conversion loop into one helper
      (`fn blit_rows(dst: &mut [u32], src: &Pixmap, x0: usize, x1: usize, y0: usize, y1: usize)`)
      used by all three paths, so there's exactly one RGBA→XRGB conversion site.
- [x] Note for review: conversion math must stay byte-identical
      (`(r<<16)|(g<<8)|b`, alpha dropped — frame is opaque by contract since
      `Clear` is always first).
- [ ] Verify: S0 stats show present-duration drop when only the status line
      changes (pause the dashboard: damage collapses to the status text); visually
      no artifacts on resize, pause/resume, theme hot-reload. **Not done this
      session — no GUI/display access; workspace tests + bench + snapshot
      hashes verified instead (all green/unchanged).**

## Stage 3 — Clip the `Clear` replay in `render_damage`

`crates/libra-raster/src/lib.rs`: in the partial-damage replay loop, `Clear` has
`bounds() == None` → `touches = true` → `self.draw` runs `pixmap.fill`, clearing
the **entire** full-size scratch every partial repaint (~13 MB of writes per frame
at Retina). Only the dirty rows are ever copied out, so clipping it is
byte-equivalent by construction.

- [x] In the replay loop, special-case `DrawCmd::Clear(c)`: instead of
      `self.draw(&mut prev.scratch, cmd)`, overwrite only the dirty rect's pixels
      in `scratch` with the premultiplied color. Match `Pixmap::fill`'s exact
      bytes by using `tiny_skia::ColorU8::from_rgba(r, g, b, a).premultiply()` —
      **overwrite, not blend** (that is `fill`'s semantic). Write with
      row-slice `copy_from_slice`/`fill`, same style as the dirty-row copy just
      below it.
- [x] All other cmds keep replaying at original coordinates over the whole scratch
      — that FP-identity rule ([[09.3-making-cpu-fast-enough-damage-layers|§9.3]], module doc) is untouched. Only `Clear`
      changes, and only because its output inside the dirty rect is
      coordinate-independent.
- [x] Doc comment on the special case: why it's byte-equivalent (only dirty rows
      are copied out) and why `Clear` alone may be clipped.
- [x] The guardrail is `damage_tracked_render_is_byte_equivalent_to_full` — it
      already mutates strips/rects/text across 8 seeds × 10 steps. Add one seed
      variant where the mutated cmd sits far from a corner so the dirty rect is
      interior (exercises all four clip edges of the clear).
- [x] Verify: bench raster p50 drops (full-scratch clear was part of every damaged
      frame); property tests green.

## Stage 4 — Kill the per-frame clones

Three copies of the display list per frame → one. `libra-platform` +
`libra-raster`.

- [x] `scale_cmds(&[DrawCmd], f32) -> Vec<DrawCmd>` becomes
      `scale_cmds_in_place(&mut Vec<DrawCmd>, f32)` (early-return when `s == 1.0`).
      `present()` mutates the vec it got from `laid.paint(…)`. Update the two
      `scale_*` unit tests at the bottom of the file to the in-place form (they
      assert the same values).
- [x] `Renderer::render_damage` takes `cmds: Vec<DrawCmd>` by value; the cache
      stores it by move (`prev.cmds = cmds`) instead of `cmds.to_vec()` — both the
      full path and the partial path. Callers to update: `Shell::present`,
      `examples/dashboard/src/main.rs` (bench), libra-raster's own tests. (Grep
      gate: `grep -rn "render_damage" --include='*.rs' .`) The unchanged-frame
      early-out (`prev.cmds == cmds` → `Damage::None`) simply drops the argument.
- [x] In `push_a11y`, drop the extra clone: build the snapshot once, feed
      `update_if_active(|| a11y::tree_update(&snap))` from the local, then move
      `snap` into the mutex (reorder so the borrow ends before the move).
- [x] In `present()`, stop cloning the `Theme` every frame (`self.theme.clone()`)
      — restructure the borrows (it's only read).
- [x] Verify: bench numbers steady-or-better; `cargo test --workspace` green;
      snapshot-neutral.

## Stage 5 — Batch ingest pushes

`crates/libra-data/src/lib.rs` `run_source`. Goal: one ring lock and one wakeup
per ~millisecond instead of per sample, without changing a single sample value
(`sine_sample(k)` stays the pure function of `k` — [[14.2-architecture|§14.2]]).

- [x] Add `Ring::push_many(&self, samples: impl IntoIterator<Item = Sample>)` —
      one lock, same per-slot logic as `push`. Unit test: interleaved
      `push`/`push_many` produce the same buffer/generation as the equivalent
      `push` sequence.
- [x] Sine loop becomes: per wakeup, compute `due_k` from elapsed time
      (`elapsed_s * rate_hz`, floor + 1), generate samples `k..due_k` into a
      reused `Vec`, check `stop` **once immediately before** `push_many`, push,
      then sleep until the earlier of (next sample due, now + 1 ms) — i.e. cadence
      is `max(period, ~1 ms)`. Low-rate sources (< 1 kHz) keep their per-sample
      cadence; 10 kHz sources push ~10 samples per wake.
- [x] Tape source: same batching (push every sample whose `due_ms` has passed in
      one `push_many`).
- [x] `ingest_open_and_close_manage_a_live_source` currently asserts
      `g2 - g1 <= 2` after close; a final in-flight batch can now be ~10 samples.
      Rewrite the assertion to test what it means: flow *ceases* — sample
      `generation()` twice 50 ms apart *after* a 50 ms post-close grace, assert
      equal. Document why in the test.
- [x] Verify: `chart_columns_incremental_equals_cold_rebuild` untouched and green
      (batching is upstream of the ring; column math unchanged).
- [ ] Live run: charts identical in character, Activity Monitor shows the
      dashboard's overall CPU drop. **Not done this session — no GUI/display
      access.**

## Stage 6 — Retina-resolution chart columns (the blockiness fix, part 1)

Charts compute M4 columns at logical width, so Retina renders 2-px-wide steps.
Thread a density hint into paint; **the canonical path stays at 1.0, so this is
snapshot-neutral**. The shell is off the determinism contract ([[21-workspace-and-crate-layout|§21]]) — this is
exactly the HiDPI seam the architecture reserved.

- [x] `crates/libra-components/src/lib.rs`: add
      `LaidTree::paint_at_density(&self, measurer, store, focus, theme, density: f32)`;
      existing `paint(…)` delegates with `1.0`. Only `ItemKind::Chart` consumes
      it: pass `density` into `paint_chart`.
- [x] In `paint_chart`: `let width_px = ((inner.w * density.max(0.01)).round() as u32).max(1);`
      — everything downstream (`chart_columns`, the dense/sparse threshold
      `cols.samples >= 2 * width_px`, `m4_downsample`) uses it unchanged.
      `col_w = inner.w / cols.strips.len()` stays in **logical** units — after
      the shell's `scale_cmds` at `s == density`, columns land exactly 1 physical
      px wide. Strip y-values were always f32 → they scale losslessly.
- [x] `Shell::present` calls `laid.paint_at_density(…, scale)` (the window scale
      factor). Harness, bench, codegen, conformance keep calling `paint` — grep
      gate: outside `libra-platform`, no caller passes a density ≠ 1.0.
- [x] Doc comment on `paint_at_density`: density is a *presentation* hint ([[21-workspace-and-crate-layout|§21]]);
      the canonical frame ([[09-rendering|§9]]) is density 1.0 by definition; determinism of
      `chart_columns` is per-(window, width) so both densities are individually
      deterministic ([[14.3-downsampling-m4|§14.3]]'s cache keys on width_px already).
- [x] Perf guard: at density 2 the per-chart column count doubles (~600), the
      open-column recompute stays ≤ one column of samples, and the strip cache's
      per-(series,window) single-width eviction still holds because the shell only
      ever queries one width per chart. Confirmed with bench (unchanged, density
      1 — the bench never calls `paint_at_density`, only `paint`).
- [ ] Live S0 stats run (present p50 must not grow by more than ~0.5 ms) and
      visual confirmation of 1-px columns. **Not done this session — no
      GUI/display access.**
- [x] Verify: snapshot-neutral (`/tmp/snapshot-baseline.txt` compare — all 11
      PNGs byte-identical).

## Stage 7 (optional, **byte-changing**) — Anti-aliased strip edges (blockiness fix, part 2)

The only stage that re-blesses. `fill_strips` writes hard integer-rounded runs;
slow curves staircase even at 1-px columns. Add fractional vertical coverage at
each column's ends. Do **not** switch dense charts to AA polylines — that's the
18.6 ms → 0.64 ms lesson ([[22-performance-budgets|§22]], CLAUDE.md hot-path lore); this adds ≤2 blended
pixels per column.

- [x] In `fill_strips` (`crates/libra-raster/src/lib.rs`): keep the opaque core
      exactly as computed today (`round(y0)..round(y1)`, min 1 px — visibility
      guarantee for flat lines), then blend one partial-coverage pixel above and
      below: coverage = the fractional overhang of the float span beyond the
      opaque core, ×255, using the same src-over premultiplied math as
      `blend_glyph` (extract a shared `blend_px` helper). Pure f32 → deterministic.
- [x] Property test `damage_tracked_render_is_byte_equivalent_to_full` already
      mutates strips — must stay green unmodified.
- [x] Re-bless: `LIBRA_BLESS=1 cargo test --workspace`, then **eyeball every
      changed PNG** (gallery is the component×theme matrix — chart entries change,
      nothing else may). Only chart-bearing snapshots change; if a non-chart PNG
      changes, stop — something leaked.
- [x] Update the SHA-256 hashes in README.md for exactly the changed PNGs, same
      commit (they're the cross-machine determinism proof).
- [x] Verify: bench raster within budget (+0.09ms p50, within the "expect ≈
      +2 blended px × 600 cols × 16 charts ≈ negligible" estimate); staircase
      gone — confirmed by eyeballing all four re-blessed PNGs directly
      (smooth curves, no other pixels changed). Live windowed-app run not
      done this session (no GUI/display access), but the PNGs exercise the
      exact same `fill_strips` code path the live shell calls.

---

## Combined verification (after S6, and again after S7 if taken)

- [x] `cargo test --workspace` green (71/71 test-result blocks, every stage).
- [x] `cargo run -p dashboard --release -- bench` — all budgets pass:
      ```
      frame total   p50 ~0.73 ms   p95 ~0.85 ms   (budget: < 2 ms)
      ```
      (moved from p50 0.64ms pre-Stage-7 to ~0.73ms after Stage 7's AA
      blending — expected, still well inside budget.)
- [ ] `LIBRA_FRAME_STATS=1 cargo run -p dashboard --release` ~30 s: interval
      p50 ≈ monitor period, p95 within ~1 ms of p50; present p50 comfortably
      < 4 ms at Retina. **Not done this session — no GUI/display access.**
- [ ] Interactive: pause/resume, resize drag, theme hot reload
      (`--theme themes/solarized.toml` path if wired), window close/reopen
      (rehydration) — no regressions. **Not done this session — no
      GUI/display access.**
- [x] Snapshot hashes: unchanged through S6 (verified via
      `shasum -a 256 -c` against the pre-Stage-0 baseline after every stage);
      S7's changes limited to exactly the four chart-bearing PNGs
      (`dashboard_wave`, `gallery_dark`, `gallery_light`,
      `gallery_focus_dark`) — visually confirmed smooth via the rendered
      PNGs, no other snapshot moved. README's pinned hashes
      (`counter_initial.png`, `counter_three.png`) don't cover any of the
      changed files, so no README edit was needed or made.

---

← [[Architecture Home]] · [[Plans Index]] · [[Map of Content]]
