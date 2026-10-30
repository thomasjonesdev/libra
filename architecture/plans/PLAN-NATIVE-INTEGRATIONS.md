---
title: "PLAN-NATIVE-INTEGRATIONS"
aliases:
  - "PLAN-NATIVE-INTEGRATIONS"
  - "platform effects plan"
tags:
  - plans
  - architecture
type: plan
---

# Native system integrations

**Status: N1–N8 shipped.** Maps Vercel json-render RN **standard actions** and
Libra [[19-platform-integration|§19]] OS concerns onto Libra's
**effects-as-data** model ([[12.1-effect-vocabulary|§12.1]]).
Umbrella: [[PLAN-UI-SURFACE]].

Libra is desktop-native (`winit` + softbuffer). “Native” here means
**host OS capabilities**, not React Native modules. Window *content*
stays Libra-rendered and **identical on macOS, Windows, and Linux**; window
*chrome* and system sheets are OS-owned and must be **explicit on each OS**
([[19-platform-integration|§19]], [[19.4-desktop-os-matrix|§19.4]]). Silent
no-ops are a bug.

## Mirror map (json-render actions → Libra)

Source: `@json-render/react-native` `standardActionDefinitions`.

| json-render action | Libra mapping | Status | Notes |
|---|---|---|---|
| `navigate` / `goBack` | `NavStack` + actions | partial | Already app-level; not an Effect |
| `setState` / `pushState` / `removeState` | pure `Action` / `reduce` | **n/a as Effect** | State changes are never effects in Libra |
| `showAlert` | `Effect::Dialog` | **shipped** (N2) | `rfd` alert and confirm. In-tree `<Dialog>` stays the snapshotable overlay |
| `share` | `Effect::Share` | **shipped** (N4) | Clipboard payload. No OS share sheet |
| `openURL` | `Effect::OpenUrl` | **shipped** (N1) | `/usr/bin/open` / `xdg-open` / `rundll32 url.dll`. `http` `https` `mailto` |
| `refresh` | Http / `libra-query` invalidate | partial | App actions, not a generic effect |

## §12.1 platform effects — staging

Already sketched in the vocabulary; wire through workers/shell + harness
intercept:

| Effect | Crate / approach | Harness |
|---|---|---|
| `Clipboard` | `arboard` (text) | **shipped** — editing-core copy/paste (shell) **and** `Effect::ClipboardSet` with typed `ClipboardResult` ack + `fx::clipboard_set()`/`resolve_clipboard` harness parity (2026-07-19) |
| `Dialog` | `rfd` OS alert / confirm — **shipped** (N2). Linux needs zenity | `fx::dialog()` + `resolve_dialog` |
| `Notify` | `notify-rust` — **staged in [[PLAN-NOTIFY]]** (tag/replace, activation routing, bundle identity) | `fx::notify()` + `resolve_notify` / `activate_notification` |
| `OpenUrl` | host opener — **shipped** (N1) | `fx::open_url()` / `fx::open_url_to` + `resolve_open_url` |
| `SetTheme` | theme table | **shipped** (shell/harness) |
| `WindowOp` | winit | title/fullscreen/resize |
| `Share` | clipboard payload — **shipped** (N4). Same arboard path as `ClipboardSet`. No share sheet | `fx::share()` + `resolve_share` |

## Component-adjacent shell capabilities

Not always Effects — sometimes layout/input plumbing:

| Capability | json-render cue | Libra plan |
|---|---|---|
| Safe area / notches | `SafeArea` | **Shipped** (N5). Insets are a layout input, zero in the canonical frame. A frameless macOS window publishes a 28px title-bar inset; fullscreen clears it |
| Keyboard type / secure entry | `TextInput.keyboardType`, `secureTextEntry` | Input/Textarea attrs → IME hints ([[PLAN-TEXTAREA]]) |
| Modal presentation | `Modal` | In-tree overlay (deterministic) **vs** OS dialog (effect) — both allowed; document when to use which |
| Long-press | `Pressable.longPress` | Pointer + [[19.1-touch|§19.1]] |
| Menus / tray / hotkeys | — | §19 table (`muda`, `tray-icon`, `global-hotkey`) — separate small plans when needed. Tray + window policy + launch-at-login: **[[PLAN-RESIDENCY]]**; global hotkeys still unclaimed |
| File pickers | — | **`Effect::PickFolder` shipped** (2026-08-25, Gloss Browse). **`Effect::PickFile` and `Effect::SaveFile` shipped** — `rfd` open and save sheets, harness `resolve_file_pick` |

## Stages

| Stage | Deliverable |
|---|---|
| **N1** | **Shipped.** `OpenUrl` end-to-end (effect → shell → harness). `http` / `https` / `mailto` |
| **N2** | **Shipped.** `Dialog` (alert + confirm) with `DialogResult` continuation |
| **N3** | `Notify` — split out to [[PLAN-NOTIFY]] (NT1–NT4) |
| **N4** | **Shipped.** `Share` writes the clipboard (text, or text plus url). A native share sheet is not attached |
| **N5** | **Shipped.** `<SafeArea edges?>` pads by shell insets. Harness `set_insets`. Canonical frame is zero, so existing snapshots stay put |
| **N6** | **Shipped.** `WindowOp` Show/Hide/Raise/Quit ([[PLAN-RESIDENCY]] R3) plus `Title`, `Fullscreen`, and `Resize`. A refused title or size is not applied. The harness records chrome; the shell mirrors it onto winit. Fullscreen does not enter the display list |
| **N7** | **Shipped.** `MenuDecl` + `App::menu` / `context_menu` + `app!` fields + harness `click_menu`. Shell builds a `muda` menubar (diff per dispatch) and a native popup after `onMenu` when `context_menu()` is `Some`. Painted `<ContextMenu>` stays for apps that return `None`. macOS and Windows attach. Linux is the N8 typed refusal. A context decl that is empty or has more than one menu is logged and not shown. See [[19.3-native-menus|§19.3]] |
| **N8** | **Shipped.** Linux menus log a typed refusal (winit has no GTK window; GTK is not linked and discarded). Linux clipboard enables `wayland-data-control`. `Options.frameless` calls `with_decorations(false)` on Windows and Linux. macOS keeps the transparent titlebar |

## Design rules

1. **Actions stay pure** — no `Alert::` / `open::` calls inside `update`.
2. **Harness parity** — every new platform effect has `expect_*` /
   `resolve_*` (or explicit “fire and forget” test helper).
3. **Determinism** — OS sheets are outside the paint contract; in-tree
   Modals are inside it and must snapshot.
4. **Closed vocabulary** — new effect variants are a §12.1 change +
   version note, not ad-hoc shell callbacks from components.

## Out of scope

- Replicating RN `Linking` / `Share` API surface in markup
- Letting catalog components invoke platform APIs directly
- Mobile-only APIs (haptics-as-required) until §19.1 touch work needs them

---

← [[Plans Index]] · [[PLAN-UI-SURFACE]] · [[12.1-effect-vocabulary|§12.1]] · [[19-platform-integration|§19]]
