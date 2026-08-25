---
title: ADR-003 — CodeMirror 6 as editor engine
type: adr
status: superseded
created: 2026-08-24
superseded_by: adr-009-libra-native
tags: [gloss, adr]
---

# ADR-003 — CodeMirror 6 as editor engine

**Superseded by [[adr-009-libra-native]].** Gloss is a Libra app. The v1
editor is `<Textarea>`. CodeMirror 6 is a webview engine and does not run
inside Libra's paint path.

## Original context

Tab-style inline completion is a launch feature. The alternatives are
ProseMirror/Tiptap or Lexical, which give better WYSIWYG prose editing but fight
inline ghost text. Obsidian itself uses CodeMirror 6.

## Original decision

CodeMirror 6.

## What would reverse the supersession

If Gloss leaves Libra for a webview shell, this ADR becomes the editor call
again. Decide before rebuilding M4, not during.
