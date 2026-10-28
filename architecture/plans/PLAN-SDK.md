---
title: "PLAN-SDK"
aliases:
  - "PLAN-SDK"
tags:
  - plans
  - architecture
type: plan
---

# Out-of-tree SDK consume (§18, §20, §21)

**Status: S1–S4 shipped.** Published to crates.io as 0.1.0 on 2026-10-26, and as 0.1.1 on 2026-10-27 with the §18.2 iteration loop.
Apps can live outside this repo. This plan makes Libra a crate you
depend on from another repository, without changing the four bets or
the closed sets.

**Read before writing code:** [[18.1-cli|§18.1]] (CLI surface),
[[05.8-app-assembly|§5.8]] (`app!` + view bundle),
[[20-packaging|§20]] (app bundles, not crate publishing),
[[21-crate-layout|§21]] (intended facade + `libra-test`),
[[06.4-one-grammar-two-engines|§6.4]],
[[12.1-effect-vocabulary|§12.1]],
[[26-open-questions|§26]] #7 (crates.io name),
[[appendix-a-determinism-checklist|Appendix A]],
[[27-first-party-applications|§27]] (first-party apps stay in-tree as
dogfood).

**Not this plan:** plugin system ([[24-non-goals|§24]]), a new grammar,
custom Effect variants, Phase 4 inspector/LSP ([[PLAN-PHASE4-DX]]).
`libra new` / `libra eject` were explicitly out of [[PLAN-PACKAGE]];
they move here as S3.

## Why this is four stages

The authoring contract compiles outside the workspace — macros emit
`::libra::` (S2) and `libra new` writes a project (S3). What still
fails an out-of-tree author is crates.io (S4). Do not "just publish
the crates."

## Goal

```sh
mkdir ~/src/myapp && cd ~/src/myapp
libra new .          # S3 — S1 is copy templates/app
libra dev
libra test
libra package
```

with a `Cargo.toml` that names `libra-ui` + `serde` (S2/S4) from a
registry, and no framework checkout on disk. Rust code still says
`use libra::`.

## Foundation (decided)

- **First-party apps stay in-tree** ([[27-first-party-applications|§27]]
  rule 5). gloss and aws-login remain dogfood; they should eventually
  import only the public surface `libra new` generates (S2+).
- **`templates/app` is not a workspace member.** It is listed in root
  `exclude` so it can live in this checkout without a dummy
  `[workspace]` table. `find_workspace_root` treats a non-member
  package as its own implicit workspace (S3).
- **Codegen stays in the app `build.rs`** ([[PLAN-PACKAGE]] P0). The CLI
  does not own the view engine.
- **Git and crates.io.** Path/git deps still work. Fonts are inside
  `libra-raster`, so a crates.io build does not need the checkout.
  Package names: `libra-ui`, `libra-ui-core`, `libra-ui-cli` (§26 #7).
- **`#[non_exhaustive]` before first publish** (S4). `Effect`,
  `ViewNode`, `IconName`, `Tone` are closed and currently exhaustive;
  [[12.1-effect-vocabulary|§12.1]] already says the vocabulary grows.

## Stages

### S1 — Out-of-tree by path/git (this change)

Committed `templates/app`: the five-file app plus tests, `libra.toml`,
icon, and the profile tables an out-of-tree workspace does not inherit.
`libra build` pins `--manifest-path` to the resolved workspace the way
`libra package` already does. Docs stop claiming `libra dev -p`.

**Done when:** `cd templates/app && cargo test && libra check . && libra build`
works against the path deps in that crate, and
`resolve(templates/app)` succeeds from the CLI tests.

### S2 — One authoring dep (shipped)

`crates/libra` re-exports `Options` / `ViewDriver` / `run_with` (feature
`app`) and hides the rest under `__private`. Workspace `libra` is
`default-features = false` so conformance can derive without winit;
apps pass `features = ["app"]`. `libra-reflect` emits `::libra::`.
`libra-codegen` emits crate-relative `libra_components::` /
`libra_core::`; `app!` aliases them through the facade so release
`view.rs` does not need those crates as direct app deps. In-tree extra
deps keep compiling.

**Done when:** `templates/app` `[dependencies]` is `libra` + `serde`,
and omitting `libra-components` is no longer a release-only `E0433`.

### S3 — CLI owns the project (shipped)

`libra new` writes the S1/S2 template. `find_workspace_root` falls back
to the package directory so `build` / `package` / `test` work with no
`[workspace]` line. Hot reload polls the view *directory*. `libra test`
coverage/trace scan the workspace root, not hardcoded `crates/` +
`examples/`.

**Done when:** `libra new myapp && cd myapp && libra dev` works against
path (this checkout) or `--git` with zero hand-edits. `libra eject`
waits — with the CLI still a cargo wrapper there is nothing to eject
from.

### S4 — Publishable and stable

Fonts live inside `libra-raster` (same bytes as `assets/fonts/`).
Internal path deps carry `version = "0.1.0"`. `publish = false` on
examples, applications, conformance. Apache-2.0 `LICENSE`, repository,
and per-crate descriptions. `#[non_exhaustive]` on `Effect`, `ViewNode`,
`IconName`, and `Tone`. `cargo test --workspace` CI. [[26-open-questions|§26]] #7
resolved: publish as `libra-ui` / `libra-ui-core` / `libra-ui-cli`
(`libra`, `libra-core`, and `libra-cli` are taken). 0.x snapshot bytes
are not a semver contract (README, [[21-crate-layout|§21]]).

**Done when:** `cargo publish --dry-run` passes the spine in dependency
order, and a fresh repo with `libra-ui = "0.1"` builds with no checkout.
The dry-run of dependents cannot succeed until the leaves are actually
on crates.io — `cargo package --allow-dirty --no-verify` is the local
proof that each tarball is self-contained.

## Design rules

1. **Do not invent consume-path features that change the bets.** Closed
   grammar, Effect-as-data, deterministic paint stay as they are.
2. **S1 does not implement `libra new`.** Copy-the-template is the
   scaffold until S3. PLAN-PACKAGE's "apps scaffold by hand" stays
   true for one more stage.
3. **Do not add `templates/app` to root `members`.** Nested workspace.
4. **Docs match the CLI, not the vision list.** `libra dev` is
   path-based; `-p` is `build` / `package` / `test` only.
5. **Appendix A:** S1 does not move fonts or change paint. S4's font
   relocation must keep snapshot bytes identical.

## Acceptance (S4)

- [x] Fonts byte-identical inside `libra-raster/fonts/` (OFL + notice)
- [x] Workspace path deps carry `version = "0.1.0"`
- [x] `publish = false` on examples, applications, conformance
- [x] `LICENSE`, repository, descriptions
- [x] `#[non_exhaustive]` on `Effect`, `ViewNode`, `IconName`, `Tone`
- [x] §26 #7 resolved (`libra-ui` / `libra-ui-core` / `libra-ui-cli`)
- [x] 0.x snapshot-byte note (README, §21)
- [x] `libra new` emits `libra-ui = "<version>"` when not built from a checkout
- [x] `cargo publish` of the spine at 0.1.0 (`libra-ui`, `libra-ui-core`, `libra-ui-cli`, and the other `libra-*` crates)

## Acceptance (S3)

- [x] `libra new <path>` writes the template; `--path` / `--git` / default
- [x] Generated `Cargo.toml` has no dummy `[workspace]`
- [x] `find_workspace_root` falls back to the nearest `[package]`
- [x] `templates/app` dropped its dummy `[workspace]` table
- [x] Root workspace `exclude = ["templates/app"]` (in-tree nest without a dummy table)
- [x] `libra new` does not copy the starter snapshot PNG (first `libra test` records it)
- [x] View hot reload polls every `.lx` under the view directory
- [x] Coverage / trace-matrix walk the workspace root
- [x] `libra eject` not implemented (nothing to eject from)

## Acceptance (S2)

- [x] Facade re-exports platform entry points + `__private::{components, platform}` (`app` feature; workspace `libra` is default-features = false)
- [x] `#[state]` / `#[actions]` / `app!` emit `::libra::`
- [x] Codegen paths are crate-relative; `app!` aliases them
- [x] `templates/app` `[dependencies]` is `libra` + `serde`
- [x] `validate_libra_app` accepts a facade-only `libra =` line
- [x] In-tree extra deps still compile; apps opt into `features = ["app"]`

## Acceptance (S1)

- [x] `templates/app` committed; not a workspace member
- [x] Template lists `libra` + `libra-core` + `libra-platform` +
      `libra-components` + `serde` + `libra-build` + `libra-test`, with
      comments on why `libra-components` and the profile tables exist
- [x] `libra build` passes `--manifest-path` + `current_dir(workspace_root)`
- [x] Governing notes and CLAUDE.md/README stop documenting
      `libra dev -p <crate>` as the discovery form
- [x] `cargo test -p libra-cli` covers `resolve(templates/app)` and the
      build invocation
- [x] `cargo test --manifest-path templates/app/Cargo.toml` green
- [x] `cargo test --workspace` still green (template is not a member)

## Out of scope (this change)

- `libra eject` (still nothing to eject from)
- Actually running `cargo publish` (needs a crates.io token; names are permanent)
- Widening `validate_libra_app` beyond the `libra-platform` /
  `libra.workspace` substring (template lists `libra-platform`)
- Moving gloss or aws-login out of `applications/`

## In code

| Stage | Location |
|---|---|
| S1 | `templates/app/`; `cli/libra/src/build.rs`; `cli/libra/src/app.rs` (tests); [[18.1-cli\|§18.1]], [[20-packaging\|§20]], [[05.8-app-assembly\|§5.8]], [[21-crate-layout\|§21]] |
| S2 | `crates/libra/src/lib.rs`; `crates/libra-reflect/src/app.rs`; `crates/libra-codegen/src/lib.rs` |
| S3 | `cli/libra/src/new.rs`; `cli/libra/src/app.rs` (`find_workspace_root`); `crates/libra-platform` (view-dir watch); `cli/libra/src/test/{coverage,trace}.rs` |
| S4 | `crates/libra-raster/src/lib.rs`; root `Cargo.toml`; `.github/workflows/`; `crates/libra-core` / `libra-components` enums |

## When this lands (vault checklist — AGENTS §9)

1. [[18.1-cli|§18.1]] — implemented-today vs vision; `dev` is path-based.
2. [[20-packaging|§20]] — command table matches the CLI.
3. [[05.8-app-assembly|§5.8]] — `view:` path matches shipping apps;
   point at `templates/app`.
4. [[21-crate-layout|§21]] — consume list is `libra` + `serde`.
5. [[AGENTS]] §8 — PLAN-SDK S1–S4 shipped (`libra-ui` 0.1.0 on crates.io).
6. [[Plans Index]] — this row.

---

← [[Plans Index]] · [[18.1-cli|§18.1]] · [[20-packaging|§20]] · [[21-crate-layout|§21]] · [[PLAN-PACKAGE]] · [[PLAN-PHASE4-DX]]
