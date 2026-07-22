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
stub. Full target list for [[07.1-builtin-catalog|§7.1]],
aligned with Vercel json-render RN standards **plus** Libra's ops-dense
components. Umbrella: [[PLAN-UI-SURFACE]].

**Build rule:** one component (or tight pair) per change. Each lands with
token-only styling ([[PLAN-TOKENS]]), semantics, keyboard/focus where
interactive, gallery fixture, and snapshots under `libra-dark` (and
light when the matrix grows). No GPU. Prefer **beautiful defaults** over
prop sprawl — variants via tokens + a small closed attr set. *Where a
component lives* (one file per component) and the authoring template are
[[PLAN-COMPONENTS]].

## Mirror map (json-render RN → Libra)

Source: `@json-render/react-native` `standardComponentDefinitions`.

| json-render | Libra name | Status today | Notes |
|---|---|---|---|
| `Container` | `Box` / keep `Column`+`Row` | partial | Padding/bg/radius via tokens; may stay layout primitives |
| `Row` | `Row` | **shipped** | Extend align/justify/wrap as needed |
| `Column` | `Column` | **shipped** | |
| `ScrollContainer` | `Scroll` | **shipped** 2026-07-18 | Vertical viewport (`<Scroll testId height?>`); clips + wheel-scrolls via the [[09.1-display-list|§9.1]] clip primitive + scroll-as-input — [[PLAN-SCROLL]]. Horizontal later |
| `SafeArea` | `SafeArea` | planned | Shell insets — [[PLAN-NATIVE-INTEGRATIONS]] |
| `Spacer` | `Spacer` | **shipped** 2026-07-16 | `<Spacer/>` spring or `size={px}`; pure layout, paints nothing |
| `Pressable` | (Button / pressable slot) | partial | Prefer real controls; generic Pressable only if composition needs it |
| `Divider` | `Separator` | **shipped** 2026-07-16 | H default / `vertical`; 1px `theme.hairline` (first T1 token consumer) |
| `Heading` | `Heading` | **shipped** 2026-07-18 | `level` 1–4 → type ramp (xl/lg/md/sm), bold; builds to a `Text` item (reuses text paint) |
| `Paragraph` | `Text` / `Paragraph` | partial | Body defaults |
| `Label` | `Label` | **deferred** | `Text size="sm" muted` already covers captions; a *true* Label needs control-association (form model), so not worth a distinct node yet |
| `Image` | `Image` | planned (§7.1) | Deterministic decode path TBD |
| `Avatar` | `Avatar` | **shipped** 2026-07-19 | Initials in a circle; image variant waits on a deterministic decode path |
| `Badge` | `Badge` | **shipped** 2026-07-17 | `tone` (neutral/danger/warning/success/info) → status `*_bg`/`*_fg` tokens (PLAN-TOKENS T2); pill, `type_xs`, `space`-padded. First component built through the [[PLAN-COMPONENTS]] `catalog/` flow. |
| `Chip` | `Chip` | planned | Select / remove events |
| `Button` | `Button` | **shipped** | Expand variants/sizes via tokens |
| `TextInput` | `Input` (+ `Textarea`) | **shipped** (Textarea 2026-07-17; IME/wrap remain — [[PLAN-TEXTAREA]]) | |
| `Switch` | `Switch` | **shipped** 2026-07-19 | Model-owned `checked={bool}` + `onToggle` |
| `Checkbox` | `Checkbox` | **shipped** 2026-07-19 | Model-owned; check glyph is a display-list Polyline |
| `Slider` | `Slider` | **shipped** 2026-07-19/20 | Model-owned; the shell's Split-drag machinery generalized to a `DragTarget` enum |
| `SearchBar` | `Search` | planned | Input chrome + submit |
| `Spinner` | `Spinner` | **blocked** | Needs either an arc/circle draw command (none today) **or** an animation phase on the paint path — the latter is §9.6 animated tokens (unbuilt). A deterministic *static* spinner is just decoration; revisit when §9.6 or an arc `DrawCmd` lands |
| `ProgressBar` | `Progress` | **shipped** 2026-07-18 | `value={0..1}` — **literal or bound** (`value={ratio}`, typed f32; §6.5 preserved — [[26-open-questions|§26]] #8); `surface_raised` track + `accent` fill, pill radius |
| `Card` | `Card` + `Panel` | **shipped** 2026-07-19 | Surface containers sharing the Dialog panel chrome (Card framed/raised, Panel plain) |
| `ListItem` | `ListItem` | **shipped** 2026-07-17 | title/subtitle/meta + `selected` (bound bool) + hover wash (`surface_raised`); leading/trailing slots later |
| `Modal` | `Dialog` | **shipped** 2026-07-19 (in-tree: `theme.scrim` + centered panel, detached overlay layout, scrim hit barrier, optional `onDismiss`; mount via `<if>`) | OS dialog remains a §12.1 effect — see native plan |

## native-sdk.dev delta (audited 2026-07-19)

The reference SDK's live catalog ([native-sdk.dev/components](https://native-sdk.dev/components))
is broader than the json-render map above. Components it lists that this
plan didn't yet track, with dispositions:

| Theirs | Disposition |
|---|---|
| `Alert` (inline callout, variant styling) | **shipped** 2026-07-19 — `tone` → status token pair, title attr + body content |
| `Accordion` (disclosure sections) | **no new primitive needed** — fully expressible today as `Button` (header) + `<if test={open}>` (body) + `Column` composition, now that `Switch`/`Checkbox` prove the model-owned toggle shape works; not promoted to a catalog component unless a real app's repetition asks for it |
| `Breadcrumb`, `Pagination`, `Stepper`, `Timeline` | **shipped** 2026-07-20/21 — see the native-sdk.dev delta table above |
| `Skeleton` (loading placeholder) | **shipped** 2026-07-19 (static); shimmer still waits on §9.6 |
| `Drawer` / `Sheet` (side/bottom-anchored modals) | **shipped** 2026-07-19 as `<Drawer side=…>` (`bottom` = Sheet) — exactly the predicted fold onto the Dialog machinery |
| `Dropdown Menu` (anchored floating menu) | the machinery `ContextMenu` (M7) needs — anchored (not centered) overlay placement is the one new piece on top of the Dialog pass |
| `Input Group` (field + accessory actions) | defer — composition may cover it once Row-in-field styling exists |
| `Toggle` (pressed-state toggle buttons) | fold into Button variants / Switch wave |
| `Bubble` (chat surfaces) | out of scope — same call as AI elements |
| `Media Surface` (external textures: video/camera) | maps to §9.5 Video — late, unchanged |
| `Resizable` (engine-managed drag panel) | **not adopted**: they split model-owned `Split` from engine-managed `Resizable`; Libra keeps only the model-owned shape (fractions in State — replayable, snapshot-exact). Same reasoning as shell-owned scroll, opposite side: pane layout *is* app state |
| `Icon` (vector registry) | already tracked; still blocked on a path/arc `DrawCmd` |

**Design confirmations from their catalog:** their `Dialog` is
"modal surface with **model-owned dismissal**" — the shape `<Dialog>`
shipped with (`<if>` mount + `onDismiss` action). Their `Split` is
draggable with model-owned fractions — M5's planned shape exactly.

## Libra ops extras (not in json-render RN; keep)

From existing §7.1 mission-control set — still in scope, later waves:

| Component | Wave | Note |
|---|---|---|
| `Grid`, ~~`Split`~~ (**shipped** 2026-07-19 — model-owned fractions, drag + harness `resize_split`), `Panel` | layout+ | Dense consoles |
| `Status`, `Icon`, `Markdown`, ~~`Tooltip`~~ (**shipped** 2026-07-19/20 — hover-input floating label, hidden-at-rest second paint pass) | display | |
| `ButtonGroup`, ~~`Radio`~~ (**shipped** 2026-07-19 — groups via `<for>` + item-scoped `onSelect`), `Select`, `Combobox`, ~~`Stepper`~~ (**shipped** 2026-07-20/21) | controls | |
| `Table`, `VirtualList`, `Tree`, `Chart`, `Sparkline`, `Gauge`, `LogView` | data | Charts/sparklines **shipped** |
| `Tabs`, `StatusBar`, `Toolbar`, `CommandPalette`, `Toast`, ~~`ContextMenu`~~ (**shipped** 2026-07-19 — anchored overlay + ListItem `onMenu`; the anchored placement `Dropdown Menu`/`Select` will reuse), ~~`Breadcrumb`~~, ~~`Pagination`~~, ~~`Timeline`~~ (all **shipped** 2026-07-20/21) | chrome | |
| `Video` | late | §9.5 |
| `Frames`, `Interval`, `Stream` | sources | **shipped** (Stream delivery thin) |
| `Canvas` | escape | §7.4 |

## Beauty-first build order

Tokens ([[PLAN-TOKENS]]) should land **before or with** wave 1 so new paint
never invents colors.

| Wave | Components | Why this order |
|---|---|---|
| **0** | Token table expansion | Shared foundation — **done** ([[PLAN-TOKENS]] T1) |
| **1** | `Spacer`, `Separator`, `Scroll` | Layout completeness — **all done** (Scroll 2026-07-18) |
| **2** | ✅ `Badge`, `Heading`, `Progress` shipped; `Label` folded into `Text`; `Spinner` blocked (§9.6 / arc cmd) | Visible “product” chrome, low interaction risk |
| **3** | ✅ `Switch`, `Checkbox`, `Radio`, `Slider` shipped 2026-07-19/20; `Search` next | Control set parity |
| **4** | `Textarea` ([[PLAN-TEXTAREA]]), Input keyboard hints | Text completeness |
| **5** | `Card`, `ListItem`, `Avatar`, `Chip`, `Image` | Composite / media |
| **6** | `SafeArea`, ~~in-view `Modal`/`Dialog`~~ (**Dialog done** 2026-07-19) + OS dialog effect | Needs [[PLAN-NATIVE-INTEGRATIONS]] |
| **7** | `Tabs`, `Toast`, `Toolbar`, `StatusBar` | App chrome |
| **8** | `VirtualList`, `Table`, `LogView`, `Tree` | Data-dense (original stub priority) |
| **9** | `Select`/`Combobox`/`CommandPalette`/`Markdown`/`Icon` (`Radio`/`ContextMenu`/`Tooltip`/`Breadcrumb`/`Pagination`/`Timeline`/`Stepper` shipped) | Long-tail |
| **10** | `Grid`/`Split`/`Panel`/`Gauge`/`Video` | Advanced / late |

Reorder freely when a real app needs something earlier — but still
**one at a time**.

## Per-component acceptance (checklist)

For each catalog add:

- [ ] Normative mention in [[07.1-builtin-catalog|§7.1]] if new
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

← [[Plans Index]] · [[PLAN-UI-SURFACE]] · [[PLAN-COMPONENTS]] · [[07.1-builtin-catalog|§7.1]]
