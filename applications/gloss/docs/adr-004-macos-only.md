---
title: ADR-004 — macOS only for v1
type: adr
status: accepted
created: 2026-08-24
tags: [gloss, adr]
---

# ADR-004 — macOS only for v1

## Context
Cross-platform desktop triples the audio work and forfeits on-device inference
that only exists on Apple silicon.

## Decision
macOS 14.2+ on Apple silicon. Universal binary built, Intel untested.

## Consequences
- Core Audio process taps for system audio, the Speech framework, and MLX for
  local inference are all available — the difference between transcription as a
  feature and transcription as a six-month port.
- A Swift sidecar can own audio instead of fighting cross-platform capture in
  Rust.
- Excludes the Windows/Linux share of the Obsidian audience entirely.
- Keeps `gloss-core` platform-agnostic anyway; only the sidecar is Mac-specific.

## What would reverse it
Demand from a paying segment on Windows. Not before the reassessment gate.
