# Libra — Phase 3

A native UI framework in Rust with a first-class testable foundation.
See LIBRA-ARCHITECTURE.md for the full design. Phase 0 proved the
determinism gate; Phase 1 built the testable foundation (effects,
persistence, integration tests, hot reload, CLI); Phase 2 built the
data plane (rings, tapes, M4 charts, subscriptions, damage tracking);
Phase 3 brings the release engine (codegen + the interp≡codegen
conformance suite, compile-time markup errors), text entry (the pure
editing core, Input, focus/keyboard/clipboard, type_text harness
verbs), theming (libra-dark / libra-light, TOML theme tables with hot
reload, Effect::SetTheme, theme-aware snapshots), AccessKit
integration (§17: the semantics tree exported to assistive tech,
screen-reader clicks dispatch real actions), libra-query, FormState,
NavStack, and the gallery — plus the ergonomics layer (§3/§5/§6.2/§12.5):
`#[libra::state]`/`#[libra::actions]`/`libra::app!`, `<for>` collection
bindings, and `<Frames>` source elements, which all four example apps
now run on (see `MIGRATION-NAMING.md`/`PLAN-ERGONOMICS.md` for the
still-in-progress rename of the underlying crates/types this sugars
over).

## What exists

- `libra-markup` — the `.lx` parser (one grammar, spans, real diagnostics)
- `libra-interp` — dev engine: AST × state reflection → ViewTree, with
  load-time validation (typo'd handler/binding = error with file:line:col);
  `<for each={path} as="name" key={name}>` over `Vec<T>` state fields,
  `<Frames onTick={name}>` mount-scoped source elements
- `libra-core` — the App contract: State / Action / pure update / Effect
  (Http, Kv*, Timer, Random, Ingest*, SetTheme) + `Remote<T, E>` +
  `Field<T>` (FormState: dirty/touched/error, pure validators, submit
  gating — plain state machinery)
- `libra-reflect` — `#[derive(LibraReflect)]` and the newer
  `#[libra::state]`/`#[libra::actions]`/`libra::app!` (re-exported
  through the `libra` facade crate): Display fields become bindable
  paths (`#[libra(skip)]` to exclude), `Vec<T>` fields become iterable
  paths for `<for>`, plain `&mut self` methods become the `Action`
  enum + a generated `reduce` (camelCase public names, §3.3); hand-
  implementing the traits stays possible
- `libra-effects` — the effects runtime: worker pool, ureq HTTP, timer
  threads with cancellation (cancelled timers are never delivered)
- `libra-kv` — persistence: redb in the platform app-data dir; get/set/
  delete/prefix-scan (localStorage, but honest — §13.1)
- `libra-data` — the data plane (§14): per-series ring buffers, ingest
  sources (tapes, deterministic generators), `.ltape` files, M4
  downsampling, and the §14.3 column cache (live windows quantize to
  pixel columns; each frame folds in only new samples — proven
  equivalent to a cold rebuild by property test)
- `libra-components` — Column, Row, Text, Button + Chart/Sparkline (read
  their series from the store at paint time; dense data renders as M4
  min/max column strips, sparse as stroked polylines); taffy layout;
  semantics; hit testing; paint to a DrawCmd display list
- `libra-draw` — the display list (+ Polyline, Strips) + `Theme` (§11):
  the named-token table components style through; `libra-dark` is
  byte-identical to the Phase 0 tokens (snapshot identity preserved),
  `libra-light` ships alongside; `Effect::SetTheme` swaps live
- `libra-raster` — the canonical executor: tiny-skia + fontdue, manual
  src-over glyph blending, deterministic PNG output; damage-tracked
  incremental rendering (§9.3) that is byte-equivalent to a full
  render by property test
- `libra-codegen` — the release engine (§6.4): compiles `.lx` to a
  static Rust `fn(state) -> ViewNode`. Bindings become direct field
  accesses, handlers become real Action references — typos are rustc
  errors carrying `// file:line:col` trailers (§6.5). Depends on
  libra-markup only: the no-drift guarantee is structural.
- `conformance/` — the §6.4 suite: golden fixtures compiled by
  build.rs through codegen AND resolved by the interpreter; trees must
  match node-for-node across state variants (unicode, quotes, extremes,
  `<for>` at 0/1/N items, `<Frames>` mounting)
- `libra-query` — the query cache (Appendix C): dedup,
  stale-while-revalidate, retry with exponential backoff, invalidation
  — pure state machinery (time injected, effects returned as data)
- `libra-text` — the §10.1 editing core: a pure state machine
  (EditState × EditOp), caret/selection/word-nav on char boundaries,
  undo/redo grouped at word boundaries — exercised without rendering
- `libra-test` — the Harness: component-test queries (`by::test_id`/
  `text`/`role`/`source`) + real-hit-test clicks; integration-test
  effect interception (`expect_effect` / `resolve_*` / `expect_no_effects`);
  snapshot tests with diff heatmaps (LIBRA_BLESS=1 to re-record);
  §10.1 verbs: type_text / edit(EditOp) / focused_edit drive the same
  editing core the shell drives; `has_source(kind)` checks a mounted
  §12.5 source element
- `libra-platform` — the winit/softbuffer windowed shell: effects wired
  through the event loop, `.lx` hot reload with state preservation, dev
  error overlay, state rehydration across restarts (`run_rehydrating`,
  §18.2), paint-boundary panic containment, TOML theme files
  (`Options::theme_file`, hot-reloaded), AccessKit (§17): the same
  semantics tree component-test queries resolve against is served to
  assistive technology (`Role::Source` items excluded), and an AT
  Click dispatches the button's real action; HiDPI via DrawCmd
  scaling; §12.5's mounted `<Frames>` elements union with
  `App::subscriptions()` for frame ticks
- `libra` — the facade crate: depend on this to get `#[libra::state]`/
  `#[libra::actions]`/`libra::app!` at their documented import path
  (still needs `libra-core`/`libra-platform`/`serde` as direct deps too
  — see its doc comment)
- `cli/libra` — `libra check`: parse+validate every .lx in microseconds
- `examples/counter` — the Phase 0 reference app; §5.8's "counter in
  ~10 lines" made real
- `examples/fetcher` — the Phase 1 reference app: fetches (GitHub zen)
  and persists (redb), fully tested across all four kinds including
  adversarial effect-result ordering
- `examples/dashboard` — the Phase 2 benchmark app (§22): 16 series ×
  10 kHz through the data plane into a 4×4 chart wall, `Vec<SeriesHandle>`
  rows bound through `<for>`; `subscriptions:` drives one action per
  tick (pause-sensitive — see its module doc for why not `<Frames>`);
  tape → byte-exact chart snapshot tests
- `examples/notes` — the Phase 3 reference app: an Input driving
  `titleChanged(String)` per keystroke (controlled input; state is
  the source of truth), Save persisting via kv, a live theme toggle
  (Effect::SetTheme); caret/selection render byte-exactly in snapshots
- `examples/gallery` — every component × state × theme (Appendix C via
  Storybook): the PR visual-review surface; snapshot matrix covers
  both built-in themes; `cargo run -p gallery -- themes/solarized.toml`
  previews a custom theme table live

## Run it

    cargo test --workspace       # the suite (headless: no display server, no GPU)
    cargo run -p counter         # the Phase 0 counter, in a real window
    cargo run -p fetcher         # fetch + persist; edit fetcher.lx live for hot reload
    cargo run -p dashboard --release             # 16 live charts @ 10 kHz each
    cargo run -p dashboard --release -- bench    # §22 budget check, headless
    cargo run -p notes                           # text entry: click the field and type
    cargo run -p gallery                         # component gallery; theme toggle
    cargo run -p gallery -- themes/solarized.toml   # custom TOML theme, hot-reloaded
    cargo run -p libra-cli -- check .   # validate all markup

Requires Rust >= 1.75. The test suite never needs a display.

## §22 budgets (measured on Apple Silicon, release)

    frame total (16 live charts, 1280×800)   p50 0.64 ms   p95 0.78 ms   (budget: < 2 ms)
    view+layout                              p50 0.02 ms
    paint (M4 via column cache)              p50 0.05 ms
    raster (damage-tracked)                  p50 0.63 ms

## The gate

Snapshot truths (committed):

    f6ac3073baecb403e071c753be6927609a14b7447d24b0f2effb4167407e6a68  counter_initial.png
    b7436ed8150b6feb764aa9e9c0b9ffd7f73bac0909df88e316b9b52005e09fe6  counter_three.png

Every stack layer is pure Rust (taffy / tiny-skia / fontdue), so these
hashes should reproduce on any machine and OS. Run the suite on your
hardware and compare — that is the cross-machine determinism proof.

Status: **verified**. Snapshots recorded in a Linux container reproduced
byte-exact on macOS (Darwin 25.1.0, Apple Silicon) on 2026-07-12.

## Remaining spike shortcuts (all planned, none structural)

- Effect equality compares effect *data* and ignores ctor identity
  (fn-pointer equality is unreliable across crates — discovered the
  hard way, by a failing cross-crate unit test).
- Reflection is flat: `{count}` binds, `{user.name}` paths and payload
  handler dispatch (`Select(item.id)`) arrive with the grammar's Phase 2+
  forms. `<for>` iterates a flat `Vec<T>` field; dot-path access into a
  loop-bound value (`key={x.id}`, `series={x.id}`) isn't supported yet
  (§6.2) — needs structured, non-Display reflection into elements.
- Text is fontdue advance-layout (no shaping/kerning); Phase 3 is parley.
- No reconciler yet — the laid tree rebuilds per dispatch (fine at this
  scale; keyed diffing + retained component state comes with focus).
- No `<if>`/conditional markup yet (§26) — §12.5 source elements
  (`<Frames>`) can't be conditionally mounted; state-dependent ambient
  sources still need a hand-written `subscriptions:` function passed
  to `libra::app!` (see `examples/dashboard`).
- Rings are Mutex-guarded, not the specced lock-free SPSC seqlock —
  nanoseconds uncontended at these rates; the API is shaped so the
  swap is invisible when it lands.
- Series handles bind through their Display form (`id|window|paused`);
  typed bindings arrive with the grammar's growth. Pause freezes
  ingest, not the window (pinned-time pause needs the retained tree).
- Damage is a single dirty-rect union (a changing status line unions
  with charts into ~full-frame); disjoint multi-rect damage + layer
  caching land with the reconciler.
- LogView/VirtualList/Table deferred to the reconciler (§7.1); the M4
  cache serves Chart/Sparkline.
- Text entry is the §10.1 core scoped to single-line Input: char
  boundaries (graphemes with parley), no IME/touch/clipboard yet, no
  Textarea/rope. Focus identity is the Input's testId until the
  reconciler brings structural IDs; caret click-positioning lands with
  it too (click focuses with caret at end).
- Hot reload watches one root view file; multi-file watching lands
  with composition.
- `libra check` validates grammar only; binding/handler validation
  against State/Action needs the app's types in scope (the interpreter
  does it at load time; codegen turns them into rustc errors).
- AccessKit ships the tree + Click actions; text-editing AT actions
  (SetTextSelection, ReplaceSelectedText) land with the reconciler's
  focus model.
- v0.1 polish backlog (§23 Phase 3 tail): remaining catalog components,
  Textarea/IME/touch, and `libra package`/`build`. The naming migration
  (`MIGRATION-NAMING.md`) is complete — Stage 8 removed the deprecated
  pre-rename aliases and the legacy PascalCase markup-handler style.
