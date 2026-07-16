# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Libra: a native UI framework in Rust where every layer — logic, views, effects, rendered pixels — is deterministic and machine-verifiable. **The Obsidian vault at `architecture/` is the source of truth** (not a monolithic markdown file). **Read `architecture/AGENTS.md` first** for an end-to-end brief; then open the relevant § note before designing anything. Cite the relevant § in doc comments (the existing code does this consistently — resolve § ids via `architecture/SECTION-INDEX.md`). The repo has **completed Phase 3's core** (§23). Done: Phases 0–2 (determinism gate; effects/KV/integration tests/LibraReflect/hot-reload; data plane + damage tracking + §22 benchmark at 0.64 ms p50), plus Phase 3's two pillars: **libra-codegen** (release engine: `.lx` → typed Rust via build.rs; typos = rustc errors with `// file:line:col` trailers; deps libra-markup ONLY — structural no-drift) with the **conformance suite** (`conformance/`: fixtures compiled by build.rs AND interpreted; `interp_and_codegen_produce_identical_trees` is the §6.4 guardrail — grammar changes must update BOTH engines and keep it green), and **§10.1 text entry's core**: libra-text (pure EditState × EditOp, undo grouped at word boundaries), Input component (controlled: state is source of truth; testId = focus identity), shell keyboard model, harness `type_text`/`edit(EditOp)`. Also landed: **theming** (§11: `Theme` in libra-draw threaded through layout+paint; `libra-dark` is byte-identical to the old tokens — existing snapshots depend on that; `Effect::SetTheme` is shell/harness-handled, never reaches workers; harness `.theme("libra-light")`), **clipboard** (shell-side arboard, text-only features — the editing core stays pure), and **`Field<T>`** (FormState). Backlog worked: TOML theme tables (`Theme::from_toml_str`, flat key/value subset, `Options::theme_file` + hot reload; sample at `themes/solarized.toml`), **AccessKit** (§17: `platform/src/a11y.rs` maps the semantics tree to a TreeUpdate; AT Clicks route through the effects sink as real actions; the adapter MUST be created before the window is first shown — the window is created hidden for this), `libra-query` (pure dedup/SWR/retry-backoff/invalidation; time injected), `NavStack`, and `examples/gallery` (component×theme snapshot matrix — component rendering changes fail there first).

**The ergonomics layer (2026-07, §3/§5/§6.2/§12.5) is built and all six examples (including gallery) run on it**: `#[libra::state]` / `#[libra::actions]` (a `State` struct + plain `&mut self` methods generate the `Action` enum and `reduce` — see `crates/libra-reflect/src/attrs.rs`; one method may be marked **`#[project]`** — §6.3's display-projection hook, auto-called by the generated `reduce` after every action and by `app!`'s no-`init:` default, so derived display fields never go stale and are never manually refreshed), `libra::app!` (one declaration for the `App` impl + view bundle + `run()` — `crates/libra-reflect/src/app.rs`, facade crate at `crates/libra`), `<for each={path} as="name" key={name}>` over `Vec<T>` state fields (§6.2, both engines — `ReflectState::collection`), **item-scoped handlers `onPress={action(loopVar)}`** (§6.2 — the loop binding's Display becomes the handler's `String` payload via `action_with_str`; `AttrValue::Call` in libra-markup, `on_press_arg` on `ViewNode::Button`/`Item`, resolved+validated in BOTH engines, dispatched by harness+shell; the `for_actions` conformance fixture is the drift guard — this is what makes per-row toggle/delete expressible, see `examples/todos`), and `<Frames onTick={name}>` mount-scoped source elements (§12.5, `Frames` only — `Interval`/`Stream` not yet built). Still future (§6.2 vision, not built): item dot-paths `{item.field}` / `key={item.id}` / `select(item.id)` — the item is a whole-value Display string, so identity must ride *in* that string (todos uses a `"<id>. <text>"` line). Naming/ergonomics staging plans are complete and live under `architecture/plans/` (`MIGRATION-NAMING`, `PLAN-ERGONOMICS`): Stage 8 has run — the deprecated aliases for the pre-rename vocabulary (see §3.2's old→new table) are gone, and markup handler names are camelCase-only in both engines (a PascalCase handler reference is now a load-time/compile-time error naming the camelCase form) — a CI naming guard (`.github/workflows/naming-guard.yml`) blocks the pre-rename names from reentering.

Remaining: catalog components, Textarea/IME/touch, packaging, `<if>`/conditional markup (tracked in §26 — blocks conditionally-mounted source elements). Do not build ahead, but keep shapes compatible with later phases.

## Architecture vault (source of truth)

**Location:** `architecture/` — open that folder as an Obsidian vault.

| Need | Go here |
|---|---|
| **End-to-end brief (start here)** | **`architecture/AGENTS.md`** |
| Entry + full outline | `architecture/Architecture Home.md` |
| Graph / MOC | `architecture/Map of Content.md` |
| Resolve `§N.N` → file | `architecture/SECTION-INDEX.md` |
| Numbered design notes | `architecture/sections/` (§1–§26, one note per section *and* subsection) |
| Appendices | `architecture/appendices/` |
| Execution plans | `architecture/plans/` (+ `architecture/Plans Index.md`) |

### How to find

1. **Always start with `architecture/AGENTS.md`** when you need system context (pipeline, bets, closed sets, crate map, what's built, how to change safely).
2. For a citation like `ARCHITECTURE.md §12.5` or `§5.2`, open `architecture/SECTION-INDEX.md` and jump to that note (or Obsidian-quick-open the alias `§12.5` / `§5.2`).
3. Parent notes link to children under **In this vault**; subsections link back to their parent. Cross-refs inside notes are wiki-links (`[[…|§N.N]]`).
4. Plans that implement architecture decisions are vault notes under `architecture/plans/`, indexed from `Plans Index.md`.

### How to update

1. **Edit the specific note** under `sections/`, `appendices/`, or `plans/` — never rebuild a single mega-document at the repo root.
2. Keep frontmatter honest: `section: "N.N"`, aliases including `§N.N` and `N.N`, tags `architecture` / `section/N` (or `plans`).
3. When adding a new subsection, create its note, link it from the parent’s **In this vault** list, and add a row to `SECTION-INDEX.md` plus the outlines in `Architecture Home.md` / `Map of Content.md`.
4. When a decision changes behavior or a contract, update the normative architecture note *and* any dependent plan status; cite the § in code/doc comments the same way existing crates do.
5. Wiki-link related notes when you introduce a dependency between topics (effects ↔ testing, markup ↔ codegen, etc.) so the graph stays a structured related dataset.
6. Follow the light **“When this change lands”** checklist in `architecture/AGENTS.md` §9 — governing note + status lines if needed; not a full vault rewrite.

### How to respect

- The vault is **normative**. Code that disagrees with it is wrong (or the vault needs an explicit, deliberate update in the same change).
- Do not invent features ahead of §23 / §26; keep shapes compatible with open questions.
- Appendix A (determinism checklist) is the non-negotiable gate for paint/actions/effects/markup changes.
- Grammar changes still require **both** engines + a `conformance/` fixture (§6.4) — the vault note is not a substitute for that guardrail.
- Historical pre-rename vocabulary may appear only in vault history notes (§2/§3.2/§21, `MIGRATION-NAMING`); CI naming-guard allows `architecture/` for that record and nowhere else in product code.

## Commands

```sh
cargo test --workspace                 # the whole suite (headless, no GPU/display needed)
cargo run -p counter                   # the Phase 0 counter in a real window
cargo run -p fetcher                   # Phase 1 reference app: real HTTP + redb persistence + hot reload
cargo run -p dashboard --release       # Phase 2: 16 live charts @ 10 kHz through the data plane
cargo run -p dashboard --release -- bench   # §22 budget check (headless, deterministic feed)
cargo run -p notes                     # Phase 3: text entry (Input + editing core + keyboard)
cargo run -p todos                     # §6.2 item-scoped handlers: per-row toggle/delete in a <for>
cargo run -p libra-cli -- check .      # parse/validate all .lx markup (microseconds)
cargo test -p fetcher --test integration    # one test *kind* (unit/components/integration/snapshots)
cargo test -p libra-markup             # one crate's unit tests
LIBRA_BLESS=1 cargo test -p counter    # re-record snapshot tests after intentional visual changes
```

**Theming invariant:** `Theme::dark()` values must stay byte-identical to `libra_draw::tokens` — every committed snapshot renders under it. New themes are additive; changing dark means re-blessing everything and updating README hashes.

Snapshot tests: missing PNG → recorded on first run; mismatch → writes `<name>.actual.png` next to the expected file and fails. Snapshots live in `examples/counter/tests/snapshots/` and are committed; their SHA-256 hashes in README.md are the cross-machine determinism proof — update both together after a bless.

## Architecture (the pipeline)

One-way data flow, every stage a pure function of the previous:

```
.lx file → libra-markup (parse → AST with spans)
         → libra-interp (AST × state reflection → ViewNode; load-time validation)
         → libra-components (ViewNode → taffy layout → LaidTree: geometry + semantics + handlers)
         → LaidTree.paint() → Vec<DrawCmd> (libra-draw: display list + theme tokens)
         → libra-raster (tiny-skia + fontdue → deterministic RGBA/PNG bytes)
```

- **libra-core** — the app contract: `App` trait is `init` + `update` only (pure; §5.3: no I/O, clocks, randomness, threads, or blocking — time/entropy arrive inside actions). Reflection lives in the `ReflectState`/`ReflectAction` supertraits, normally via `#[derive(LibraReflect)]` or (preferred for new code) `#[libra::state]`/`#[libra::actions]` (both re-exported here): Display fields → bindable paths, `#[libra(skip)]` to exclude (non-Display state like `Remote` keeps a plain display-projection field maintained by an action, per §6.3), `Vec<T>` fields → iterable paths for `<for>` via `ReflectState::collection`; unit Action variants → handler names, single-`String`/`FrameInfo` variants → control-payload/tick handlers, other payload variants are effect results and don't construct by name. The §12.1 Effect vocabulary (Http, Kv*, Timer/CancelTimer, Random) and `Remote<T,E>` live here. **Gotcha:** `Effect`'s `PartialEq` is hand-written to compare effect data and ignore ctor identity — fn-pointer equality breaks across crate boundaries; don't "simplify" it back to a derive.
- **libra-reflect** — the proc-macro crate: `LibraReflect` (struct/enum derive) plus the newer `#[state]`/`#[actions]`/`app!` (re-exported through the `libra` facade crate as `#[libra::state]` etc. — see `crates/libra/src/lib.rs`). `attrs.rs` holds the state/actions macros (dispatchability classification, camelCase public-name mapping per §3.3), `app.rs` holds `app!` (state/view/window/title/`init:`/`subscriptions:` fields — the last two are optional escape hatches for custom startup effects and state-dependent ambient sources). Dev-dep cycle with libra-core is intentional (tests exercise the derive/macros against the real traits).
- **libra-effects** — the only I/O actor: worker pool (http/kv/random) + dedicated timer threads; `IngestOpen/Close` manage data-plane source threads. Results return as actions through a `Sink<M>` (tests: mpsc; windowed shell: winit `EventLoopProxy`). Cross-effect ordering is deliberately unguaranteed (§12.3) — state must tolerate reordering, and integration tests should exploit it.
- **libra-data** — the data plane (§14): Mutex-guarded rings (SPSC seqlock later; API-compatible), `.ltape` tapes (line-oriented `t_ms<TAB>value`), deterministic generators, M4. **The §14.3 column cache** (`SeriesStore::chart_columns`) is the chart hot path: windows quantize to whole pixel columns; *closed* columns are cached immutable (per-series time is monotonic), only the open column recomputes per paint. Its incremental-equals-cold-rebuild property test is the guardrail — keep it passing. In tests, feed data with `feed_tape_sync`/`Harness::feed_tape` (synchronous, threadless); ingest threads are for live runs only.
- **Rendering hot-path lore (learned from the §22 bench, 18.6 ms → 0.64 ms):** dense chart data must render as `DrawCmd::Strips` (opaque per-column min/max runs — the M4 paper's own rendering), never as long AA polylines; and `render_damage` must replay cmds at **original coordinates** into a full-size scratch (translated re-rasterization is not FP-bit-identical for stroked paths). The `damage_tracked_render_is_byte_equivalent_to_full` property test enforces the equivalence contract — any raster change must keep it green.
- **libra-kv** — redb store in the platform app-data dir. Apps never touch it directly; only `Effect::Kv*` through the effects runtime.
- **libra-platform** — the winit/softbuffer windowed shell. Presentation glue only: it drives the same pipeline as the harness, then blits. It is **not** on the determinism contract's paint path — HiDPI works by scaling the DrawCmd stream at present time; the canonical frame stays the logical-resolution render. Effects results dispatch via `user_event`; `.lx` hot reload polls mtime (state preserved; parse errors keep the last good view + draw an overlay); `run_rehydrating` snapshots state (serde JSON) on close and restores it at launch, discarding unparseable snapshots (§18.2); paint runs inside `catch_unwind` — a paint panic becomes an overlay frame, not a dead window. §12.5: `Shell` tracks `markup_frame_handlers` (refreshed every relayout from mounted `ItemKind::Source` items) and unions them with `App::subscriptions()`'s `Sub::Frames` in `sync_subscriptions`/`tick_frame` — either form of frame-tick declaration works. Pointer clicks only until Phase 3 text entry.
- **libra-test** — the in-process `Harness<A: App>`: builds the real view via the real interpreter, `by::test_id/text/role/source` queries against the semantics tree, `click()` through real hit testing, `assert_snapshot()` byte-exact PNG comparison (mismatch writes `.actual.png` + `.diff.png` heatmap), `has_source(kind)` for non-panicking mounted-source-element checks. Integration tests: effects from `init`/`update` are intercepted into a queue — `expect_effect(fx::http())`, `resolve_http/kv_get/kv_set/kv_scan/random`, `fire_timer`, `expect_no_effects`.
- **examples/counter, fetcher, dashboard, notes, gallery, todos** — all ported to `#[libra::state]`/`#[libra::actions]`/`libra::app!`; each has `tests/unit.rs` (or inline `#[cfg(test)] mod tests` in `src/lib.rs` for small apps like counter), `tests/components.rs`, `tests/integration.rs` (where applicable — dashboard/todos have none, no effects), `tests/snapshots.rs` per §15 — one file per test *kind*, no numeric prefixes. Follow that split for new example apps. `examples/dashboard` is the E2/E3 reference: `Vec<SeriesHandle>` rows bound through `<for>`, and `subscriptions:` (not `<Frames>`) for pause-sensitive ticking — see its `src/lib.rs` module doc for why. `examples/todos` is the §6.2 item-scoped-handler reference: `items: Vec<Todo>` source of truth + §6.3 `active`/`done`/`summary` display projections maintained by its `#[project]` hook (actions only mutate `items` — no manual refresh anywhere), with per-row `onPress={complete(t)}`/`onPress={remove(t)}` carrying the row line as the `String` payload.

Key invariants (Appendix A checklist — any change must keep these):
- Views are data with a **closed grammar**: no expressions, closures, mutation, or side effects in `.lx`. If it needs computing, it goes on State.
- Effects are data: actions return `Effect` descriptions, never perform them.
- Rendering is a pure function of the display list — identical bytes on every machine. Nothing environment-dependent (system fonts, GPU, wall clock) may enter the paint path.
- Every component emits `Semantics` (role/text/test_id) — it's the shared tree for a11y **and** test queries (`Role::Source` is the one exception: mounted, test-query-only, excluded from the AccessKit projection — §12.5).
- Markup/interp errors carry `Span` (line:col) and, where possible, a suggestion.

## Phase 0 spike shortcuts (deliberate, listed in README.md)

- Reflection is hand-written in the oldest code paths (`App::field` / `App::msg_by_name`, now `ReflectState::field` / `ReflectAction::action_by_name`); `#[derive(LibraReflect)]` and `#[libra::state]`/`#[libra::actions]` are the two generated paths (the latter preferred for new apps). Keep them in sync with the Action enum when touching an app.
- No reconciler: the laid tree rebuilds fully per dispatch.
- Text is fontdue advance-layout only (no shaping/kerning); fonts are bundled Liberation Sans via `include_bytes!` from `assets/fonts/`.
- Dependency versions for the determinism-critical stack are **pinned exactly** (`=0.5.2` taffy, `=0.11.4` tiny-skia, `=0.8.0` fontdue) in the workspace `Cargo.toml` — bumping any of them can change rendered bytes and requires re-blessing snapshots.
- The grammar has no `<if>`/conditional element yet (§26) — §12.5 source elements (`<Frames>`) can't be conditionally mounted; state-dependent ambient sources still need `libra::app!`'s `subscriptions:` field (see dashboard).

## Crate layout rules

- `libra-interp` and `libra-codegen` may depend on `libra-markup` only — the no-drift guarantee (§6.4) is structural. **Any grammar change (new element/attr/default) must be made in BOTH engines and covered by a conformance fixture** — the `conformance/` tests are the drift alarm.
- The editing core (`libra-text`) stays pure — no clocks, no platform types, no rendering. The shell and harness both drive it; new editing behavior means new `EditOp`s + unit tests there, then thin mapping layers.
- **All** deps — workspace-internal crates *and* third-party — are declared once in `[workspace.dependencies]` in the root `Cargo.toml`; crate manifests reference them with `<dep>.workspace = true` (add crate-specific features alongside if needed). Don't inline a `version = "…"` in a crate manifest — hoist it to the root so every crate stays on one version + feature policy (this is also where the determinism-critical rendering stack is pinned exactly). New crates go in `crates/` (auto-membered by the `crates/*` glob). Apps using `#[libra::state]`/`#[libra::actions]`/`libra::app!` need `libra` (for the macro import path) *and* `libra-core` (always) *and* `libra-platform` (if using `app!`) as direct dependencies — the generated code references `::libra_core::`/`::libra_platform::` paths directly, not `::libra::` (see `crates/libra/src/lib.rs`'s doc comment). `#[libra::state]`'s bundled `Serialize`/`Deserialize` derive also needs `serde` (`features = ["derive"]`) as a direct dependency.
