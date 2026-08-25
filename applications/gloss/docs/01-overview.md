---
title: Overview
type: spec
status: draft
created: 2026-08-24
tags: [gloss, product]
---

# Overview

## Problem

A mature Markdown vault degrades. Notes accumulate faster than they get filed,
links that should exist never get written, the same topic ends up split across
three half-notes, and documentation that describes a codebase silently stops
describing it. The failure is quiet — you discover it months later, when
retrieval fails and you can't find the thing you know you wrote.

Every individual remedy exists as an Obsidian plugin. Semantic search, agentic
edits, auto-tagging, transcription. None of them compose into one experience,
because the plugin API can't own the editing loop.

## Thesis

The Cursor move applies, with one correction: Cursor forked Code – OSS because
VS Code's moat was extensions and settings. Obsidian's moat is a file path.
There is nothing to fork and nothing to migrate — Gloss points at a directory of
Markdown and is instantly compatible. Users can keep Obsidian open beside it.

That makes the bundle, not the fork, the whole product:

- Inline completion that understands the vault, not just the paragraph
- Capture that produces a filed, linked, structured note without syntax
- A relational layer that maintains itself
- All of it reviewable, reversible, and legible in the first session

## Where the analogy breaks — and what follows

**Code has a verifier. Notes don't.** Cursor could be wrong constantly and still
win, because a compiler answers in seconds and the cost of a rejected suggestion
is zero. A bad link or a wrong reorganization in a vault degrades silently and
compounds. Consequence: the agent is *propose-only* far longer than Cursor's
was, and unsupervised background mutation is capped hard. See
[[decisions/adr-002-patch-layer]].

**Willingness to pay runs the wrong direction.** Developers are the highest-WTP
audience in software and Cursor sat on a work budget. Obsidian users
self-selected for free, local, and no vendor. Bundling doesn't fix this — the
wedge has to. See [[decisions/adr-006-audience-of-one]].

**The value moment has to be instant.** Tab was legible in thirty seconds.
Relational upkeep is a slow-burn benefit nobody feels on day one. Capture and
retroactive linking are acquisition; the relational agent is retention.

## The wedge: vault-as-codebase-guide

The narrowest thing with a real buyer attached. Notes reference code symbols;
when the code moves, the note is flagged stale. Expose the vault over MCP and it
becomes the memory layer that coding agents read as context.

This matters for three reasons:

1. It is the thing I already do, so dogfooding is honest.
2. Engineers with a tools budget will pay for documentation that doesn't rot.
   PKM hobbyists will not pay for anything.
3. It needs only [[02-architecture|L0 and L1]]. No editor, no UI, no audio.

## First-session value (the launch demo)

Ordered by how undeniable they are on a vault the user already has:

1. **Retroactive link pass** — "340 connections your notes should have had."
   Proves the relational engine before they type a word.
2. **Inbox amnesty** — 200 orphaned notes become one reviewable diff: titled,
   filed, tagged, linked.
3. **Near-duplicate merge** — three half-notes on one topic, reconciled.
4. **Paste anything → filed note** — URL, PDF, screenshot, thread.
5. **Ask a question, get a *note*** — a durable artifact with citations back to
   sources, not a chat bubble.

## Non-goals

- **Not a fork of anything.** Not Obsidian (closed source, EULA forbids it), not
  Logseq/SiYuan (AGPL, poisons a commercial layer), not VS Code.
- **Not a sync product.** v1 defers to the user's existing sync or git. Building
  sync is a year people consistently underestimate. See [[07-risks]].
- **Not cross-platform in v1.** See [[decisions/adr-004-macos-only]].
- **Not a hosted service in v1.** See [[decisions/adr-005-byok]].
- **Not a custom model in v1.** A trained Tab model is a hosted-era problem.
- **Not a WYSIWYG prose editor.** Libra `<Textarea>` is source-first. A later
  markdown crate may add paint, not a webview editor.
- **No autonomous unsupervised writes. Ever, in v1.**
