---
title: App functions
type: spec
status: draft
created: 2026-08-25
tags: [gloss, product]
---

# App functions

What the Libra shell does today, what each control maps to, and what is still
blocked on index / catalog / agent layers. This is the M4 “usable editor” checklist
— not the M2 MCP wedge or M3 review surface.

Implementation lives in `applications/gloss/views/gloss.lx` (chrome) and
`applications/gloss/src/actions.rs` (handlers). Vault bytes still move only
through `Effect::Process` → `gloss list|cat|apply|undo|backlinks`.

## Rail (left column)

| Control | Action | Status |
| --- | --- | --- |
| `+` | `newNote` — create `10-Inbox/Untitled*.md` via patch | **Shipped** |
| `C` | `openCommandPalette` — quick-open dialog (`Search` + note list) | **Shipped** (catalog `CommandPalette` later) |
| `D` | `openDailyNote` — open or create today's `Daily/YYYY-MM-DD.md` | **Shipped** |
| `G` | `openGraph` — outgoing wikilinks + vault backlink scan | **Shipped** (list, not graph viz) |
| `U` | `undo` — revert last journaled patch run | **Shipped** |
| `S` | `openSettings` — vault path + Browse | **Shipped** |

## File list (left pane)

| Control | Action | Status |
| --- | --- | --- |
| Filter | `<Search>` → `queryChanged` — live substring filter on paths | **Shipped** |
| Clear `x` | `clearQuery` | **Shipped** |
| Tree row | `selectFile` — folder expand/collapse or open note | **Shipped** |
| Right-click row | `openInNewTab` | **Shipped** |
| `+/` | `newFolder` — `Untitled/Untitled*.md` scaffold | **Shipped** |
| `A` | `toggleSort` — alpha vs tree order | **Shipped** |
| `-` | `collapseAll` | **Shipped** |
| Browse | `browseVault` — native folder sheet | **Shipped** |

## Editor strip

| Control | Action | Status |
| --- | --- | --- |
| Back / Fwd | `goBack` / `goForward` — note history | **Shipped** |
| Tab row | `<Tabs>` / `<Tab>` → `selectNote` / `closeTab` / `newTab` | **Shipped** |
| Preview / Edit | `togglePreview` — `<Markdown>` vs `<Textarea>` | **Shipped** |
| Preview wikilink click | `followWikilink` — resolve + open | **Shipped** |
| `@mention` completion | Type `@note` → dialog → `[[wikilink]]` insert | **Shipped** |

## Modals

| Surface | Action | Status |
| --- | --- | --- |
| Quick open | `palettePick` → `selectNote` | **Shipped** |
| `@mention` | `mentionPick` → silent `[[wikilink]]` | **Shipped** |
| Links drawer | `graphPick` → `selectNote`; backlinks via `gloss backlinks` | **Shipped** |
| Settings drawer | vault path + rules excerpt (`.gloss/rules.md`) | **Shipped** |

## Keyboard shortcuts (§19.2)

Declared on the app via `Gloss::shortcuts()` — shell dispatches when no focused
text control owns the chord:

| Chord | Action |
| --- | --- |
| ⌘K / ⌘P | `openCommandPalette` |
| ⌘D | `openDailyNote` |
| ⌘N | `newNote` |
| ⌘G | `openGraph` |
| ⌘E | `togglePreview` |
| Escape | `dismiss` — close top overlay / toast |

Cmd+K **inline edit** (review-surface patch return) stays blocked on M3; ⌘K/⌘P
map to quick open for v1.

## Autosave / I/O

| Path | CLI | Status |
| --- | --- | --- |
| List vault | `gloss list --root DIR` | **Shipped** |
| Read note | `gloss cat --root DIR --path REL` | **Shipped** |
| Save edit | `gloss apply --patch-json …` | **Shipped** |
| Undo | `gloss undo --journal …` | **Shipped** |
| Backlinks | `gloss backlinks --root DIR --path REL` | **Shipped** |

Journal: `<vault>/.gloss/journal.json`. See [[03-data-model#Patch and checkpoint format]].

## Still open for M4 exit

These are the remaining **editor-shell** gaps before “write in Gloss for a week”:

| Function | Blocker | Target |
| --- | --- | --- |
| Cmd+K inline edit | Review surface (L2) + patch return | M3 then wire |
| Full graph view | M1 index + layout; drawer is backlink list only | M1+ |

**Landed in this pass:** `@`-mentions (dialog + `[[wikilink]]` emit), `.gloss/rules.md`
read on vault open (excerpt in settings), keyboard shortcuts (§19.2), catalog chrome
(`<Toolbar>`, `<Search>`, `<Tabs>`, `<Toast>`).

## Explicitly not M4

Index rebuild, embeddings, MCP tools, Tab ghost-text, audio capture, review
hunks, OS notifications — see [[04-roadmap]].

## Daily note convention

`openDailyNote` looks for an existing file in order:

1. `Daily/YYYY-MM-DD.md`
2. `Daily Notes/YYYY-MM-DD.md`
3. `10-Inbox/YYYY-MM-DD.md`
4. `Journal/YYYY-MM-DD.md`
5. `YYYY-MM-DD.md`

If none exist, it creates `Daily/YYYY-MM-DD.md` with `# YYYY-MM-DD` body.
Date comes from the host calendar (`date +%Y-%m-%d` on macOS v1).

## Link resolution

Preview navigation, outgoing links, and backlink scan share
`gloss_core::resolve_link_target`: exact relative path → `stem.md` →
case-insensitive stem / suffix match. Same rules as [[08-obsidian-compat#Link resolution]]
for v1 (no `.obsidian/app.json` read yet).
