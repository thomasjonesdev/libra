---
title: "16. The Automation Server"
section: "16"
aliases:
  - "§16"
  - "16"
  - "16. The Automation Server"
tags:
  - architecture
  - section/16
type: architecture
---

# 16. The Automation Server

Every [[15-testing|§15]] kind tests *inside* the process. The automation server drives the *real running app* from outside — the layer that makes Libra apps legible to external harnesses and AI agents.

- Enabled by `libra dev` (and `--automation` on release builds when the manifest permits). Listens on a local unix socket / named pipe; JSON-line protocol.
- **Verbs**: `snapshot` (the full semantics tree with geometry, as JSON), `query` (selector → node ids), `dispatch` (typed action by public camelCase name + payload — validated against the reflected `Action` schema, [[03.3-naming-conventions|§3.3]]), `input` (synthetic pointer/key at coordinates), `screenshot` (deterministic PNG of the live framebuffer), `state` (redacted-by-annotation state dump), `perf` (frame timings).
- Because dispatch is *typed actions*, an agent doesn't scrape pixels to press a button — it can act at the semantic level, with the pixel level available for verification. `libra skills` emits an agent-skill bundle documenting the app's action schema, view ids, and protocol — the app self-describes.
- Security: off in release unless manifested; local-only; peer-credential checked; `#[libra(redact)]` state fields never serialize over it.

---

---

← [[Architecture Home]] · [[Map of Content]]
