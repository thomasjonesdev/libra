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

- `libra package` wraps `cargo-packager`: `.app`/DMG (macOS), MSI/NSIS (Windows), AppImage/deb (Linux). Icon pipeline from one square PNG.
- Signing/notarization: config-file driven (identities in `libra.toml [package.sign]`), documented per platform; CI recipes ship in the scaffold.
- Binary size expectation: single-digit MB (no browser, no JS runtime; fonts dominate).
- Auto-update: **out of scope v0** ([[24-non-goals|§24]]) — the packager emits update-feed metadata so `cargo-packager`-compatible updaters slot in later.
- **Packaging is a prerequisite, not only a finishing step.** Two OS capabilities refuse to work without real app identity: notifications need a bundled/signed `.app` plus consent on macOS and a registered AppUserModelID on Windows ([[PLAN-NOTIFY]] NT3), and launch-at-login needs a bundle for macOS `SMAppService` ([[PLAN-RESIDENCY]] R5). Both degrade honestly in dev (`NotifyResult::Unsupported`) rather than pretending — but neither can be *finished* before `libra package` exists.

---

---

← [[Architecture Home]] · [[Map of Content]]
