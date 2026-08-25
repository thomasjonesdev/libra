---
title: ADR-002 — All writes through one patch layer
type: adr
status: accepted
created: 2026-08-24
tags: [gloss, adr]
---

# ADR-002 — All writes through one patch layer

## Context
Notes have no verifier. Unlike code, a wrong edit produces no compiler error and
degrades silently over months. The agent will also be editing files a human is
editing concurrently.

## Decision
Every mutation — agent, user, importer, migration — goes through a single patch
layer of structured ops carrying `expect_hash`. No component calls `fs::write`.
Each agent run is one git checkpoint.

## Consequences
- Undo-a-whole-run, per-hunk review, and provenance all become possible.
- Concurrent human/agent editing is safe by construction: a stale patch fails
  rather than clobbers.
- Rejected patches become labelled training data.
- Every feature that writes is blocked on the review surface (M3).

## What would reverse it
Nothing in v1. If a future version earns enough trust for unsupervised writes,
they still go through this layer — trust changes the review policy, not the
write path.
