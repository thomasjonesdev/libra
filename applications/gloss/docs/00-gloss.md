---
title: Gloss
type: moc
status: draft
created: 2026-08-24
tags: [gloss, project]
---

# Gloss

Point it at your existing vault. It just works. The thesis is not a
better feature. It is a bundle that composes.

What and why: [[11-coherence]].

> [!info] Name
> A gloss is a note written alongside a text to explain it. That is
> the product: notes that explain other notes, and a vault that
> explains a codebase.
>
> It is also a common English word, so collisions are likely. Check
> GitHub, crates.io, npm, `gloss.md` / `glossapp.*`, and USPTO class 9
> before the first public commit.

## The documents

| Doc | What it covers |
| --- | --- |
| [[11-coherence]] | What this is and why. Read first. |
| [[01-overview]] | Same list, plus non-goals |
| [[02-architecture]] | Layers, components, process model, tech choices |
| [[03-data-model]] | Index schema, patch/checkpoint format, config |
| [[04-roadmap]] | Milestones M0–M5, dependency spine, exit criteria |
| [[05-delivery]] | Repo layout, toolchain, testing, CI, release |
| [[06-operations]] | Keys, cost accounting, recovery, telemetry, support |
| [[07-risks]] | Risk register with triggers and mitigations |
| [[08-obsidian-compat]] | What must parse, what must never be touched |
| [[09-app-functions]] | Shipped shell actions, rail map, M4 gaps |
| [[10-libra]] | What is Gloss work vs a Libra framework change |
| [[catalog-handoff]] | Catalog chrome already adopted. Do not rebuild it. |
| [[decisions/index\|Decisions]] | ADRs — the calls that are expensive to reverse |

## Locked decisions

- Building for myself first. No defined paying user yet. — [[decisions/adr-006-audience-of-one]]
- macOS desktop only for v1. — [[decisions/adr-004-macos-only]]
- BYOK inference. No hosted subscription at launch. — [[decisions/adr-005-byok]]
- Multi-root index from commit #1. — [[decisions/adr-001-multi-root]]
- Every write goes through one patch layer. — [[decisions/adr-002-patch-layer]]
- Obsidian vaults are a first-class, byte-safe input. — [[decisions/adr-008-obsidian-compat]]
- Libra-native shell. The editor is `<Textarea>`. Preview is `<Markdown>`. — [[adr-009-libra-native]]
- CodeMirror 6 is superseded. — [[adr-003-codemirror]]

## The one-line thesis

Not a better feature. A bundle that composes, because the plugin API
can't own the editing loop.
