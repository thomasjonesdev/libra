---
title: "PLAN-NOTES-APP"
aliases:
  - "PLAN-NOTES-APP"
  - "notes parity plan"
tags:
  - plans
  - architecture
type: plan
---

# Notes app — feature & design parity with the native SDK reference

**Status: in progress — M0/M1 landed 2026-07-16.** Grow `examples/notes` from
the Phase 3 text-entry demo (one title field + save + theme toggle) into a
**feature- and design-complete notes app**, at parity with the reference
notes example in
[vercel-labs/native](https://github.com/vercel-labs/native/tree/main/examples/notes)
(`examples/notes`: folders sidebar · note list · editor, keyboard-first,
single-store persistence). Like `dashboard` is the data-plane reference and
`todos` the item-fields reference, **notes becomes the UI-surface reference
app** — the real app that pulls [[PLAN-UI-SURFACE]] work in demand order.

**Mirror target is capability + design quality, not their APIs.** They are
Zig/comptime; we are Libra: every feature must land through the existing
contracts — pure actions ([[05.3-purity-rules|§5.3]]),
closed grammar ([[06.2-closed-grammar|§6.2]]),
effects-as-data ([[12.1-effect-vocabulary|§12.1]]),
deterministic paint ([[appendix-a-determinism-checklist|Appendix A]]).

**Read before writing code:** [[10.1-text-entry|§10.1]],
[[06.3-grammar-restrictions|§6.3]] (`#[project]`),
[[12.5-declarative-subscriptions|§12.5]],
[[PLAN-CATALOG]], [[PLAN-TEXTAREA]], [[PLAN-TOKENS]], [[PLAN-NATIVE-INTEGRATIONS]].

## Feature map (their app → Libra mechanism)

| Reference feature | Libra mechanism | Have it? |
|---|---|---|
| Folders + notes model, soft delete (`deleted_ms`), Recently Deleted | `Vec<Folder>` / `Vec<Note>` state + `<for>` + item-scoped handlers (todos pattern) | **yes** |
| Derived title (first line), preview snippet, word count, filtered lists | `#[project]` hook — recomputed after every action, never stale (§6.3) | **yes** |
| Relative timestamps ("2m / 3h / 1w") that update live | `now_ms` in state ticked by `<Interval every={30000} onTick={tick}>` (Instant payload); labels derived in `#[project]`. Time enters via action payloads — purity holds (§5.3) | **yes** |
| Single-file store, restore before first render | serde whole-store → one `Effect::kv_set` key; `init` does `kv_get` + hydrate. Version field + migration test | **yes** |
| Debounced autosave; exactly one write in flight | `Effect::Timer`/`CancelTimer` debounce + a `save_in_flight`/`dirty` pair in state; re-persist on ack if dirty | **yes** |
| Search (case-insensitive full-text), distinct empty states | query field + `#[project]` filtering; `<if test={…isEmpty}>` empty states | **yes** |
| Multi-line editor, chrome dissolved into background | **`Textarea`** on the pure `EditState` core — [[PLAN-TEXTAREA]] stage 1 | **no — biggest blocker** |
| Scrollable list (reference: model-owned offset) | **`Scroll`** — shipped ([[PLAN-SCROLL]]) with the offset as a **shell-owned deterministic input** (like focus/hover), *deliberately not* the reference's model-owned shape: no scroll action in the log, snapshots at offset 0. Decided 2026-07-19 to keep it; a controlled opt-in (offset echo) is a possible later § if restart-persistent scroll is ever wanted | **yes (adapted)** |
| List rows (icon/name/count · title/snippet/meta), hover + selection washes | **`ListItem`** + `Separator`/`Spacer` — [[PLAN-CATALOG]] waves 1/5 (pulled forward) | no |
| Resizable panes (drag + keyboard), fractions in state | **`Split`** — [[PLAN-CATALOG]] pulled forward; needs shell pointer-drag | no |
| Modal create/rename folder dialog, inline validation; confirm permanent delete | **`Dialog`** in-tree (deterministic, snapshot-able — [[PLAN-NATIVE-INTEGRATIONS]] design rule 3) | no |
| Keyboard shortcuts (Cmd+N, Cmd+1…7, Esc, …) declared app-level | **new**: `shortcuts:` declaration validated against reflected actions; shell dispatches; harness `press_shortcut` | no — needs a small § design first |
| Context menu on rows (menu item dispatches typed action with row id) | in-tree **`ContextMenu`** first (deterministic); native `muda` later ([[PLAN-NATIVE-INTEGRATIONS]] N7) | no |
| Copy note body to clipboard with typed outcome | `Effect::ClipboardSet` — small §12.1 addition ([[PLAN-NATIVE-INTEGRATIONS]]) | no |
| System light/dark auto-follow | **adapted**: keep explicit `Effect::SetTheme` toggle for v1; OS-scheme follow is shell work, deferred to [[PLAN-NATIVE-INTEGRATIONS]] | adapted |
| Fixed capacity caps (6 folders / 48 notes / 4 KiB) | **adapted**: theirs are Zig allocation limits; we adopt soft caps only as store-size hygiene, enforced in actions | adapted |

## Stages

Each stage lands green (`cargo test --workspace`), with the §15 test-kind
split, and updates this plan's status line. Framework work is done **in the
owning plan** (catalog/textarea/tokens/native), pulled in this order — this
plan owns the app and the acceptance bar.

| Stage | Deliverable | Framework pull |
|---|---|---|
| **M0 — done** | Model + projections + persistence: `Folder`/`Note` structs, selection, soft delete, search filter, derived title/preview/word-count/relative-time via `#[project]`; versioned whole-store Kv persistence (`StoreV2` + untagged `StoredFormat` migrating a hypothetical v1 shape) with boot restore; debounced autosave with single-write-in-flight (`save_gen` supersession, `save_pending` re-persist). 22 unit tests (derivation, trash/restore/permanent-delete, folder/note caps, debounce/single-flight, v1→v2 migration, corrupt-store recovery) | none — all shipped |
| **M1 — done** | Three-pane skeleton with today's components: sidebar `<for>` folders (Button rows), list `<for>` filtered notes, single-line editor (interim `Input`), search `Input`, `<if>` empty states ("no notes" vs "no matches"), Recently Deleted visible only when non-empty (`open_recently_deleted` — plain `onPress={x(literal)}` isn't grammar, so it's its own zero-arg action rather than an item-scoped one), `<Interval every={30000} onTick={tick}>` timestamp tick. 6 integration + 8 component + 5 snapshot tests | none |
| **M2 — done except Scroll adoption** | Real list presentation (landed 2026-07-16/17): `ListItem` rows with title/snippet/meta + selection and hover washes in both the folder rail and the note list; hairline seams; spring-pinned add-folder row; `list_summary` footer; **responsive weighted panes** (`grow={1}/{1.5}/{2.5}`). Remaining: wrap the note list + folder rail in the shipped `<Scroll>` ([[PLAN-SCROLL]]) — app-side markup only, zero framework work | [[PLAN-CATALOG]] waves 1 + `ListItem` |
| **M3 — done** | Multi-line editor (landed 2026-07-17): `Textarea` replaced the interim Input (`editor-body`); Enter/Up/Down/newline-preserving paste in the shell; metadata line sits above. Chrome-dissolve styling waits on [[PLAN-TOKENS]] T3 (M8) | [[PLAN-TEXTAREA]] stage 1 |
| **M4 — done** | Keyboard shortcuts (landed 2026-07-19): **[[19.2-keyboard-shortcuts-declarations-not-handlers|§19.2]]** written first — chord grammar, static `Shortcut` declarations, load-time validation, and the focused-precedence rule that keeps §10.1's editing keymap intact. `Shortcut`/`Chord`/`parse_chord`/`build_shortcut_table` in libra-core (pure, shared by shell + harness); `shortcuts:` field in `app!`; shell chord matching; harness `press_shortcut`. Notes declares cmd+n / cmd+backspace / cmd+alt+up/down / cmd+1…6 (`jumpFolder` static args) / escape. Deferred to their blockers: rename + new-folder (M6 Dialog), copy (M7 clipboard) | new shell + `app!` work |
| **M5** | Resizable panes: `Split` with drag + focused-divider keyboard resize; fractions live in state so rebuilds reproduce layout exactly (snapshots stay deterministic) | [[PLAN-CATALOG]] `Split` + shell pointer-drag |
| **M6 — done** | Folder dialogs (landed 2026-07-19): in-tree `<Dialog>` shipped through the catalog flow (both engines, `dialog.lx` fixture, workbench cell, `Role::Dialog` a11y) — full-viewport `theme.scrim` + centered panel, **detached overlay layout** (paints on top from anywhere in markup), scrim as modal hit barrier with optional `onDismiss`. Notes: create/rename folder dialogs with inline empty/duplicate/cap validation (`dialog_error` projection), permanent-delete confirm, `cmd+shift+n`/`cmd+shift+r` shortcuts, and a contextual `dismiss` action on escape (cancel dialog, else clear search). The sidebar's interim inline add-folder row became the reference's bottom-pinned "New Folder" button | [[PLAN-CATALOG]] `Dialog` |
| **M7** | Context menus + clipboard: in-tree `ContextMenu` anchored to rows, items dispatch item-scoped actions (no prior selection needed); `Effect::ClipboardSet` for copy-note with acked outcome | [[PLAN-CATALOG]] `ContextMenu` · [[PLAN-NATIVE-INTEGRATIONS]] clipboard op |
| **M8** | Design pass: token ramps so nothing in the app hard-codes paint (hover/selection washes, hairline separators, muted meta line); light-theme snapshot matrix in gallery; final bless + README hashes | [[PLAN-TOKENS]] T1–T4 |

M0–M1 needed **zero framework changes**, confirmed in practice — the app is
honestly usable (folders, notes, search, autosave, trash) on today's catalog.
M2+ interleave with catalog waves; reorder within M2–M7 freely if a component
lands early, but keep the **one component per change** rule from
[[PLAN-CATALOG]].

**M1 gotcha for later stages:** `ReflectState::field`/`collection`/
`collection_rows` key on the **raw Rust field identifier** (snake_case) —
unlike action/handler names, state field paths get no camelCase conversion
today. Multi-word State fields (`folder_rows`, `has_trash`, …) bind in markup
as written, e.g. `<for each={folder_rows}>`, `{selected_meta}`. Existing
example apps never surfaced this (their bindable fields are single words).
Worth a §3.3/§6.2 clarifying note if a later stage wants markup-side
camelCase for fields — not done here to stay in scope.

## Acceptance (feature-complete bar)

- [ ] Three resizable panes; folders (incl. Recently Deleted behavior), note list with derived title/snippet/relative time, multi-line editor with metadata line
- [ ] Search with live filtering and both empty states; Escape clears
- [x] Debounced autosave, exactly-one-write-in-flight, boot restore, versioned store + migration test
- [ ] Full keyboard operation per the M4 shortcut set; context menus; copy-to-clipboard
- [ ] Create/rename/delete folder via validated dialogs; permanent-delete confirm
- [ ] All five test kinds present and green; snapshot matrix covers dark + light; `cargo test --workspace` green
- [ ] Zero raw paint literals in the app — tokens only ([[PLAN-TOKENS]])
- [ ] Appendix A holds end-to-end: no clocks in actions (`Instant` payloads only), no computation in markup, all I/O as intercepted effects

## Out of scope (v1)

- Native NSMenu/`muda` context menus and OS scheme auto-follow — later stages of [[PLAN-NATIVE-INTEGRATIONS]]
- IME/touch selection ([[PLAN-TEXTAREA]] stages 2–3) — Textarea lands first without them
- Markdown rendering, note linking, sync — not in the reference app either

---

← [[Plans Index]] · [[PLAN-UI-SURFACE]] · [[10.1-text-entry|§10.1]] · [[07.1-builtin-catalog|§7.1]]
