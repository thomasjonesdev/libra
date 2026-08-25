---
title: "20. Packaging & Distribution"
section: "20"
aliases:
  - "§20"
  - "20"
  - "20. Packaging & Distribution"
tags:
  - architecture
  - section/20
type: architecture
---

# 20. Packaging & Distribution

Libra apps ship as native binaries wrapped by **`libra build`** (release artifact) and **`libra package`** (OS bundles). The CLI wraps Cargo + [cargo-packager](https://docs.rs/cargo-packager); identity and icons come from **`libra.toml`** beside the app crate — not from runtime environment lookups.

## Commands

| Command | Purpose |
|---|---|
| `libra dev -p <crate>` | Debug run; hot-reloads `.lx`; uses `[package].icon` when present |
| `libra build -p <crate>` | Release binary (view codegen + LTO) → `target/release/<crate>` |
| `libra package -p <crate>` | Platform bundle from `libra.toml` → `<app>/dist/` |
| `libra package … --target macos\|windows\|linux` | Host must match target (cross-compile deferred) |
| `libra package … --format <fmt>` | Override default format(s) for the target |

Install the CLI once: `cargo install --path cli/libra --force`.

**Packaging** also requires `cargo install cargo-packager --locked` on the build machine.

## `libra.toml` v1

Minimal manifest beside the app `Cargo.toml`. Paths are relative to the manifest file.

| Section | Fields | Notes |
|---|---|---|
| `[app]` | `name`, `bundle_id`, `app_user_model_id` | Display name + OS identity (NT3/R5 consumers) |
| `[package]` | `icon` | Square PNG; packager derives `.icns` / platform sizes |
| `[package.sign.macos]` | `identity`, `notary_profile`, `provider_short_name` | Optional; unsigned OK for local sharing |
| `[package.sign.windows]` | `thumbprint` | Optional Authenticode signing |
| `[package.sign.linux]` | — | Reserved (deb/AppImage signing later) |

`libra package` also writes `packaging/libra-identity.toml` (gitignored) and bundles it as a packager **resource** — `app_user_model_id`, `bundle_id`, and `product_name` for [[PLAN-NOTIFY]] NT3 and [[PLAN-RESIDENCY]] R5 consumers at runtime.

Window title and logical size remain on `libra::app!` for single-window v1 apps ([[26-open-questions|§26]] defers multi-window manifest fields).

Full schema and stage plan: [[PLAN-PACKAGE]].

## Platform outputs

| OS | Default formats | Status |
|---|---|---|
| macOS | `.app` + `.dmg` | **Shipped** |
| Windows | NSIS installer | **Shipped** — run on Windows; `[package.sign.windows].thumbprint` optional |
| Linux | `.deb` + AppImage | **Shipped** — run on Linux |

- Icon pipeline: one square PNG → platform icon sets (no author sizing matrix).
- Signing/notarization: config-file driven via `[package.sign.*]`; CI recipes in the app README scaffold.
- Binary size: single-digit MB (no browser, no JS runtime; bundled fonts dominate).
- Auto-update: **out of scope v0** ([[24-non-goals|§24]]) — cargo-packager can emit update-feed metadata later.

## Reference implementation

**`applications/aws-login`** — end-to-end packaging reference:

- Sample `libra.toml` + `assets/icon.png`
- Developer docs: build, package, sign, test — `applications/aws-login/README.md`
- End-user install paths (DMG / NSIS / deb / AppImage)

## Why packaging is early

Packaging is a **prerequisite**, not only a finishing step:

- **Notifications** ([[PLAN-NOTIFY]] NT3) need a bundled `.app` + consent on macOS and a registered AppUserModelID on Windows.
- **Launch-at-login** ([[PLAN-RESIDENCY]] R5) needs a macOS bundle for `SMAppService`.

Both degrade honestly in dev (`NotifyResult::Unsupported`) rather than pretending — but neither can be *finished* before `libra package` exists.

**Execution plan:** [[PLAN-PACKAGE]] — **complete** (P0 release codegen through P6 Windows/Linux bundles).

---

← [[Architecture Home]] · [[Map of Content]]
