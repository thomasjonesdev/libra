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

- [[19.1-touch-first-class-not-an-afterthought]]

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

Policy: window *chrome*, menus, and dialogs are native (they must feel like the OS); window *content* is Libra-rendered (it must be deterministic). DPI: layout in logical pixels; physical rounding once at paint ([[08-layout|§8]]); per-monitor scale changes are just damage. IME caveats per platform tracked in [[25-risks-and-mitigations|§25]].

**Staging:** effect wiring + SafeArea / share / open-URL — [[PLAN-NATIVE-INTEGRATIONS]] (under [[PLAN-UI-SURFACE]]). Catalog components that need OS sheets return effects; they never call platform APIs from `update`.

---

← [[Architecture Home]] · [[Map of Content]]
