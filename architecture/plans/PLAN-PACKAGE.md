---
title: "PLAN-PACKAGE"
aliases:
  - "PLAN-PACKAGE"
tags:
  - plans
  - architecture
type: plan
---

# `libra build` + `libra package` + app manifest (§18, §20)

**Status: in progress (P0–P1 shipped).** Implements the packaging lines of [[18.1-cli|§18.1]] and
[[20-packaging|§20]]. CLI `check`/`dev`/`test` foundations exist; this plan
adds the release build graph and distributable bundles.

**Read before writing code:** [[20-packaging|§20]] (the normative packaging
contract), [[18.1-cli|§18.1]] (CLI surface), [[18.2-iteration-loop|§18.2]]
(codegen runs *only* in release — dev stays on the interpreter),
[[06.4-one-grammar-two-engines|§6.4]] (the dual-engine contract),
[[05.8-app-assembly|§5.8]] (`app!` view bundle and `options()`),
[[appendix-a-determinism-checklist|Appendix A]].

**Dogfood target:** `applications/aws-login` — the first app that needs a
shareable Mac binary and the first consumer of the identity fields that
[[PLAN-NOTIFY]] NT3 and [[PLAN-RESIDENCY]] R5 require on macOS/Windows
([[27.1-aws-sso-session-manager|§27.1]], [[PLAN-AWS-LOGIN]] A8–A10).

## Why the old three-step list was incomplete

The original plan named three deliverables:

1. `libra.toml` manifest
2. `libra package` — release bundle (codegen views, assets, themes)
3. Document install path in README

That list skipped a **hard prerequisite** visible only by reading the code
today:

- `libra-codegen` works — `conformance/build.rs` proves it at build time.
- **Apps do not use it.** Every app embeds views via `include_str!` in
  `libra::app!` and `libra-platform` always calls `libra_interp::resolve`.
- There is no workspace `[profile.release]` LTO policy yet.
- The CLI has no `build` or `package` commands.

Until **P0** lands, `libra package` would wrap a dev-style binary that still
parses `.lx` at runtime — not the release contract [[18.2-iteration-loop|§18.2]]
and [[20-packaging|§20]] describe.

## Goal

A contributor or app author can run:

```sh
libra build -p aws-login      # release binary: codegen views + LTO
libra package -p aws-login    # .app + DMG on macOS (unsigned first; signing optional)
```

…from a `libra.toml` beside the app crate, with one square PNG driving the
icon pipeline, and identity fields ready for downstream native integrations
without retrofit.

## Foundation (decided)

- **Release engine:** `build.rs` per app crate, folding `.lx` through
  `libra-codegen` the same way `conformance/build.rs` does today
  ([[06.4-one-grammar-two-engines|§6.4]]). Component auto-discovery follows
  [[PLAN-COMPOSE]] (PascalCase sibling `.lx` files with a `<props>` root).
- **Runtime switch:** `libra-platform` uses the generated `build_*()` fn in
  release builds and `libra_interp::resolve` in debug builds
  (`#[cfg(debug_assertions)]`). The harness stays on the interpreter — tests
  pass view strings directly and do not need the release path.
- **Packager:** `libra package` wraps **`cargo-packager`** per [[20-packaging|§20]].
  Not a hard dependency of the workspace — detected at CLI start with an
  install hint, same pattern as `cargo-nextest` in [[PLAN-TEST-CLI]].
- **Manifest scope v1:** single-window apps only. `libra::app!` keeps owning
  window size/title via `options()` ([[05.8-app-assembly|§5.8]]); `libra.toml`
  v1 owns **identity + packaging** fields. Multi-window roots stay in
  [[26-open-questions|§26]] — do not implement until an app needs them.
- **Managed but ejectable:** [[18.1-cli|§18.1]] ethos. `libra build`/`package`
  are thin wrappers over Cargo + cargo-packager; `libra eject` (plain Cargo
  workspace you own) remains a separate, later concern — not a gate for this
  plan.

## `libra.toml` v1 schema

Minimal manifest beside the app crate's `Cargo.toml`. All paths relative to
the manifest file.

```toml
[app]
name = "AWS Login"                         # human label; bundle display name
bundle_id = "dev.libra.aws-login"          # macOS CFBundleIdentifier
app_user_model_id = "Libra.AwsLogin"     # Windows toast AppUserModelID

[package]
icon = "assets/icon.png"                   # square PNG; packager derives platform sizes

# Optional — P4; absent ⇒ unsigned bundle (fine for local sharing)
[package.sign.macos]
identity = "Developer ID Application: …"
# notary profile / apple-id fields documented per platform in README scaffold

[package.sign.windows]
# thumbprint or cert path — documented, not required for P3

[package.sign.linux]
# reserved; AppImage/deb signing is P6
```

**Fields deliberately omitted from v1** (do not build ahead):

| Deferred field | Governing note | Why wait |
|---|---|---|
| `[[window]]` roots | [[26-open-questions|§26]] | Multi-window; `app!` suffices for v1 |
| `[permissions.*]` | [[16-automation-server|§16]] | Automation server polish is Phase 4 |
| `[fonts.*]` | [[10-text|§10]] | Bundled defaults work; override later |
| `[theme]` | [[11-theming|§11]] | Apps use `options().theme_file()` today |
| Auto-update feed | [[24-non-goals|§24]] | Explicit v0 non-goal |

**Downstream consumers of identity fields** — design these names now, wire
them in later plans:

| Field | Consumer | Stage |
|---|---|---|
| `bundle_id` | [[PLAN-NOTIFY]] NT3 (macOS consent), [[PLAN-RESIDENCY]] R5 (`SMAppService`) | NT3 / R5 after **P3** |
| `app_user_model_id` | [[PLAN-NOTIFY]] NT3 (Windows toast registration) | NT3 after **P3** |
| `[package.sign.*]` | Gatekeeper / SmartScreen distribution | **P4** optional |

## CLI surface

```
libra build -p <crate>          release binary (codegen + LTO)
libra package -p <crate>        platform bundle from libra.toml
  --target macos|windows|linux  default: host OS
  --format dmg|app|…            default: platform-appropriate (dmg on macOS)
```

Discovery reuses the app-crate walk from `cli/libra/src/dev.rs` (upward search
for a Libra app `Cargo.toml` with `libra` + `libra-platform` deps and
`src/main.rs`). `-p` scopes to a workspace member by crate name.

`libra check` validating `libra.toml` is **not** in this plan — that belongs
to the fuller §18.1 vision. P2 adds a parse-only validator used internally by
`libra package`.

## Stages

| Stage | Deliverable | Gate |
|---|---|---|
| **P0** | App release codegen wiring: shared build helper (extract `conformance/build.rs` pattern), per-app `build.rs`, generated view fn included in release, `libra-platform` cfg switch (interp debug / codegen release). Start with `applications/aws-login`; extend to `examples/*` once the pattern is proven | `cargo build --release -p aws-login` succeeds; release binary does not call `libra_interp::resolve` at runtime; `cargo test --workspace` green (harness unchanged) |
| **P1** | `libra build -p <crate>` CLI command; workspace `[profile.release]` with LTO (`lto = "thin"` or `"fat"` — pick one, document in plan note when landing) | `libra build -p aws-login` produces `target/release/aws-login`; binary size within [[22-performance-budgets|§22]] release budget (< 8 MB for counter-class apps as sanity check) |
| **P2** | `libra.toml` v1 parser + validation (required fields for package, sensible errors); add manifest to `applications/aws-login` with placeholder icon | parser unit tests in `cli/libra`; `libra package` fails clearly when manifest missing or malformed |
| **P3** | `libra package -p <crate>` on **macOS**: `cargo-packager` integration, square PNG → icon pipeline, `.app` + DMG output, unsigned bundles OK | manual: DMG installs, `.app` launches on a clean Mac without Xcode; icon visible in Finder/Dock |
| **P4** | Signing + notarization via `[package.sign.macos]`; documented per-platform setup; CI recipe in app scaffold/README | manual: signed `.app` passes Gatekeeper on a machine that didn't build it |
| **P5** | README install-path docs; sample `libra.toml` in `applications/aws-login`; cross-link [[20-packaging|§20]] | reviewer can follow docs cold |
| **P6** | Windows (MSI/NSIS + AppUserModelID registration hook for NT3) and Linux (AppImage/deb) — after macOS path is stable | platform owners verify one bundle each |

**Parallel work that does *not* block this plan:**

- [[PLAN-NOTIFY]] NT1–NT2 (Linux + activation routing)
- [[PLAN-RESIDENCY]] R1–R4 (tray, window policy, single instance)

**Work that must wait for P3:**

- [[PLAN-NOTIFY]] NT3 macOS/Windows identity wiring
- [[PLAN-RESIDENCY]] R5 macOS launch-at-login
- [[PLAN-AWS-LOGIN]] A8–A10 (notifications + residency)

## P0 implementation notes

The conformance crate is the reference implementation:

- `conformance/build.rs` — enumerates `.lx` fixtures, builds component
  registry, emits `build_<name>()` fns via `libra_codegen::generate_with_registry`.
- Generated code lands in `OUT_DIR`; the app crate `include!`s it.

For apps, the build script must know:

- Root view path (from `app!`'s `view:` field — today duplicated as a literal
  in `build.rs` until a codegen hook or manifest field centralizes it; **acceptable
  duplication for P0**, with a `// must match app! view:` comment).
- State/Action type paths (`crate::State`, `crate::Action` for §5.9-split apps).
- Component `.lx` siblings in the view directory ([[PLAN-COMPOSE]] discovery).

Platform release path (sketch):

```rust
#[cfg(debug_assertions)]
{ /* existing interp path */ }

#[cfg(not(debug_assertions))]
{
    include!(concat!(env!("OUT_DIR"), "/view.rs"));
    build_root(&self.app) // generated fn name fixed by build.rs convention
}
```

Do **not** change `libra::app!`'s `pub const VIEW: &str = include_str!(…)` in P0
— tests and dev hot reload depend on it. Release builds simply stop reading it
at runtime once the platform switch lands.

## Design rules

1. **Codegen only in release.** [[18.2-iteration-loop|§18.2]] is non-negotiable.
   Debug/`libra dev`/`cargo run` never pay proc-macro codegen cost for view edits.
2. **One grammar, two engines.** Any build-script or platform change that could
   diverge interp from codegen must keep `interp_and_codegen_produce_identical_trees`
   green ([[06.4-one-grammar-two-engines|§6.4]]).
3. **Identity fields are data, not env inputs.** Bundle id, AppUserModelID, and
   icon path come from `libra.toml` — never from runtime environment lookups
   that would break determinism or reproducible builds.
4. **Unsigned before signed.** P3 ships unsigned `.app`/DMG; P4 adds signing.
   Do not block local sharing on Apple developer accounts.
5. **Packaging is OS chrome.** Bundles, icons, and DMG layout are outside the
   paint contract ([[19-platform-integration|§19]]). No snapshot re-validation
   required for P0–P5 unless a code change alters pixels.
6. **Soft optional tools.** `cargo-packager` missing ⇒ print install hint and
   exit non-zero with a clear message; never a silent fallback to copying a raw
   binary and calling it done.

## Acceptance

- [x] `cargo build --release -p aws-login` uses codegen, not runtime interp (P0)
- [x] `libra build -p aws-login` is the documented release path (P1)
- [x] `applications/aws-login/libra.toml` exists with identity + icon fields
- [x] `libra package -p aws-login` emits a `.app` and DMG on macOS (P3)
- [ ] Icon from one square PNG appears in Finder/Dock (P3 — manual verify)
- [x] `[package.sign.macos]` path documented; signing optional (P4)
- [x] `bundle_id` and `app_user_model_id` present for NT3/R5 to consume (P2)
- [x] README install-path docs; sample `libra.toml`; cross-link [[20-packaging|§20]] (P5)
- [x] `cargo test --workspace` green at every stage boundary
- [ ] No snapshot PNGs change unless an unrelated pixel change landed in the
      same PR

## Out of scope

- **`libra new` / `libra eject`** — §18.1 vision; apps scaffold by hand today
- **`libra check` + `libra.toml` + a11y audit** — fuller §18.1; parse-only
  validation for `libra package` is in scope (P2)
- **Auto-update** — [[24-non-goals|§24]] v0 non-goal; cargo-packager update-feed
  metadata is a P6+ note only
- **Multi-window `libra.toml`** — [[26-open-questions|§26]] resolved design,
  not v1 packaging
- **Replacing `app!` window/title with manifest** — `options()` remains source
  of truth for single-window apps in v1
- **NOTIFY / RESIDENCY implementation** — this plan *enables* NT3/R5; do not
  implement those effects here

## In code

| Stage | Location |
|---|---|
| P0 | `crates/libra-build/` (new) or shared helper extracted from `conformance/build.rs`; `applications/aws-login/build.rs`; `crates/libra-platform/src/lib.rs` (cfg switch) |
| P1 | `cli/libra/src/build.rs`; root `Cargo.toml` `[profile.release]`; `cli/libra/src/main.rs` |
| P2 | `cli/libra/src/manifest.rs` (parse/validate `libra.toml`); `applications/aws-login/libra.toml` |
| P3 | `cli/libra/src/package.rs`; `cli/libra/Cargo.toml` (optional `cargo-packager` invocation via `std::process::Command`) |
| P4 | `cli/libra/src/package/sign.rs`; README scaffold |
| P5 | `README.md`; `applications/aws-login/README` or crate doc |
| P6 | `cli/libra/src/package/` platform modules |

Reference today:

- `conformance/build.rs` — codegen pattern to extract
- `cli/libra/src/dev.rs` — app crate discovery
- `crates/libra-reflect/src/app.rs` — `VIEW` / `options()` / `app!` contract
- `crates/libra-codegen/src/lib.rs` — `generate_with_registry`, `Config`

## When this lands (vault checklist — [[AGENTS#9 How to change the system safely|AGENTS §9]])

1. [[20-packaging|§20]] — no change expected (plan implements existing contract).
2. [[AGENTS]] §8 — move packaging from "still open" to in-progress/done as
   stages land; cite P0–P6 status. **P0–P5 landed** (macOS build/package path + docs).
3. [[Plans Index]] — update status column as stages complete.
4. [[PLAN-NOTIFY]] / [[PLAN-RESIDENCY]] — confirm NT3/R5 dependency on P3 is
   recorded (this plan satisfies RESIDENCY's "PLAN-PACKAGE should record that").

---

← [[Plans Index]] · [[20-packaging|§20]] · [[18.1-cli|§18.1]] · [[PLAN-NOTIFY]] · [[PLAN-RESIDENCY]] · [[PLAN-AWS-LOGIN]] · [[23-roadmap|§23]]
