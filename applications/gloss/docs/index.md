---
title: Decisions
type: moc
status: draft
created: 2026-08-24
tags: [gloss, adr]
---

# Decisions

ADRs for the calls that are expensive or impossible to reverse. Format is
deliberately short: context, decision, consequences, and what would reverse it.

| ADR | Decision | Status |
| --- | --- | --- |
| [[adr-001-multi-root]] | Multi-root index from commit #1 | Accepted |
| [[adr-002-patch-layer]] | All writes through one patch layer | Accepted |
| [[adr-003-codemirror]] | CodeMirror 6 as editor engine | Superseded |
| [[adr-004-macos-only]] | macOS only for v1 | Accepted |
| [[adr-005-byok]] | BYOK inference, no hosted subscription | Accepted |
| [[adr-006-audience-of-one]] | Build for myself first | Accepted |
| [[adr-007-no-fork]] | Build fresh; fork nothing | Accepted |
| [[adr-008-obsidian-compat]] | Obsidian vaults are a first-class input | Accepted |
| [[adr-009-libra-native]] | Libra app, not Tauri + CodeMirror | Accepted |
