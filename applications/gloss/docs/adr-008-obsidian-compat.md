---
title: ADR-008 — Obsidian vaults are a first-class input
type: adr
status: accepted
created: 2026-08-24
tags: [gloss, adr]
---

# ADR-008 — Obsidian vaults are a first-class input

## Context
The onboarding advantage in [[adr-007-no-fork]] is that Gloss points at an
existing folder and the graph is already there. That advantage only holds if
Gloss reads real Obsidian vaults accurately and can run alongside Obsidian on
the same directory without either app corrupting the other's work.

Obsidian's Markdown dialect is not CommonMark. Wikilinks, block references,
embeds, callouts, nested tags, and inline Dataview fields are all non-standard,
and link resolution is by global filename index rather than path.

## Decision
Obsidian vault compatibility is a baseline constraint on the patch layer, not a
feature. Gloss parses Obsidian's dialect, resolves links exactly as Obsidian
does, treats anything it doesn't fully understand as opaque, and never
reformats. Full spec in [[../08-obsidian-compat]].

## Consequences
- Zero-migration onboarding, and users can keep Obsidian open beside Gloss while
  they decide. This is the main answer to the lock-in objection.
- `SetFrontmatter` must be a surgical byte-range edit rather than a YAML
  parse-mutate-serialize cycle, or every touched note produces a noisy diff.
- The patch layer must fail closed: a file whose parse doesn't round-trip
  byte-identically is untouchable.
- A byte-identity no-op test on a real vault becomes a release gate.
- Community plugin formats beyond Dataview inline fields are out of scope —
  unbounded, unschema'd, and opaque is the correct default.
- Rules out any future storage format that isn't plain Markdown on disk.

## What would reverse it
Nothing. Interoperability is the onboarding story; without it Gloss is just
another editor asking for a migration.
