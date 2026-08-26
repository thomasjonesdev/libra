---
title: ADR-009 — Libra-native shell, not Tauri
type: adr
status: accepted
created: 2026-08-25
tags: [gloss, adr]
---

# ADR-009 — Libra-native shell, not Tauri

## Context

The first draft of Gloss specified Tauri v2, React, and CodeMirror 6. Gloss
lives in the Libra repo. Libra already has a windowed shell, a closed view
grammar, intercepted effects, and a multi-line `<Textarea>`. Hosting a webview
editor would step off the determinism contract and fork the product into a
second UI stack.

## Decision

Gloss is a first-party Libra app (`applications/gloss`). The vault core stays
a headless Rust library. Preview is display-only `<Markdown>` via `libra-markdown`.
The source editor is Libra `<Textarea>` until that crate grows inline paint.
Writes leave reduce as `Effect::Process` to the `gloss`
CLI (the same binary, with `list` / `apply` subcommands).

## Consequences

- Tests use the same harness ladder as `aws-login` and `notes`.
- Closed grammar: ranking, hunks, and filters are State projections. The
  view does not compute.
- Ghost-text Tab is Textarea paint on a plain buffer, the same split as
  `<Markdown>`. It is not a hole in §24. See [[10-libra]] and
  [[11-coherence]].
- File I/O is still a framework gap. Process-to-self is the current path.
  `Effect::FileRead` / `FileWrite` are sketched in §12.1 and are not in
  the enum.
- M2 (MCP) must not wait on any of the above.

## What would reverse it

Shipping Gloss as a webview product outside Libra. That would also reverse
[[adr-003-codemirror]] back to accepted.
