---
title: ADR-001 — Multi-root index from commit #1
type: adr
status: accepted
created: 2026-08-24
tags: [gloss, adr]
---

# ADR-001 — Multi-root index from commit #1

## Context
v1 has one vault. Multi-vault workspaces are a later feature. The tempting move
is to model a single root now and generalize later.

## Decision
Every path in the system is `(RootId, RelPath)`. There is no bare `PathBuf` in
`gloss-core`. The index is multi-root from the first commit even though only one
root will exist for months.

## Consequences
- Slightly more ceremony in every early query.
- Workspaces become a UI feature later rather than a rewrite.
- Cache keys, FTS documents, and vector rows all carry root identity for free.

## What would reverse it
Nothing. Retrofitting means rewriting every query, cache key, and path
assumption in the codebase — the single most expensive retrofit in the project.
