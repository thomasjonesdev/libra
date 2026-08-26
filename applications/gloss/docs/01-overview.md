---
title: Overview
type: spec
status: draft
created: 2026-08-24
tags: [gloss, product]
---

# Overview

The product list is [[11-coherence]]. This page is the same claims with
the old overview shape around them: problem, then the list, then what
we refuse.

## Problem

A mature Markdown vault degrades. Notes accumulate faster than they get
filed, links that should exist never get written, the same topic ends
up split across three half-notes, and documentation that describes a
codebase silently stops describing it. The failure is quiet. You
discover it months later, when retrieval fails and you can't find the
thing you know you wrote.

Every individual remedy exists as an Obsidian plugin. Semantic search,
agentic edits, auto-tagging, transcription. None of them compose,
because the plugin API can't own the editing loop.

## What Gloss is

**Point it at your existing vault and it just works.** No import, no
migration, no new format. Keep Obsidian open beside it. Every
competitor asks you to move. Gloss asks for a folder path.

**It shows you 340 connections your notes should have had.** Before you
type a word. The vault you already spent years building gets visibly
better in the first sixty seconds. The equivalent of Cursor's Tab
moment, but for the graph.

**Your inbox of 200 orphaned notes becomes one reviewable diff.**
Titled, filed, tagged, linked. The pile everyone is quietly ashamed of,
cleared in an afternoon of accepting hunks.

**Nothing changes without your approval, and every run undoes in one
click.** Notes have no compiler to catch a bad edit. The damage is
silent and compounds for months. Propose-only plus per-run checkpoints
is what makes an agent safe to point at a decade of your own thinking.

**Talk for thirty minutes, get back a filed, linked note.** Not a
transcript dumped in a folder. A structured note that knows what it
connects to, because the graph is right there. Granola proved people
pay for this. Nobody has done it inside a local vault.

**Your docs stop rotting.** Notes reference code. When the code moves,
the note is flagged stale. Documentation that tells you when it's lying
is a thing no PKM tool offers and every engineer has needed.

**Your vault becomes memory for your coding agents.** Expose it over
MCP and Claude Code reads your accumulated context as a first-class
source. You stop pasting background into prompts.

**Relations without syntax.** Type `@`, get the link. Newcomers get a
connected vault without learning wikilinks. Veterans stop
hand-maintaining what should be automatic.

**Your keys, your data, your choice of model.** Local option for
anything sensitive, visible per-operation cost, no subscription. The
audience that chose plain Markdown files chose it for exactly these
reasons.

**One coherent product instead of nine plugins.** All of the above
exists somewhere as a separate plugin nobody has bundled, because the
plugin API can't own the editing loop. That's the whole thesis: not a
better feature, a bundle that composes.

## Non-goals

- **Not an Obsidian clone.** Compat is a folder path
  ([[adr-008-obsidian-compat]]). The UI is how you review the bundle.
- **Not a fork of anything.** Not Obsidian, not Logseq/SiYuan, not
  VS Code. See [[adr-007-no-fork]].
- **Not a sync product.** v1 defers to the user's existing sync or git.
  See [[07-risks]].
- **Not cross-platform in v1.** See [[adr-004-macos-only]].
- **Not a hosted service in v1.** See [[adr-005-byok]].
- **Not a custom model in v1.** A trained Tab model is a hosted-era
  problem.
- **Not a WYSIWYG prose editor.** Source is a string in `<Textarea>`.
  Preview paints markdown styles through `<Markdown>`. See [[10-libra]].
- **No autonomous unsupervised writes. Ever, in v1.**
