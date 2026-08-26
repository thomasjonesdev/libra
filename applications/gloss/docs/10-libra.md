---
title: Libra constraints
type: spec
status: draft
created: 2026-08-25
tags: [gloss, libra, planning]
---

# Libra constraints

The first draft of Gloss assumed Tauri, React, and CodeMirror. ADR-009
threw that away. These notes still talk as if the hard parts are "build
the index" and "build the agent." That is true for M1 and M2. It is
false for anything that has to feel like an editor.

This note is the split. Gloss work stays in `applications/gloss`. Libra
work is a framework change with both engines, a conformance fixture when
the grammar moves, and Appendix A. Do not hide a framework gap inside
an app workaround and call the milestone done.

Read with [[04-roadmap]] and [[adr-009-libra-native]].

## The spine is still right, and we already broke it once

M2 (MCP) is still the first thing I will use every day. It needs L0 and
L1 only. No Libra change. Do not stall it on `File*` or Textarea wrap.

M4 already shipped ahead of M3. I needed a window to open my vault. The
original order said review first, editor second, so the launch demo
could land on a real accept/reject loop. We now have a usable shell and
no review surface. Live with that. Do not spend the next month making
the shell look like Obsidian. The missing product is still M1 then M2
then M3.

## What Libra already gives us

| Capability | Where | Gloss uses it |
| --- | --- | --- |
| Closed `.lx` + harness | §6, §15 | editor chrome, snapshots |
| `<Textarea>` hard newlines | PLAN-TEXTAREA stage 1 | Raw edit |
| `<Markdown>` display-only | `libra-markdown` | Reader mode |
| `<Toolbar>` `<Tabs>` `<Search>` `<Toast>` `<StatusBar>` `<Drawer>` `<Dialog>` `<Split>` `<Icon>` | PLAN-CATALOG wave 7 | chrome. Adopted. Stop rebuilding it. |
| `Effect::PickFolder` | §12.1 | Browse |
| `Effect::Process` | §12.6 | list / cat / apply / undo / backlinks / index / neighbors / suggest-links / `date` |
| `Effect::Http` | §12.1 | BYOK later. One-shot request/response. |
| `Effect::Timer` / `<Interval>` | §12.5 | toast timeout, crude poll |
| Shortcuts | §19.2 | ⌘K ⌘P ⌘D ⌘N ⌘G ⌘E |
| `libra package` | §20 | macOS `.app`. No updater. |

Process-to-self is how vault bytes move today. It keeps reduce pure and
the harness honest. It is also a subprocess per list/cat/apply. Fine
for a dogfood editor. Wrong for an incremental index that has to stay
under 200ms, and wrong for every keystroke save if the vault is large.

## What does not need Libra

These stay in Gloss crates. Build them without waiting on the catalog.

| Work | Crate | Milestone |
| --- | --- | --- |
| Patch layer, path jail, parse | `gloss-core` | M0. Landed. Watcher and `gix` still open. |
| Notes + resolved links, incremental hash, title-mention suggestions | `gloss-index` | M1. Started. |
| Chunks, tantivy, embeddings, hybrid rank, cost ledger | `gloss-index` | rest of M1 |
| MCP stdio tools | `gloss-mcp` | M2 |
| `code_refs` vs git | `gloss-core` / index | R1 |
| Provider trait, prompts, cassettes | `gloss-llm` | M1/M3 |
| Patch propose / accept / reject as data | `gloss-core` | M3 |
| CLI review (`gloss review apply`) | `gloss-cli` | M3 can ship without a hunk widget |
| Audio sidecar | Swift, not Libra | R2 |
| State projections (`tab_rows`, later `hunk_rows`) | `applications/gloss` | authoring rule, not a framework feature |

The closed grammar is the authoring rule the first draft never wrote
down. `.lx` cannot compute. Ranking, hunk splitting, graph walks, and
filter rows live on State. The view binds a projection. If you find
yourself wanting an expression in markup, the work is a `#[project]`
field, not a grammar change.

## Ownership

| Need | Owner | Status | Notes |
| --- | --- | --- | --- |
| Vault bytes in/out of reduce | Libra `Effect::FileRead` / `FileWrite` | sketched in §12.1, **absent from the enum** | Process-to-self until this lands. Jail stays in `gloss-core`. File* takes a `PathBuf`. Pass a path that already survived the jail. |
| Folder picker | Libra | **Shipped** | `PickFolder` |
| File picker | Libra `PickFile` | planned | Paste-a-path can wait. Import a PDF later. |
| Vault watcher | Libra or Gloss poll | open | `notify` in a worker is a new effect. Today: relist on focus or `<Interval>`. Do not `fs` inside reduce. |
| Host clock | Libra (Instant in an action) | workaround | Daily note shells out to `date`. A tick/clock payload is cleaner. Not blocking. |
| Keychain | Libra platform effect | missing | BYOK keys cannot live in the vault and should not ride `Process` argv. `security-framework` behind an interceptable effect, or we cannot test it. |
| HTTP completion | Libra `Http` | **Shipped** (one shot) | Enough for a whole-response complete. |
| Token stream / incremental model output | Libra PLAN-STREAM-DELIVERY | planned | Tab and visible "thinking" need chunks before the process exits. |
| Soft wrap, IME, large paste | Libra PLAN-TEXTAREA 2–4 | stage 1 only | This is the real M4-exit gap. Hard-wrapped source is not "write here for a week." |
| Selection in an action | Libra `libra-text` + Textarea | partial | Cmd+K inline edit needs the current range in the action payload, then a patch, not a string replace of the whole buffer. |
| Ghost-text Tab | Libra Textarea paint | not a §24 hole | Buffer stays a string. Paint a muted run after the caret. Accept is `EditOp`. Same split as `<Markdown>`. Needs paint + a ghost string in state, not a policy amendment. |
| Wikilink paint in the editor | Libra / `libra-markdown` | display only | Preview already paints `[[wikilinks]]`. Source view is a Textarea. Do not start a second parser. |
| Hunk / diff widget | Libra catalog **or** compose | missing | M3 CLI does not need this. M3 in the window does. Prefer `ListItem` + `Chip` + `<if>` first. A `Diff` node only if that repeats badly. |
| Large file list | Libra `VirtualList` / `Tree` | wave 8, unbuilt | Hundreds of notes are fine. Thousands will jank. Not M2. |
| Command palette chrome | Libra `CommandPalette` | planned | Dialog + Search works. Replace later. |
| Graph drawing | Libra `Canvas` or new node | not needed | The G drawer is a list. A force graph is decoration. |
| Image / screenshot paste | Libra `Image` | blocked on decode | First-session "paste a screenshot" is not an app feature until this exists. |
| OS notifications | Libra PLAN-NOTIFY | planned | In-app `<Toast>` only. Daily digest banners wait. |
| Menubar / context menus | Libra §19.3 / N7 | **Started** | `MenuDecl` + `muda`. Gloss File/Go/View + note context. Tray / residency still PLAN-RESIDENCY. |
| Auto-update | neither, for v1 | Libra §24 non-goal | `05-delivery` Sparkle line is a later Gloss choice, not a Libra task. |

## Tab is paint, not a policy fight

§24 was written as if styled text were a future danger. Gloss and
`libra-markdown` already paint it. The remaining fence is the buffer
(a string) and the toolbar (none). Tab is a ghost run on that string.
Amend nothing in §24 to start it. Build Textarea overlay paint when M1
can retrieve neighbors.

Until then, "Tab" in the roadmap means the model and the prompt.

## Per-milestone Libra work

### M0, M1, M2, R1. None required

Index and MCP talk to files from a library. The editor can keep using
Process. A watcher would be nice for M1 incremental refresh while the
app is open. Poll if you have to.

### M3. Review as data first

Ship accept/reject on the CLI against the journal. The launch demo
(retroactive links, inbox amnesty) is a patch run plus a review
command. The window can grow a hunk list from projections when that
gets annoying.

Libra work only when we want in-window hunk chrome that the catalog
cannot compose. Do not invent `Diff` on day one.

Cmd+K inline edit is M3 plus selection, not M3 plus a new editor.

### M4 exit. Write here for a week

The remaining editor pain is Libra, not missing rail buttons.

1. PLAN-TEXTAREA stage 2–4 (soft wrap is the one I will feel on day one)
2. `FileRead` / `FileWrite` so save/open is not a subprocess
3. Optional: clock payload for daily notes

Graph viz is not on this list.

### M4.1. Mostly done

`libra-markdown` + `<Markdown>` already renders the fixture kinds the
exit names. Remaining misses (wikilink click already dispatches) are
parser gaps in that crate, which is Libra, but they are small.

### M5. Libra first, then the model

Ghost paint and selection on a plain buffer. Then the model and the
cache prefix. Do not train against a Dialog that inserts a paragraph.
That measures the wrong thing. §24 does not need to move.

### R2. Swift plus M3, not Libra

Mic and system-audio taps stay out of the raster path. Filing the
transcript is a patch plus review.

## Rules for changing Libra on Gloss's behalf

1. If M2 can ship without it, it waits.
2. Grammar change: both engines + `conformance/` fixture. No exceptions.
3. New `Effect` variant: workers or shell, plus harness `fx::*` /
   `resolve_*`. Copy `PickFolder` / `Process`, not a side door.
4. Paint/text change: Appendix A. Snapshots will move. Validate on
   purpose.
5. Do not add a catalog node that `.lx` composition already expresses.
6. Do not put vault writes anywhere except the patch layer, including
   inside a new File* helper.

## What I will not wait for

CommandPalette. Chip facets. Image. Tree. Canvas graph. OS banners.
Residency. Sparkle. IME on day one of dogfood (soft wrap first). A
custom Tab model.
