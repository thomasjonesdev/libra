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

**Status: planned.** Maps Vercel json-render RN **standard actions** and
Libra [[19-platform-integration|§19]] OS concerns onto Libra's
**effects-as-data** model ([[12.1-effect-vocabulary|§12.1]]).
Umbrella: [[PLAN-UI-SURFACE]].

Libra is desktop-native (`winit` + softbuffer). “Native” here means
**host OS capabilities**, not React Native modules. Window *content*
stays Libra-rendered and deterministic; window *chrome* and system
sheets may be OS-owned ([[19-platform-integration|§19]] policy).

## Mirror map (json-render actions → Libra)

Source: `@json-render/react-native` `standardActionDefinitions`.

| json-render action | Libra mapping | Status | Notes |
|---|---|---|---|
| `navigate` / `goBack` | `NavStack` + actions | partial | Already app-level; not an Effect |
| `setState` / `pushState` / `removeState` | pure `Action` / `reduce` | **n/a as Effect** | State changes are never effects in Libra |
| `showAlert` | `Effect::Dialog` | planned in §12.1 | Prefer `rfd` / platform alert — [[19-platform-integration|§19]] |
| `share` | `Effect::Share` (add) | planned | Desktop: share sheet / “copy link” fallback per OS |
| `openURL` | `Effect::OpenUrl` | planned in §12.1 | `open` / `xdg-open` / ShellExecute |
| `refresh` | Http / `libra-query` invalidate | partial | App actions, not a generic effect |

## §12.1 platform effects — staging

Already sketched in the vocabulary; wire through workers/shell + harness
intercept:

| Effect | Crate / approach | Harness |
|---|---|---|
| `Clipboard` | `arboard` (text) | **shipped** — editing-core copy/paste (shell) **and** `Effect::ClipboardSet` with typed `ClipboardResult` ack + `fx::clipboard_set()`/`resolve_clipboard` harness parity (2026-07-19) |
| `Dialog` | `rfd` / OS alert | `expect_effect` + `resolve_dialog` |
| `Notify` | `notify-rust` — **staged in [[PLAN-NOTIFY]]** (tag/replace, activation routing, bundle identity) | `fx::notify()` + `resolve_notify` / `activate_notification` |
| `OpenUrl` | OS open | `expect_effect(fx::open_url(…))` |
| `SetTheme` | theme table | **shipped** (shell/harness) |
| `WindowOp` | winit | title/fullscreen/resize |
| `Share` *(new)* | platform share / clipboard fallback | intercept like OpenUrl |

## Component-adjacent shell capabilities

Not always Effects — sometimes layout/input plumbing:

| Capability | json-render cue | Libra plan |
|---|---|---|
| Safe area / notches | `SafeArea` | Shell publishes insets → `SafeArea` layout ([[PLAN-CATALOG]] wave 6) |
| Keyboard type / secure entry | `TextInput.keyboardType`, `secureTextEntry` | Input/Textarea attrs → IME hints ([[PLAN-TEXTAREA]]) |
| Modal presentation | `Modal` | In-tree overlay (deterministic) **vs** OS dialog (effect) — both allowed; document when to use which |
| Long-press | `Pressable.longPress` | Pointer + [[19.1-touch|§19.1]] |
| Menus / tray / hotkeys | — | §19 table (`muda`, `tray-icon`, `global-hotkey`) — separate small plans when needed. Tray + window policy + launch-at-login: **[[PLAN-RESIDENCY]]**; global hotkeys still unclaimed |
| File pickers | — | Fold into `Dialog` / dedicated `Effect::PickFile` when an app needs it |

## Stages

| Stage | Deliverable |
|---|---|
| **N1** | `OpenUrl` end-to-end (effect → shell → harness) |
| **N2** | `Dialog` (alert + confirm) with `DialogResult` continuation |
| **N3** | `Notify` — split out to [[PLAN-NOTIFY]] (NT1–NT4) |
| **N4** | `Share` + document clipboard relationship |
| **N5** | Safe-area insets plumbing + `<SafeArea>` |
| **N6** | `WindowOp` subset used by real apps — [[PLAN-RESIDENCY]] R3 needs `Show`/`Hide`/`Raise`/`Quit`; build the variant **once**, in whichever lands first |
| **N7** | Menubar / context menu via `muda` when chrome catalog needs it — reuses [[PLAN-RESIDENCY]] R1's `muda` dependency and action-name validation |

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
