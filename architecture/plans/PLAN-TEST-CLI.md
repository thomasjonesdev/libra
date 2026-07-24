---
title: "PLAN-TEST-CLI"
aliases:
  - "PLAN-TEST-CLI"
tags:
  - plans
  - architecture
type: plan
---

# `libra test` — the centralized SDK test CLI

**Status: in progress (v1).** Implements the `libra test` line of
[[18.1-cli|§18.1]] and the coverage / traceability contract of
[[15.8-ci-shape|§15.8]]. Until now the only way to run the suite was
`cargo test --workspace`; this makes one command the transparent front
door to all five [[15-testing|§15]] test kinds.

## Goal

A vitest-shaped runner over the workspace: fast filtering, per-component
runs, a grouped pass/fail/timing reporter, coverage on by default, and a
watch loop — so contributors and app authors stop reaching for raw
`cargo` incantations. This realizes an existing decision; it invents no
new framework surface.

## Foundation (decided)

Wrap **`cargo nextest`** (filtering, parallelism, machine output) +
**`cargo llvm-cov`** (coverage). `libra test` detects both at start:

- both present → full experience (nextest reporter + coverage).
- missing → print a one-line install hint
  (`cargo install cargo-nextest cargo-llvm-cov`) and **fall back to plain
  `cargo test`** (streamed output + tallied footer, no coverage). Never a
  hard dependency — the command runs on a bare checkout.

This matches the "managed but ejectable" CLI ethos of [[18.1-cli|§18.1]].

## CLI surface

```
libra test [FILTER…]              substring filter on test names (`libra test Button`)
  -p, --package <crate>           scope to one crate/app (repeatable)
  --kind <k>                      unit | component | integration | snapshot
  --component <Name>              gallery workbench + component tests for one component
  --coverage[=actions|full|off]   default = actions; off skips instrumentation
  --no-coverage                   alias for --coverage=off
  --watch                         re-run on file change (coverage forced off)
  --validate                      LIBRA_VALIDATE=1 (re-record snapshots)
  --trace-matrix[=<out>]          emit REQ→test→artifact JSON+HTML (§15.8)
  -- <extra nextest args>         escape hatch
```

**Mappings**
- `--kind` → nextest filter on the binary/test path: `unit`→lib tests,
  `component`→`tests/components.rs` (and the gallery workbench),
  `integration`→`tests/integration.rs`, `snapshot`→`tests/snapshots.rs`.
- `--component Slider` → substring `slider` over the gallery workbench
  tests (named `snapshot_workbench_<component>_<theme>`) plus any
  `tests/components.rs` case matching the name.
- Positional `FILTER` → nextest `-E 'test(~<filter>)'`; multiple filters
  OR together (vitest semantics).

## Coverage default

`--coverage=actions` (default) reports the **actions-layer** number per
app plus overall line %. The actions layer is where meaningful behavior
coverage lives ([[15.8-ci-shape|§15.8]]): the runner filters the
llvm-cov export to each app's `actions.rs` / `#[libra::actions]` impls.
`--coverage=full` reports every crate (the CI / pre-release path).
**Watch forces coverage off** — instrumented rebuilds are too heavy for
an iterate loop, and vitest likewise treats coverage as the non-watch
path.

## Traceability — `#[libra::verifies("REQ-…")]`

A **pass-through** attribute macro (added in `libra-reflect`, re-exported
via the `libra` facade) that expands to its item unchanged. The
requirement string is read by `libra test --trace-matrix` from **source**
(grep), not at runtime — no registry plumbing. The matrix correlates
each annotated test to its artifact (snapshot PNG from
`assert_snapshot("<name>")` → `examples/<app>/tests/snapshots/<name>.png`,
or replay hash) and emits `trace-matrix.json` + a self-contained HTML
table.

## Not in v1

- `--replay <file>` ([[18.1-cli|§18.1]]) — replay tests
  ([[15.6-replay-tests|§15.6]]) aren't built yet; the flag is reserved,
  not implemented.
- nextest's `libtest-json` output is experimental; the reporter keeps a
  stable fallback (parse default nextest output / plain `cargo test`) so
  we never hard-depend on the unstable format.

## Stages

1. **Vault** (this note + [[18.1-cli|§18.1]] / [[15.8-ci-shape|§15.8]]).
2. `#[libra::verifies]` pass-through attribute + facade re-export.
3. `cli/libra/src/test/` module: args → tool detection → reporter →
   coverage → watch → trace.
4. Wire the `test` arm into `main.rs`; add `serde` / `serde_json` to the
   CLI manifest.
5. CLI unit tests (arg parsing + kind mapping + fallback selection),
   end-to-end verification with nextest + llvm-cov installed.

← [[Plans Index]] · [[18.1-cli|§18.1]] · [[15.8-ci-shape|§15.8]]
