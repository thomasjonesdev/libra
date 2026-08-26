---
title: What and why
type: spec
status: draft
created: 2026-08-25
tags: [gloss, product]
---

# What and why

This is the product. If a feature does not serve one of these, it waits.
Roadmap and milestones live in [[04-roadmap]]. This page is the why.

## Point it at your existing vault and it just works

No import, no migration, no new format. Keep Obsidian open beside it.
Every competitor asks you to move. Gloss asks for a folder path.

See [[adr-008-obsidian-compat]] and [[adr-007-no-fork]].

## It shows you 340 connections your notes should have had

Before you type a word. The vault you already spent years building gets
visibly better in the first sixty seconds. The equivalent of Cursor's
Tab moment, but for the graph.

This is the first-session proof. It needs the index ([[04-roadmap|M1]])
and a review surface ([[04-roadmap|M3]]). It does not need a cloned
editor. The first local signal is already queryable: title mentions that
are not yet wikilinks (`gloss suggest-links`). Embeddings and the
review surface come next.

## Your inbox of 200 orphaned notes becomes one reviewable diff

Titled, filed, tagged, linked. The pile everyone is quietly ashamed of,
cleared in an afternoon of accepting hunks.

## Nothing changes without your approval

Every run undoes in one click. Notes have no compiler to catch a bad
edit. The damage is silent and compounds for months. Propose-only plus
per-run checkpoints is what makes an agent safe to point at a decade of
your own thinking.

See [[adr-002-patch-layer]].

## Talk for thirty minutes, get back a filed, linked note

Not a transcript dumped in a folder. A structured note that knows what
it connects to, because the graph is right there. Granola proved people
pay for this. Nobody has done it inside a local vault.

Audio is [[04-roadmap|R2]]. Filing still goes through the same review
loop.

## Your docs stop rotting

Notes reference code. When the code moves, the note is flagged stale.
Documentation that tells you when it's lying is a thing no PKM tool
offers and every engineer has needed.

That is [[04-roadmap|R1]].

## Your vault becomes memory for your coding agents

Expose it over MCP and Claude Code reads your accumulated context as a
first-class source. You stop pasting background into prompts.

That is [[04-roadmap|M2]]. First daily use. No more editor chrome
required.

## Relations without syntax

Type `@`, get the link. Newcomers get a connected vault without
learning wikilinks. Veterans stop hand-maintaining what should be
automatic.

## Your keys, your data, your choice of model

Local option for anything sensitive, visible per-operation cost, no
subscription. The audience that chose plain Markdown files chose it
for exactly these reasons.

See [[adr-005-byok]] and [[06-operations]].

## One coherent product instead of nine plugins

All of the above exists somewhere as a separate plugin nobody has
bundled, because the plugin API can't own the editing loop. That's
the whole thesis: not a better feature, a bundle that composes.

Owning the loop is why Gloss is a Libra app with one patch layer, not
an Obsidian plugin and not an Obsidian skin. Compat is a folder path.
The UI is how you review the bundle. Matching Obsidian is not the
work.

## What this is not

An Obsidian clone. Sync. A hosted subscription. Unsupervised writes.
A markdown editor whose job is to look like the thing you already
have open in the other window.
