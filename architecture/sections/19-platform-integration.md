---
title: "19. Platform Integration"
section: "19"
aliases:
  - "§19"
  - "19"
  - "19. Platform Integration"
tags:
  - architecture
  - section/19
type: architecture
---

# 19. Platform Integration

## In this vault

- [[19.1-touch]]
- [[19.2-keyboard-shortcuts-declarations-not-handlers]]

## In code

- `crates/libra-platform/src/lib.rs` — the winit/softbuffer windowed shell

---

| Concern | Crate / approach |
|---|---|
| Windows & event loop | `winit` (also: multi-window, monitors, DPI) |
| Framebuffer present | `softbuffer` |
| Menus (native menubar, context) | `muda` |
| System tray | `tray-icon` |
| Native file/message dialogs | `rfd` |
| Clipboard | `arboard` |
| Notifications | `notify-rust` / platform APIs |
| App directories | `directories` |
| Global hotkeys | `global-hotkey` |

Policy: window *chrome*, menus, and dialogs are native (they must feel like the OS); window *content* is Libra-rendered (it must be deterministic). DPI: layout in logical pixels; physical rounding once at paint ([[08-layout|§8]]); per-monitor scale changes are just damage. IME caveats per platform tracked in [[25-risks|§25]].

## Pointer affordances (the `cursor:` analog)

The mouse cursor is an OS resource, not a painted pixel — so it is native chrome by the policy above, and it is off the determinism contract by construction (nothing enters the display list).

It is **derived, never authored**. There is no `cursor="pointer"` markup attribute: the shape is a function of what an item *is*, so `LaidTree::cursor_at(x, y)` reads the same laid tree the click path reads, and an affordance can never promise an interaction the click path won't perform. The alternative — a decorative attribute, CSS's mistake — adds a second, hand-maintained source of truth that silently drifts from behavior and would widen the closed grammar ([[06.2-closed-grammar|§6.2]]) for something already knowable.

| Under the pointer | `Cursor` | Why |
|---|---|---|
| Nothing interactive; static text | `Default` | Libra text isn't selectable — an I-beam would advertise a gesture that does nothing |
| Anything in the press slot (Button, ListItem, toggles, crumb, page button, dismissible scrim) | `Pointer` | A press dispatches its handler |
| `Input` / `Textarea` (`Role::TextInput`) | `Text` | A press places a caret instead of dispatching ([[10.1-text-entry|§10.1]]) |
| A `<Split>` divider gutter | `ColResize` | A press starts a drag |
| A `<Slider>` track | `Pointer` | Click-to-set / drag |
| A disabled Pagination arrow | `NotAllowed` | Present but inert |

`cursor_at` deliberately mirrors `hit`'s topmost-first walk *and* its clip and modal-barrier rules: a row scrolled out of its viewport or sitting behind a `<Dialog>` scrim advertises nothing, because it can't be clicked either. Its two extra arms are the pointer targets that live outside the press slot (divider gutter, slider track) — the same pair `Shell::click` special-cases.

The shell owns only the translation and the timing: `cursor_icon()` maps the closed set to winit's `CursorIcon`, and `Shell::sync_cursor` re-derives it on `CursorMoved` and after every relayout (a dispatch can change the affordance under a still pointer), applying it to the window only on change. An active drag pins its shape — the pointer routinely leaves the gutter mid-resize, and flickering back to an arrow there reads as a dropped grab.

Harness verbs: `cursor_over(sel)` / `cursor_at(x, y)` make it a component-level assertion ([[15.3-component-tests|§15.3]]) — the one interaction detail snapshots structurally cannot cover.

**Staging:** effect wiring + SafeArea / share / open-URL — [[PLAN-NATIVE-INTEGRATIONS]] (under [[PLAN-UI-SURFACE]]). Notifications — [[PLAN-NOTIFY]]. Tray/menubar, close-to-tray, single instance, launch-at-login — [[PLAN-RESIDENCY]]. Global hotkeys remain unclaimed. Catalog components that need OS sheets return effects; they never call platform APIs from `update`.

**Where native chrome meets the data flow:** OS-owned surfaces are outside the paint contract but *not* outside the one-way flow — their contents are declared as data (a pure `TrayDecl` projection, the [[19.2-keyboard-shortcuts-declarations-not-handlers|§19.2]] shortcut table) with action names validated at startup, and their clicks enter through the effects sink as ordinary actions, the same route AccessKit `Click`s already take ([[17-accessibility|§17]]).

---

← [[Architecture Home]] · [[Map of Content]]
