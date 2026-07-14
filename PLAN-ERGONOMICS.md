# Ergonomics Implementation Plan

Implements the DX layer specced in LIBRA-ARCHITECTURE.md (2026-07 revision). **Read before writing code:** §3.1–§3.3 (vocabulary + naming conventions), §5 (state/actions/app assembly), §6.2 (grammar additions), §12.5 (source elements), and `MIGRATION-NAMING.md` (this plan interleaves with it).

Design is settled — the architecture doc is authoritative. This plan is execution order, acceptance criteria, and the traps.

## Combined execution order

| # | Work | Defined in |
|---|---|---|
| 1 | Mechanical renames | MIGRATION-NAMING.md Stages 0–3 |
| 2 | `#[libra::state]` / `#[libra::actions]` macros | MIGRATION-NAMING.md Stage 4 + §5.1–§5.2 |
| 3 | **E1** `libra::app!` assembly macro | §5.8, below |
| 4 | **E2** Collection bindings (`Vec<T>` + `<for>`) | §6.2, below |
| 5 | **E3** Source elements (`Frames`/`Interval`/`Stream`) | §12.5, below |
| 6 | Test reorg + example ports (now including E2/E3 usage) | MIGRATION-NAMING.md Stages 5–6, amended below |
| 7 | Docs sweep, then alias removal | MIGRATION-NAMING.md Stages 7–8 |

Universal ground rules (same as MIGRATION-NAMING.md): one PR per step, `cargo test --workspace` green at every boundary, **byte-identical snapshots** unless a step explicitly says otherwise (none here does), README hashes untouched, grammar changes always land in BOTH engines with conformance fixtures (`interp_and_codegen_produce_identical_trees` is the drift alarm).

---

## E1 — `libra::app!` (§5.8)

**Where:** macro in `libra-reflect` (macro_rules or proc-macro, implementer's choice). Decision point: §21 plans a `libra` facade crate that doesn't exist yet — either create it now (re-exporting core + reflect + platform entry) so the call is literally `libra::app!`, or export from `libra_core` with a doc alias and create the facade in Stage 7. Prefer creating the facade; it's small and §21 already commits to it.

**Generates:** the `App` impl (init from `State::init()`, else `Default`), the view bundle (watched dev path + `include_str!` release embedding from the one `view:` field), `main()` when the crate is a bin. No logic — every field literal or path; reject anything else at macro-expansion time with a clear error.

**Acceptance:**
- [ ] `examples/counter` compiles as exactly the §5.8 listing (state + actions + `app!`), `main.rs` reduced to a one-line delegation or deleted (bin target pointing at lib).
- [ ] Harness path unchanged: existing component/integration/snapshot tests pass without edits beyond the naming migration's.
- [ ] `libra-cli check` still validates the view (bundle is discoverable without executing the app).
- [ ] Counter snapshots byte-identical.

**Traps:** hot-reload needs the *filesystem path*, release needs the *embedded string* — the bundle must carry both from one declaration; don't let dev and release resolve the path differently (that's a §6.4-class drift bug in spirit).

## E2 — Collection bindings (§6.2)

**Where:** `libra-reflect` (bindable iterable paths for `Vec<T>` fields), `libra-interp` AND `libra-codegen` (`<for each={field}>` over reflected collections), `conformance/` fixture.

**Grammar line to hold:** iteration only — **no index expressions** (`{series[3]}` must be a span-diagnosed error with a suggestion, not a feature).

**Acceptance:**
- [ ] Conformance fixture: a `Vec` field iterated with `<for each={items} as="x" key={x.id}>` produces identical trees in both engines; an indexing attempt produces the same diagnostic in both.
- [ ] `examples/dashboard`: `s0`–`s15` collapse to `pub series: Vec<SeriesHandle>`; `dashboard.lx`'s 16 `<Chart>` lines become one `<for>` (per row: chunk in state or a nested Vec — model it in state per §6.3, not in markup).
- [ ] Dashboard snapshot **byte-identical** (same handles, same layout, same tree — the `<for>` must reconcile to the same structural IDs; if IDs shift, fix ID assignment, don't re-bless).
- [ ] `cargo run -p dashboard --release -- bench` still meets the §22 budget (0.64 ms p50 baseline; no regression beyond noise).

**Traps:** `key` is required when the collection can reorder — the dashboard's can't, but the fixture must cover both keyed and unkeyed. `SeriesHandle` is `#[libra(skip)]`-adjacent territory: handles are bindable as chart props but not Display-bound as text; make the reflect classification explicit.

## E3 — Source elements (§12.5)

**Where:** `libra-markup` (three new elements: `Frames`, `Interval`, `Stream` — closed set), both engines (fold to inert data nodes), runtime in `libra-core`/shell + harness (collect mounted declarations post-commit, union with `subscriptions(state)`, diff vs running set), `libra-test` (`by::source(...)`, `assert_mounted`), `crates/libra-platform/src/a11y.rs` (exclude the test-only role from the AccessKit TreeUpdate), `conformance/` fixtures.

**Semantics (normative, §12.5):** identity = (element kind, resolved props); start new / cancel removed after each commit; props are literals or bindings only; results arrive as ordinary recorded actions; zero DrawCmds, zero layout.

**Acceptance:**
- [ ] Conformance: fixture with source elements inside `<if>`/`<for>` produces identical trees in both engines.
- [ ] Unit tests on the differ: mount starts, unmount cancels, prop change = cancel + restart, duplicate declarations dedupe against `subscriptions(state)`.
- [ ] Component test verbs work: `h.get(by::source("frames")).assert_mounted()`; absence assertable when `running == false`.
- [ ] `examples/dashboard`: delete `fn subscriptions`, add `<if test={running}><Frames onTick={frame} /></if>` to the view. All existing tests pass; snapshots byte-identical (element is invisible).
- [ ] Hot-reload behavior: editing the markup toggles sources live, state preserved (manual check + an interp-level test).
- [ ] A11y audit unaffected: `libra check --a11y` output unchanged; source role never reaches AccessKit.

**Traps:** the runtime already diffs `subscriptions(state)` — reuse that differ, do not build a second lifecycle. Cancellation must drop late ticks (§12.3 zombie rule) identically for markup-declared sources. Naming per §3.3: `onTick`, `onEvent`, `every` — camelCase props.

## Amendments to MIGRATION-NAMING.md Stage 6 (example ports)

- [ ] Dashboard port includes E2 (series Vec) and E3 (`<Frames>` replacing `subscriptions`) — land E2/E3 framework support first, then the port PR uses all of it.
- [ ] All examples adopt `libra::app!` in their port PR.
- [ ] Fetcher keeps `Remote` + display-projection (§6.3) — the ergonomics layer doesn't change that pattern.

## Final verification (after step 7)

```sh
cargo test --workspace
cargo run -p libra-cli -- check .
shasum -a 256 examples/*/tests/snapshots/*.png | diff - /tmp/snapshot-baseline.txt
cargo run -p dashboard --release -- bench
```

Plus: `examples/counter/src/lib.rs` reads like §5.8's listing, `examples/dashboard/src/lib.rs` has no numbered series fields and no `subscriptions`, and CLAUDE.md describes all of it.
