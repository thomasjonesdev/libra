---
title: "24. Non-Goals"
section: "24"
aliases:
  - "§24"
  - "24"
  - "24. Non-Goals"
tags:
  - architecture
  - section/24
type: architecture
---

# 24. Non-Goals

Stated so scope creep has to argue with a document:

- **Not general-purpose.** No ambition to host arbitrary consumer apps; the catalog and grammar serve data-dense ops tools.
- **No plugin/extension system.** Custom Rust components and app-defined effect executors are the only extension points, both inside the determinism contract.
- **No web target, no iOS/Android** in v0.x. Everything above the platform layer (raster, layout, text, loop) is mobile-portable; the gap is platform glue (winit maturity, touch/IME, lifecycle, store packaging) — a Phase 5+ project, not a redesign. Note: Linux touch tablets are NOT mobile in this taxonomy — they are the Linux target plus touch input, in scope from Phase 1.
- **No WebView embedding.** If a surface needs a browser, this is the wrong framework for that surface.
- **No CSS, no stylesheets, no cascade.** Tokens + node attributes only.
- **No GPU renderer until profiling demands it** ([[09.4-gpu-executor|§9.4]]), and never as the canonical test target.
- **No async runtime in the public API.** Concurrency is the runtime's problem; apps see actions.
- **No i18n framework** in v0.x (text *display* is international from day one; *editing* scope is [[10-text|§10]]'s).
- **A signed update feed is in** ([[20-packaging|§20]], [[PLAN-RELEASE]]). The shell checks `latest.json` when `[package.update]` is set, confirms, then replaces the bundle. There is no Sparkle dependency and no updater inside `update()`. Apps that omit the table never check.
- **No attributed editing buffer, no formatting toolbar.** The edit core stays
  `EditState × EditOp` on a plain UTF-8 string ([[10.1-text-entry|§10.1]]).
  No bold button, no block editor, no markdown-as-you-type that mutates
  spans under the caret. That is the death trap §25 is about.
  **Styled display is already in.** `<Markdown>` paints strong, emphasis,
  strike, code, links, and `[[wikilinks]]` as catalog runs. Italic is a
  shipped face (PLAN-GEIST). A caret-aligned ghost run on a still-plain
  buffer (Gloss Tab) is the same kind of paint, not a new editor. Do not
  read this bullet as "Libra cannot draw styled text." It already does.
- **Video is planned, not early** ([[09.5-video|§9.5]]): decode path specced, built Phase 5+ or when a deployment demands it — no longer a hard non-goal.

---

---

← [[Architecture Home]] · [[Map of Content]]
