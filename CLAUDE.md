# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Libra: a native UI framework in Rust where every layer — logic, views, effects, rendered pixels — is deterministic and machine-verifiable. **LIBRA-ARCHITECTURE.md is the source of truth**; consult it before designing anything, and cite the relevant § in doc comments (the existing code does this consistently). The repo has **completed Phase 3's core** (§23). Done: Phases 0–2 (determinism gate; effects/KV/Tier-3/LibraReflect/hot-reload; data plane + damage tracking + §22 benchmark at 0.64 ms p50), plus Phase 3's two pillars: **libra-codegen** (release engine: `.lx` → typed Rust via build.rs; typos = rustc errors with `// file:line:col` trailers; deps libra-markup ONLY — structural no-drift) with the **conformance suite** (`conformance/`: fixtures compiled by build.rs AND interpreted; `interp_and_codegen_produce_identical_trees` is the §6.4 guardrail — grammar changes must update BOTH engines and keep it green), and **§10.1 text entry's core**: libra-text (pure EditState × EditOp, undo grouped at word boundaries), Input widget (controlled: model is source of truth; testId = focus identity), shell keyboard model, harness `type_text`/`edit(EditOp)`. Also landed: **theming** (§11: `Theme` in libra-draw threaded through layout+paint; `libra-dark` is byte-identical to the old tokens — existing snapshots depend on that; `Cmd::SetTheme` is shell/harness-handled, never reaches workers; harness `.theme("libra-light")`), **clipboard** (shell-side arboard, text-only features — the editing core stays pure), and **`Field<T>`** (FormState). Backlog worked: TOML theme tables (`Theme::from_toml_str`, flat key/value subset, `Options::theme_file` + hot reload; sample at `themes/solarized.toml`), **AccessKit** (§17: `platform/src/a11y.rs` maps the semantics tree to a TreeUpdate; AT Clicks route through the effects sink as real msgs; the adapter MUST be created before the window is first shown — the window is created hidden for this), `libra-query` (pure dedup/SWR/retry-backoff/invalidation; time injected), `NavStack`, and `examples/gallery` (widget×theme snapshot matrix — widget rendering changes fail there first). Remaining: catalog widgets, Textarea/IME/touch, packaging. Do not build ahead, but keep shapes compatible with later phases.

## Commands

```sh
cargo test --workspace                 # the whole suite (headless, no GPU/display needed)
cargo run -p counter                   # the Phase 0 counter in a real window
cargo run -p fetcher                   # Phase 1 reference app: real HTTP + redb persistence + hot reload
cargo run -p dashboard --release       # Phase 2: 16 live charts @ 10 kHz through the data plane
cargo run -p dashboard --release -- bench   # §22 budget check (headless, deterministic feed)
cargo run -p notes                     # Phase 3: text entry (Input + editing core + keyboard)
cargo run -p libra-cli -- check .      # parse/validate all .lx markup (microseconds)
cargo test -p fetcher tier3_           # single test(s) by name prefix
cargo test -p libra-markup             # one crate's unit tests
LIBRA_BLESS=1 cargo test -p counter    # re-record Tier-4 snapshots after intentional visual changes
```

**Theming invariant:** `Theme::dark()` values must stay byte-identical to `libra_draw::tokens` — every committed snapshot renders under it. New themes are additive; changing dark means re-blessing everything and updating README hashes.

Snapshot tests: missing PNG → recorded on first run; mismatch → writes `<name>.actual.png` next to the expected file and fails. Snapshots live in `examples/counter/tests/snapshots/` and are committed; their SHA-256 hashes in README.md are the cross-machine determinism proof — update both together after a bless.

## Architecture (the pipeline)

One-way data flow, every stage a pure function of the previous:

```
.lx file → libra-markup (parse → AST with spans)
         → libra-interp (AST × model reflection → ViewNode; load-time validation)
         → libra-widgets (ViewNode → taffy layout → LaidTree: geometry + semantics + handlers)
         → LaidTree.paint() → Vec<DrawCmd> (libra-draw: display list + theme tokens)
         → libra-raster (tiny-skia + fontdue → deterministic RGBA/PNG bytes)
```

- **libra-core** — the app contract: `App` trait is `init` + `update` only (pure; §5.2: no I/O, clocks, randomness, threads, or blocking — time/entropy arrive inside msgs). Reflection lives in the `ReflectModel`/`ReflectMsg` supertraits, normally via `#[derive(LibraReflect)]` (re-exported here): Display fields → bindable paths, `#[libra(skip)]` to exclude (non-Display state like `Remote` keeps a plain display-projection field maintained in `update`, per §6.3); unit Msg variants → handler names, payload variants are effect results and don't construct by name. The §12.1 Cmd vocabulary (Http, Kv*, Timer/CancelTimer, Random) and `Remote<T,E>` live here. **Gotcha:** `Cmd`'s `PartialEq` is hand-written to compare effect data and ignore msg-ctor identity — fn-pointer equality breaks across crate boundaries; don't "simplify" it back to a derive.
- **libra-reflect** — the proc-macro crate for `LibraReflect`. Dev-dep cycle with libra-core is intentional (tests exercise the derive against the real traits).
- **libra-effects** — the only I/O actor: worker pool (http/kv/random) + dedicated timer threads; `IngestOpen/Close` manage data-plane source threads. Results return as msgs through a `Sink<M>` (tests: mpsc; windowed shell: winit `EventLoopProxy`). Cross-effect ordering is deliberately unguaranteed (§12.3) — models must tolerate reordering, and tier-3 tests should exploit it.
- **libra-data** — the data plane (§14): Mutex-guarded rings (SPSC seqlock later; API-compatible), `.ltape` tapes (line-oriented `t_ms<TAB>value`), deterministic generators, M4. **The §14.3 column cache** (`SeriesStore::chart_columns`) is the chart hot path: windows quantize to whole pixel columns; *closed* columns are cached immutable (per-series time is monotonic), only the open column recomputes per paint. Its incremental-equals-cold-rebuild property test is the guardrail — keep it passing. In tests, feed data with `feed_tape_sync`/`Harness::feed_tape` (synchronous, threadless); ingest threads are for live runs only.
- **Rendering hot-path lore (learned from the §22 bench, 18.6 ms → 0.64 ms):** dense chart data must render as `DrawCmd::Strips` (opaque per-column min/max runs — the M4 paper's own rendering), never as long AA polylines; and `render_damage` must replay cmds at **original coordinates** into a full-size scratch (translated re-rasterization is not FP-bit-identical for stroked paths). The `damage_tracked_render_is_byte_equivalent_to_full` property test enforces the equivalence contract — any raster change must keep it green.
- **libra-kv** — redb store in the platform app-data dir. Apps never touch it directly; only `Cmd::Kv*` through the effects runtime.
- **libra-platform** — the winit/softbuffer windowed shell. Presentation glue only: it drives the same pipeline as the harness, then blits. It is **not** on the determinism contract's paint path — HiDPI works by scaling the DrawCmd stream at present time; the canonical frame stays the logical-resolution render. Effects results dispatch via `user_event`; `.lx` hot reload polls mtime (state preserved; parse errors keep the last good view + draw an overlay); `run_rehydrating` snapshots the model (serde JSON) on close and restores it at launch, discarding unparseable snapshots (§18.2); paint runs inside `catch_unwind` — a paint panic becomes an overlay frame, not a dead window. Pointer clicks only until Phase 3 text entry.
- **libra-test** — the in-process `Harness<A: App>`: builds the real view via the real interpreter, `by::test_id/text/role` queries against the semantics tree, `click()` through real hit testing, `assert_snapshot()` byte-exact PNG comparison (mismatch writes `.actual.png` + `.diff.png` heatmap). Tier 3: cmds from `init`/`update` are intercepted into a queue — `expect_cmd(cmd::http())`, `resolve_http/kv_get/kv_set/kv_scan/random`, `fire_timer`, `expect_no_cmds`.
- **examples/counter** (Phase 0) and **examples/fetcher** (Phase 1: fetch + persist + `Remote`) — reference apps; `tests/tiers.rs` names tests `tier1_*` (pure update), `tier2_*` (headless view queries/clicks), `tier3_*` (effect-script flows), `tier4_*` (byte-exact snapshots) per §15. Follow that naming for new tier tests.

Key invariants (Appendix A checklist — any change must keep these):
- Views are data with a **closed grammar**: no expressions, closures, mutation, or side effects in `.lx`. If it needs computing, it goes on the Model.
- Effects are data: `update` returns `Cmd` descriptions, never performs them.
- Rendering is a pure function of the display list — identical bytes on every machine. Nothing environment-dependent (system fonts, GPU, wall clock) may enter the paint path.
- Every widget emits `Semantics` (role/text/test_id) — it's the shared tree for a11y **and** test queries.
- Markup/interp errors carry `Span` (line:col) and, where possible, a suggestion.

## Phase 0 spike shortcuts (deliberate, listed in README.md)

- Reflection is hand-written (`App::field` / `App::msg_by_name`); Phase 1 replaces with `#[derive(LibraReflect)]`. Keep the two in sync with the Msg enum when touching an app.
- No reconciler: the laid tree rebuilds fully per dispatch.
- Text is fontdue advance-layout only (no shaping/kerning); fonts are bundled Liberation Sans via `include_bytes!` from `assets/fonts/`.
- Dependency versions for the determinism-critical stack are **pinned exactly** (`=0.5.2` taffy, `=0.11.4` tiny-skia, `=0.8.0` fontdue) in the workspace `Cargo.toml` — bumping any of them can change rendered bytes and requires re-blessing snapshots.

## Crate layout rules

- `libra-interp` and `libra-codegen` may depend on `libra-markup` only — the no-drift guarantee (§6.4) is structural. **Any grammar change (new element/attr/default) must be made in BOTH engines and covered by a conformance fixture** — the `conformance/` tests are the drift alarm.
- The editing core (`libra-text`) stays pure — no clocks, no platform types, no rendering. The shell and harness both drive it; new editing behavior means new `EditOp`s + Tier-1 tests there, then thin mapping layers.
- Workspace-internal deps go through `[workspace.dependencies]` in the root `Cargo.toml`; new crates go in `crates/` (auto-membered by the `crates/*` glob).
