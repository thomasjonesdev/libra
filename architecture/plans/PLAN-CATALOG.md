---
title: "PLAN-CATALOG"
aliases:
  - "PLAN-CATALOG"
tags:
  - plans
  - architecture
type: plan
---

# Component catalog — expanded inventory

**Status: planned.** Supersedes the short “VirtualList → Table → LogView”
stub. Full target list for [[07.1-the-built-in-catalog-closed-curated|§7.1]],
aligned with Vercel json-render RN standards **plus** Libra's ops-dense
components. Umbrella: [[PLAN-UI-SURFACE]].

**Build rule:** one component (or tight pair) per change. Each lands with
token-only styling ([[PLAN-TOKENS]]), semantics, keyboard/focus where
interactive, gallery fixture, and snapshots under `libra-dark` (and
light when the matrix grows). No GPU. Prefer **beautiful defaults** over
prop sprawl — variants via tokens + a small closed attr set.

## Mirror map (json-render RN → Libra)

Source: `@json-render/react-native` `standardComponentDefinitions`.

| json-render | Libra name | Status today | Notes |
|---|---|---|---|
| `Container` | `Box` / keep `Column`+`Row` | partial | Padding/bg/radius via tokens; may stay layout primitives |
| `Row` | `Row` | **shipped** | Extend align/justify/wrap as needed |
| `Column` | `Column` | **shipped** | |
| `ScrollContainer` | `Scroll` | planned | Vertical/horizontal; §7.1 |
| `SafeArea` | `SafeArea` | planned | Shell insets — [[PLAN-NATIVE-INTEGRATIONS]] |
| `Spacer` | `Spacer` | planned (§7.1) | Fixed size or `grow` |
| `Pressable` | (Button / pressable slot) | partial | Prefer real controls; generic Pressable only if composition needs it |
| `Divider` | `Separator` | planned (§7.1) | H/V |
| `Heading` | `Heading` or `Text size=` | planned | Levels h1–h4 → type tokens |
| `Paragraph` | `Text` / `Paragraph` | partial | Body defaults |
| `Label` | `Label` | planned | Caption / form label |
| `Image` | `Image` | planned (§7.1) | Deterministic decode path TBD |
| `Avatar` | `Avatar` | planned | Image or initials |
| `Badge` | `Badge` | planned (§7.1) | Semantic variants via tokens |
| `Chip` | `Chip` | planned | Select / remove events |
| `Button` | `Button` | **shipped** | Expand variants/sizes via tokens |
| `TextInput` | `Input` (+ `Textarea`) | partial | Multiline → [[PLAN-TEXTAREA]] |
| `Switch` | `Switch` | planned | |
| `Checkbox` | `Checkbox` | planned | |
| `Slider` | `Slider` | planned | |
| `SearchBar` | `Search` | planned | Input chrome + submit |
| `Spinner` | `Spinner` | planned | Deterministic frame, not wall-clock spin in snapshots |
| `ProgressBar` | `Progress` | planned | 0…1 value |
| `Card` | `Card` | planned | Prefer composition; elevate via tokens |
| `ListItem` | `ListItem` | planned | Title/subtitle/leading/trailing |
| `Modal` | `Dialog` / `Modal` | planned | In-tree chrome **and/or** OS dialog — see native plan |

## Libra ops extras (not in json-render RN; keep)

From existing §7.1 mission-control set — still in scope, later waves:

| Component | Wave | Note |
|---|---|---|
| `Grid`, `Split`, `Panel` | layout+ | Dense consoles |
| `Status`, `Icon`, `Markdown`, `Tooltip` | display | |
| `ButtonGroup`, `Radio`, `Select`, `Combobox`, `Stepper` | controls | |
| `Table`, `VirtualList`, `Tree`, `Chart`, `Sparkline`, `Gauge`, `LogView` | data | Charts/sparklines **shipped** |
| `Tabs`, `StatusBar`, `Toolbar`, `CommandPalette`, `Toast`, `ContextMenu` | chrome | |
| `Video` | late | §9.5 |
| `Frames`, `Interval`, `Stream` | sources | **shipped** (Stream delivery thin) |
| `Canvas` | escape | §7.4 |

## Beauty-first build order

Tokens ([[PLAN-TOKENS]]) should land **before or with** wave 1 so new paint
never invents colors.

| Wave | Components | Why this order |
|---|---|---|
| **0** | Token table expansion | Shared foundation |
| **1** | `Spacer`, `Separator`, `Scroll` | Layout completeness; unblocks long UIs |
| **2** | `Heading`/`Label` polish, `Badge`, `Progress`, `Spinner` | Visible “product” chrome, low interaction risk |
| **3** | `Switch`, `Checkbox`, `Slider`, `Search` | Control set parity with RN catalog |
| **4** | `Textarea` ([[PLAN-TEXTAREA]]), Input keyboard hints | Text completeness |
| **5** | `Card`, `ListItem`, `Avatar`, `Chip`, `Image` | Composite / media |
| **6** | `SafeArea`, in-view `Modal`/`Dialog` + OS dialog effect | Needs [[PLAN-NATIVE-INTEGRATIONS]] |
| **7** | `Tabs`, `Toast`, `Toolbar`, `StatusBar` | App chrome |
| **8** | `VirtualList`, `Table`, `LogView`, `Tree` | Data-dense (original stub priority) |
| **9** | `Select`/`Combobox`/`Radio`/`CommandPalette`/`ContextMenu`/`Tooltip`/`Markdown`/`Icon` | Long-tail |
| **10** | `Grid`/`Split`/`Panel`/`Gauge`/`Video` | Advanced / late |

Reorder freely when a real app needs something earlier — but still
**one at a time**.

## Per-component acceptance (checklist)

For each catalog add:

- [ ] Normative mention in [[07.1-the-built-in-catalog-closed-curated|§7.1]] if new
- [ ] `ViewNode` + layout + paint; **only theme tokens** (no raw hex in component)
- [ ] Semantics (role/label/state) for AccessKit + harness queries
- [ ] Both engines if markup attrs grow + conformance fixture when grammar changes (§6.4)
- [ ] Gallery cell × theme; snapshots committed; bless deliberately
- [ ] Doc comment cites §7.1 / this plan
- [ ] Defaults look finished at rest (padding, type, contrast) — beauty bar is the point

## Explicitly out of catalog v1

- Arbitrary CSS-like style bags on every node
- RN-only gestures as required API (Libra: pointer + §19.1 touch plan)
- Generative-UI / AI-elements chat kit (separate product decision)

---

← [[Plans Index]] · [[PLAN-UI-SURFACE]] · [[07.1-the-built-in-catalog-closed-curated|§7.1]]
