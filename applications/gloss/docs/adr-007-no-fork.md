---
title: ADR-007 — Build fresh; fork nothing
type: adr
status: accepted
created: 2026-08-24
tags: [gloss, adr]
---

# ADR-007 — Build fresh; fork nothing

## Context
The originating idea was "Cursor, but for Obsidian." Cursor forked Code – OSS,
which is MIT-licensed. Obsidian's core is closed source and its EULA forbids
reverse engineering and redistribution, so there is nothing to fork. The
open-source alternatives in the category — Logseq, SiYuan, Trilium — are all
AGPL/GPL, which is incompatible with a commercial layer.

## Decision
Build fresh. The vault core is Rust. The shell is a Libra app. See
[[adr-009-libra-native]]. There is still nothing to fork.

## Consequences
- No license entanglement and no upstream merge burden.
- Onboarding is better than Cursor's ever was: no settings to import, no
  extensions to migrate. Point at a folder and the graph is already there.
- Users can run Obsidian alongside while they decide, which removes the
  lock-in objection entirely.
- Costs years of accumulated polish that a fork would have inherited. Accepted —
  the bundle, not feature parity, is the product.

## What would reverse it
Nothing. The legal position is not negotiable.
