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
`applications/gloss/src/{model,state,actions}` (handlers). Vault bytes still
move only through `Effect::Process` →
`gloss list|cat|apply|undo|backlinks|index|neighbors`. Framework gaps that
look like missing app features are in [[10-libra]].

## Rail (left column)

| Control | Action | Status |
| --- | --- | --- |
| `plus` | `newNote` — create `10-Inbox/Untitled*.md` via patch | **Shipped** (`icon="plus"`) |
| `command` | `openCommandPalette` — quick-open dialog (`Search` + note list) | **Shipped** (catalog `CommandPalette` later) |
| `calendar` | `openDailyNote` — open or create today's `Daily/YYYY-MM-DD.md` | **Shipped** |
| `graph` | `openGraph` — outgoing wikilinks + vault backlink scan | **Shipped** (list, not graph viz) |
| `undo` | `undo` — revert last journaled patch run | **Shipped** |
| `folder` | `openVaultPicker` — dialog of saved vaults + Browse | **Shipped** |
| `settings` | `openSettings` — current vault path + rules | **Shipped** |

## File list (left pane)

| Control | Action | Status |
| --- | --- | --- |
| Filter | `<Search>` → `queryChanged` — live substring filter on paths | **Shipped** |
| Clear `x` | `clearQuery` | **Shipped** |
| Tree row | `selectFile` — folder expand/collapse or open note | **Shipped** |
| Right-click row | `fileContext` then native menu: Open / Open in new tab | **Shipped** (§19.3) |
| `+/` | `newFolder` — `Untitled/Untitled*.md` scaffold | **Shipped** |
| `A` | `toggleSort` — alpha vs tree order | **Shipped** |
| `-` | `collapseAll` | **Shipped** |
| Vault name | current vault stem (picker is on the rail) | **Shipped** |

## Editor strip

| Control | Action | Status |
| --- | --- | --- |
| Back / Fwd | `goBack` / `goForward` — note history | **Shipped** |
| Tab row | `<Tabs>` / `<Tab>` → `selectNote` / `closeTab` / `newTab` | **Shipped** |
| Reader / Raw | `toggleReaderMode` — reader column vs full-file source | **Shipped** |
| `@mention` completion | Type `@note` → dialog → `[[wikilink]]` insert | **Shipped** |

## Modals

| Surface | Action | Status |
| --- | --- | --- |
| Quick open | `palettePick` → `selectNote` | **Shipped** |
| `@mention` | `mentionPick` → silent `[[wikilink]]` | **Shipped** |
| Links drawer | `graphPick` → `selectNote`; backlinks via `gloss backlinks` | **Shipped** |
| Vault picker | rail folder → saved vaults (switch / right-click forget) + Browse | **Shipped** |
| Settings drawer | current vault path + rules excerpt | **Shipped** |

## Keyboard shortcuts (§19.2)

Declared on the app via `Gloss::shortcuts()` — shell dispatches when no focused
text control owns the chord:

| Chord | Action |
| --- | --- |
| ⌘K / ⌘P | `openCommandPalette` |
| ⌘D | `openDailyNote` |
| ⌘N | `newNote` |
| ⌘G | `openGraph` |
| ⌘E | `toggleReaderMode` |
| Escape | `dismiss` — close top overlay / toast |

Cmd+K **inline edit** (review-surface patch return) stays blocked on M3; ⌘K/⌘P
map to quick open for v1.

## Native menus (§19.3)

File (Open Vault, Open Recent, Close Vault), Go (same chords as the
shortcut table), View (Reader/Raw, Settings). Right-click a note for
Open / Open in new tab. Recents come from the Kv workspace list.

## Autosave / I/O

| Path | CLI | Status |
| --- | --- | --- |
| List vault | `gloss list --root DIR` | **Shipped** |
| Read note | `gloss cat --root DIR --path REL` | **Shipped** |
| Save edit | `gloss apply --patch-json …` | **Shipped** |
| Undo | `gloss undo --journal …` | **Shipped** |
| Backlinks | `gloss backlinks --root DIR --path REL` | **Shipped** |
| Index | `gloss index --root DIR --db FILE` | **Shipped** (M1 notes + links) |
| Neighbors | `gloss neighbors --root DIR --path REL --db FILE` | **Shipped** |

Journal: `<vault>/.gloss/journal.json`. See [[03-data-model#Patch and checkpoint format]].
Index DB stays outside the vault. See [[03-data-model#Where things live]].

## Still open for M4 exit

These are the remaining **editor-shell** gaps before “write in Gloss for a week”:

| Function | Blocker | Target |
| --- | --- | --- |
| Soft wrap + IME | PLAN-TEXTAREA stages 2–4 | Libra. This is the week-one pain. |
| `FileRead` / `FileWrite` | sketched in §12.1, not in the enum | Libra. Process-to-self until then. |
| Cmd+K inline edit | Review surface (L2) + Textarea selection in the action | M3 then Libra |
| Full graph view | M1 index + a drawing node we do not have | Not an M4 exit. Drawer is a list. |

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
