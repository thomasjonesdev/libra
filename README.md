# Libra — Phase 3

A native UI framework in Rust with a first-class testable foundation.
See LIBRA-ARCHITECTURE.md for the full design. Phase 0 proved the
determinism gate; Phase 1 built the testable foundation (effects,
persistence, Tier 3, hot reload, CLI); Phase 2 built the data plane
(rings, tapes, M4 charts, subscriptions, damage tracking); Phase 3
brings the release engine (codegen + the interp≡codegen conformance
suite, compile-time markup errors), text entry (the pure editing core,
Input, focus/keyboard/clipboard, type_text harness verbs), theming
(libra-dark / libra-light, TOML theme tables with hot reload,
Cmd::SetTheme, theme-aware snapshots), AccessKit integration (§17: the
semantics tree exported to assistive tech, screen-reader clicks
dispatch real msgs), libra-query, FormState, NavStack, and the
gallery.

## What exists

- `libra-markup` — the `.lx` parser (one grammar, spans, real diagnostics)
- `libra-interp` — dev engine: AST × model reflection → ViewTree, with
  load-time validation (typo'd handler/binding = error with file:line:col)
- `libra-core` — the App contract: Model / Msg / pure update / Cmd
  (Http, Kv*, Timer, Random, Ingest*, SetTheme) + `Remote<T, E>` +
  `Field<T>` (FormState: dirty/touched/error, pure validators, submit
  gating — plain model machinery)
- `libra-reflect` — `#[derive(LibraReflect)]`: Display fields become
  bindable paths (`#[libra(skip)]` to exclude), unit Msg variants become
  handler names; hand-implementing the traits stays possible
- `libra-effects` — the effects runtime: worker pool, ureq HTTP, timer
  threads with cancellation (cancelled timers are never delivered)
- `libra-kv` — persistence: redb in the platform app-data dir; get/set/
  delete/prefix-scan (localStorage, but honest — §13.1)
- `libra-data` — the data plane (§14): per-series ring buffers, ingest
  sources (tapes, deterministic generators), `.ltape` files, M4
  downsampling, and the §14.3 column cache (live windows quantize to
  pixel columns; each frame folds in only new samples — proven
  equivalent to a cold rebuild by property test)
- `libra-widgets` — Column, Row, Text, Button + Chart/Sparkline (read
  their series from the store at paint time; dense data renders as M4
  min/max column strips, sparse as stroked polylines); taffy layout;
  semantics; hit testing; paint to a DrawCmd display list
- `libra-draw` — the display list (+ Polyline, Strips) + `Theme` (§11):
  the named-token table widgets style through; `libra-dark` is
  byte-identical to the Phase 0 tokens (snapshot identity preserved),
  `libra-light` ships alongside; `Cmd::SetTheme` swaps live
- `libra-raster` — the canonical executor: tiny-skia + fontdue, manual
  src-over glyph blending, deterministic PNG output; damage-tracked
  incremental rendering (§9.3) that is byte-equivalent to a full
  render by property test
- `libra-codegen` — the release engine (§6.4): compiles `.lx` to a
  static Rust `fn(model) -> ViewNode`. Bindings become direct field
  accesses, handlers become real Msg references — typos are rustc
  errors carrying `// file:line:col` trailers (§6.5). Depends on
  libra-markup only: the no-drift guarantee is structural.
- `conformance/` — the §6.4 suite: golden fixtures compiled by
  build.rs through codegen AND resolved by the interpreter; trees must
  match node-for-node across model states (unicode, quotes, extremes)
- `libra-query` — the query cache (Appendix C): dedup,
  stale-while-revalidate, retry with exponential backoff, invalidation
  — pure model machinery (time injected, effects returned as Cmds)
- `libra-text` — the §10.1 editing core: a pure state machine
  (EditState × EditOp), caret/selection/word-nav on char boundaries,
  undo/redo grouped at word boundaries — exercised without rendering
- `libra-test` — the Harness: Tier-2 queries + real-hit-test clicks;
  Tier-3 cmd interception (expect_cmd / resolve_* / expect_no_cmds);
  Tier-4 byte-exact snapshots with diff heatmaps (LIBRA_BLESS=1 to
  re-record); §10.1 verbs: type_text / edit(EditOp) / focused_edit
  drive the same editing core the shell drives
- `libra-platform` — the winit/softbuffer windowed shell: effects wired
  through the event loop, `.lx` hot reload with state preservation, dev
  error overlay, model rehydration across restarts (`run_rehydrating`,
  §18.2), paint-boundary panic containment, TOML theme files
  (`Options::theme_file`, hot-reloaded), and AccessKit (§17): the same
  semantics tree Tier-2 queries resolve against is served to assistive
  technology, and an AT Click dispatches the button's real msg; HiDPI
  via DrawCmd scaling
- `cli/libra` — `libra check`: parse+validate every .lx in microseconds
- `examples/counter` — the Phase 0 reference app (Tiers 1/2/4)
- `examples/fetcher` — the Phase 1 reference app: fetches (GitHub zen)
  and persists (redb), fully tested at Tiers 1–4 including adversarial
  effect-result ordering
- `examples/dashboard` — the Phase 2 benchmark app (§22): 16 series ×
  10 kHz through the data plane into a 4×4 chart wall; `Sub::frames`
  drives one msg per tick; tape → byte-exact chart snapshot tests
- `examples/notes` — the Phase 3 reference app: an Input driving
  `TitleChanged(String)` per keystroke (controlled input; the model is
  the source of truth), Save persisting via kv, a live theme toggle
  (Cmd::SetTheme); caret/selection render byte-exactly in snapshots
- `examples/gallery` — every widget × state × theme (Appendix C via
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
    cargo run -p gallery                         # widget gallery; theme toggle
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

- Cmd equality compares effect *data* and ignores msg-ctor identity
  (fn-pointer equality is unreliable across crates — discovered the
  hard way, by a failing cross-crate Tier-1 test).
- Reflection is flat: `{count}` binds, `{user.name}` paths and payload
  handler dispatch (`Select(item.id)`) arrive with the grammar's Phase 2+
  forms.
- Text is fontdue advance-layout (no shaping/kerning); Phase 3 is parley.
- No reconciler yet — the laid tree rebuilds per dispatch (fine at this
  scale; keyed diffing + retained widget state comes with focus).
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
  against Model/Msg needs the app's types in scope (the interpreter
  does it at load time; codegen turns them into rustc errors).
- AccessKit ships the tree + Click actions; text-editing AT actions
  (SetTextSelection, ReplaceSelectedText) land with the reconciler's
  focus model.
- v0.1 polish backlog (§23 Phase 3 tail): remaining catalog widgets,
  Textarea/IME/touch, `libra package`/`build`.
