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

---

---

← [[Architecture Home]] · [[Map of Content]]
