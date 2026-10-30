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

## In code

- `crates/libra-platform/src/automate.rs` — JSON-line verbs, unix socket
- `cli/libra/src/dev.rs` — sets `LIBRA_AUTOMATION=1` on the debug run

## Implemented today

`libra dev` turns the server on for debug builds. The shell listens on a unix socket (mode `0600`) under the temp dir and prints `[libra] automation <path>`. Release builds ignore the variable. One JSON object per line, answered with one line: `{id, ok, ...}`.

| Verb | Body |
|---|---|
| `snapshot` | Semantics tree with geometry (`testId`, `role`, `text`, `x`, `y`, `w`, `h`) |
| `query` | Same nodes, filtered by `testId`, `text`, and/or `role` |
| `dispatch` | `action` (camelCase) plus optional `arg`. A name that does not resolve is `ok: false` |
| `state` | Bindable fields as strings. `#[libra(redact)]` values are absent |
| `schema` | `actions`, `fields` (not redacted), `redacted` (names only) |
| `log` | Recent actions (`Debug` text). Identical neighbors collapse (`Ticked × N`). Recorded when the inspector or the socket is on |

`input`, `screenshot`, `perf`, the release manifest gate, and `libra skills` are still open. Windows has no listener yet (the shell logs that it needs a unix socket).

## Contract

- Because dispatch is *typed actions*, an agent doesn't scrape pixels to press a button — it can act at the semantic level, with the pixel level available for verification once `screenshot` lands. `libra skills` will emit an agent-skill bundle documenting the app's action schema, view ids, and protocol — the app self-describes.
- Security: off in release; local unix socket at mode `0600`; `#[libra(redact)]` state fields never serialize their values (`schema.redacted` lists the names).

---

---

← [[Architecture Home]] · [[Map of Content]]
