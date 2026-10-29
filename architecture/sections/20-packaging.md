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
| `libra dev [path]` | Debug run; hot-reloads `.lx`; uses `[package].icon` when present. Path-based — `libra dev examples/counter`, not `-p` |
| `libra build [path] [-p crate]` | Release binary (view codegen + LTO). Pins the workspace manifest. In-repo: `libra build -p aws-login`. Out-of-tree: `libra build` from the app dir (`templates/app`) |
| `libra icon <png> [-o dir]` | Desktop icon set from one square PNG |
| `libra package -p <crate>` | Platform bundle from `libra.toml` → `<app>/dist/`. **Local backup** — never calls GitHub |
| `libra package … --target macos\|windows\|linux` | Host must match target (cross-compile deferred). `--target` selects a bundle format, not a rustc triple |
| `libra package … --format <fmt>` | Override default format(s) for the target |
| `libra publish` | Upload `dist/` and, when configured, `latest.json`. `--ci` writes the workflow. `--no-upload` stops after the local feed |

Install the CLI once: `cargo install --path cli/libra --force`.

**Packaging** also requires `cargo install cargo-packager --locked` on the build machine.

## `libra.toml` v1

Minimal manifest beside the app `Cargo.toml`. Paths are relative to the manifest file.

| Section | Fields | Notes |
|---|---|---|
| `[app]` | `name`, `bundle_id`, `app_user_model_id` | Display name + OS identity (NT3/R5 consumers) |
| `[package]` | `icon` | Square PNG. `libra icon` writes `.icns` / `.ico` beside it; `libra package` passes those through when the files exist |
| `[package.sign.macos]` | `identity`, `notary_profile`, `provider_short_name` | Optional. Absent → ad-hoc sign. Present, with a keychain profile → local notarization. This is the laptop path |
| `[package.sign.windows]` | `thumbprint` | Optional Authenticode signing |
| `[package.sign.linux]` | — | Reserved (deb/AppImage signing later) |
| `[package.publish]` | `provider = "github"`, `repo` | Optional. Absent → nothing to upload. `libra package` still works |
| `[package.update]` | `endpoint`, `pubkey` | Optional. Absent → the shell never checks. The private key stays in `LIBRA_UPDATE_KEY` |

`libra package` also writes `packaging/libra-identity.toml` (gitignored) and bundles it as a packager **resource** — `app_user_model_id`, `bundle_id`, `product_name`, `version`, and, when `[package.update]` is set, `update_endpoint` plus `update_pubkey`. [[PLAN-NOTIFY]] NT3 and [[PLAN-RESIDENCY]] R5 read the identity fields. The shell reads the update fields ([[PLAN-RELEASE]]).

Window title and logical size remain on `libra::app!` for single-window v1 apps ([[26-open-questions|§26]] defers multi-window manifest fields).

Full schema and stage plan: [[PLAN-PACKAGE]].

## Platform outputs

| OS | Default formats | Status |
|---|---|---|
| macOS | `.app` + `.dmg` | **Shipped** |
| Windows | NSIS installer | **Shipped** — run on Windows; `[package.sign.windows].thumbprint` optional |
| Linux | `.deb` + AppImage | **Shipped** — run on Linux |

- Icon pipeline: `libra icon` turns one square PNG (at least 512 px) into png / icns / ico / hicolor. `libra package` passes `icon.icns` and `icon.ico` through when they sit next to the source PNG under `icons/`.
- Signing/notarization: config-file driven via `[package.sign.*]`. On a Mac, `libra package` is enough: the keychain notary profile notarizes, and a missing sign table stays ad-hoc. `libra publish --ci` writes a workflow that runs that same `libra package` on a runner.
- Binary size: single-digit MB (no browser, no JS runtime; bundled fonts dominate).
- Updates: when `[package.update]` is set, `libra publish` uploads a minisign-signed `latest.json` whose first understood target is `darwin-aarch64` (the JSON has a slot per host). The shell fetches it, verifies, confirms, replaces the `.app`, and relaunches. Decline leaves the app running. `LIBRA_NO_UPDATE=1` skips the check. No check runs from `libra dev`.
- Three-OS **content** is identical regardless of where you package ([[19.4-desktop-os-matrix|§19.4]]). Bundles are host-native; the framebuffer is not.

## Local build

`libra package` is the backup. It does not read `[package.publish]`, does not call `gh`, and still notarizes when `[package.sign.macos]` and the keychain profile are present. `libra publish` uploads a `dist/` that command already produced. `libra publish --no-upload` writes `latest.json` beside it and stops. `libra publish --ci` only writes the workflow.

Execution plan: [[PLAN-RELEASE]].

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
