//! Shared component vocabulary: the ViewTree a view engine produces
//! (`ViewNode`), the flattened `LaidTree` of `Item`s layout lays it out to,
//! and the `Semantics` / `Role` tree a11y and test queries share. Plus the
//! `InputFocus` paint state and the `TextMeasurer` contract.
//!
//! Architecture: ARCHITECTURE.md §7 (components), §12.5 (source elements).

use libra_core::SeriesHandle;
use libra_draw::{Color, FontStyle, Rect, Theme};

// ---------------------------------------------------------------------------
// ViewTree — what view engines produce (ARCHITECTURE.md §4.1)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextSize {
    Sm,
    Md,
    Xl,
}

impl TextSize {
    pub fn px(self, theme: &Theme) -> f32 {
        match self {
            TextSize::Sm => theme.type_sm,
            TextSize::Md => theme.type_md,
            TextSize::Xl => theme.type_xl,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    Primary,
    Secondary,
    Ghost,
}

/// Semantic tone for status components (`Badge`; later `Status`/`Toast`).
/// Resolves to a `theme.<tone>_bg`/`_fg` token pair — never a hex literal
/// (PLAN-TOKENS T2). Both engines map the markup `tone="…"` string to this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tone {
    #[default]
    Neutral,
    Danger,
    Warning,
    Success,
    Info,
}

/// Which edge a `<Drawer>` pins to (`bottom` covers the reference
/// SDK's Sheet).
/// A `<StepperItem>`'s derived status (§7.1) — computed by the
/// enclosing `<Stepper>` from index vs. `current`, never an attr.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepStatus {
    Completed,
    Active,
    Pending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DrawerSide {
    #[default]
    Left,
    Right,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BoxStyle {
    pub gap: f32,
    pub padding: f32,
    /// Flex-grow weight. `0.0` = none; the bare `grow` flag = `1.0`;
    /// `grow={2.5}` = weighted share — panes stay proportional as the
    /// window resizes (responsive by construction, no breakpoints).
    pub grow: f32,
    /// align-items: center
    pub cross_center: bool,
    /// justify-content: center
    pub main_center: bool,
    /// Fixed main-axis-independent width in logical px (`width={240}`)
    /// — pane sizing for sidebar/list/editor layouts (§7.1 Layout).
    /// `None` = auto (content / flex).
    pub width: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ViewNode {
    Column { style: BoxStyle, children: Vec<ViewNode> },
    Row { style: BoxStyle, children: Vec<ViewNode> },
    Text { content: String, size: TextSize, bold: bool, muted: bool, grow: bool, wrap: bool, test_id: Option<String> },
    /// `on_press` is the handler action name; `on_press_arg`, when
    /// `Some`, is the resolved `String` payload for an item-scoped
    /// handler (`onPress={toggle(item)}`, §6.2) — the dispatch uses
    /// `action_with_str` instead of `action_by_name`.
    Button {
        label: String,
        variant: Variant,
        on_press: String,
        on_press_arg: Option<String>,
        test_id: Option<String>,
    },
    /// `<Badge tone="success">Online</Badge>` (§7.1, PLAN-CATALOG wave 2): a
    /// small status pill. Non-interactive — child text is the label; `tone`
    /// selects a semantic `theme.<tone>_bg`/`_fg` pair (PLAN-TOKENS T2).
    Badge { label: String, tone: Tone, test_id: Option<String> },
    /// `<Heading level={2}>Title</Heading>` (§7.1, PLAN-CATALOG wave 2): a
    /// semantic heading. `level` 1–4 maps to the type ramp (xl/lg/md/sm),
    /// always bold; child text is the content. Renders as a bold `Text` item.
    Heading { content: String, level: u8, test_id: Option<String> },
    /// `<Progress value={0.6} />` (§7.1, PLAN-CATALOG wave 2): a determinate
    /// progress bar, `value` 0..=1. Token track + accent fill.
    Progress { value: f32, test_id: Option<String> },
    /// `<Scroll testId="…" height={px?}>…</Scroll>` (§7.1, PLAN-SCROLL): a
    /// vertical scroll viewport. Content lays out at natural height; the
    /// viewport clips it and offsets it by the scroll position (a deterministic
    /// input, keyed by `test_id`). `height` fixes the viewport; omitted, it
    /// grows to fill.
    Scroll { height: Option<f32>, test_id: String, children: Vec<ViewNode> },
    /// Time-series line chart (§7.1 Data): reads its ring at paint time,
    /// M4-downsampled to pixel width. Grows to fill width; fixed height.
    Chart { handle: SeriesHandle, height: f32, test_id: Option<String> },
    /// Inline mini-chart: fixed size, no panel chrome.
    Sparkline { handle: SeriesHandle, width: f32, height: f32, test_id: Option<String> },
    /// Single-line text entry (§10.1). `value` is the model's field
    /// (controlled input: the model is the source of truth); typing
    /// dispatches `on_change` with the control-provided new text.
    /// `test_id` is required — it is the focus identity until the
    /// reconciler brings structural IDs.
    Input { value: String, placeholder: String, on_change: String, test_id: String },
    /// `<ListItem>` (§7.1, PLAN-CATALOG): a pressable list row —
    /// title line, optional muted `subtitle`/`meta` lines, `selected`
    /// wash (`theme.surface_raised`; also the hover wash). The whole
    /// row is the press target; `on_press_arg` carries the item-scoped
    /// payload exactly like Button (§6.2).
    ListItem {
        title: String,
        subtitle: String,
        meta: String,
        selected: bool,
        on_press: String,
        on_press_arg: Option<String>,
        /// `onMenu={action}` / `onMenu={action(loopVar.field)}` (M7):
        /// dispatched by right-click; typically opens a `<ContextMenu>`.
        on_menu: Option<String>,
        on_menu_arg: Option<String>,
        test_id: Option<String>,
    },
    /// `<Textarea>` (§10.1, PLAN-TEXTAREA stage 1): multi-line
    /// controlled text entry on the same pure editing core as Input.
    /// Hard newlines only (no soft wrap yet); grows to fill its column.
    Textarea { value: String, placeholder: String, on_change: String, test_id: String },
    /// `<Spacer/>` (§7.1 Layout, PLAN-CATALOG wave 1): pure layout —
    /// `size: Some(px)` takes fixed room on the parent's main axis,
    /// `None` is a flex spring (`grow`) pushing siblings apart. Paints
    /// nothing.
    Spacer { size: Option<f32> },
    /// `<Separator/>` (§7.1 Layout, PLAN-CATALOG wave 1): a 1px
    /// hairline rule across the parent's cross axis — horizontal in a
    /// `Column`, `vertical` for pane seams in a `Row`. Paints
    /// `theme.hairline` (PLAN-TOKENS T1).
    Separator { vertical: bool },
    /// `<Split fraction={0..1} onResize={action} testId=…>` (§7.1,
    /// PLAN-CATALOG / PLAN-NOTES-APP M5): a two-pane draggable
    /// splitter with **model-owned fractions** — `fraction` binds a
    /// State f32 (typed, like `<Progress value>`), the shell's divider
    /// drag dispatches `onResize` with the new fraction's Display as
    /// its String payload, and the app echoes it into the bound field
    /// (the controlled shape; pane layout *is* app state — replayable,
    /// snapshot-exact). Exactly two children.
    Split { fraction: f32, on_resize: String, test_id: String, children: Vec<ViewNode> },
    /// `<Alert tone="danger" title=…>body</Alert>` (§7.1, PLAN-CATALOG):
    /// an inline callout — status-token tinted panel, bold title line,
    /// muted body from the element's text content.
    Alert { tone: Tone, title: String, body: String, test_id: Option<String> },
    /// `<Card>…</Card>` (§7.1): the bordered, elevated surface
    /// container — `surface_raised` + hairline frame. Ordinary flex
    /// column inside (gap/padding attrs like Column).
    Card { style: BoxStyle, children: Vec<ViewNode> },
    /// `<Panel>…</Panel>` (§7.1): the plain surface container.
    Panel { style: BoxStyle, children: Vec<ViewNode> },
    /// `<Skeleton width? height? />` (§7.1): a static loading
    /// placeholder block (deterministic — shimmer waits on §9.6).
    /// No width = stretch.
    Skeleton { width: Option<f32>, height: f32 },
    /// `<Avatar name={…} size? />` (§7.1): initials in a circle
    /// (image variant waits on a deterministic decode path).
    Avatar { name: String, size: f32, test_id: Option<String> },
    /// `<Switch checked={bool} onToggle={action} />` (§7.1): on/off
    /// with **model-owned state** — the bound bool is the truth, the
    /// press dispatches, the app flips.
    Switch { checked: bool, on_toggle: String, test_id: Option<String> },
    /// `<Checkbox checked={bool} onToggle={action}>Label</Checkbox>`
    /// (§7.1): binary choice, model-owned like Switch; the check glyph
    /// draws with the display list's Polyline.
    Checkbox { checked: bool, on_toggle: String, label: String, test_id: Option<String> },
    /// `<Radio selected={bool} onSelect={action}>Label</Radio>` (§7.1):
    /// single choice; groups come from a `<for>` with the item-scoped
    /// `onSelect={pick(opt.id)}` form.
    Radio {
        selected: bool,
        on_select: String,
        on_select_arg: Option<String>,
        label: String,
        test_id: Option<String>,
    },
    /// `<Breadcrumb>` (§7.1, PLAN-CATALOG): a hierarchy trail — its
    /// `BreadcrumbItem` children render with an auto-inserted chevron
    /// between them; the container is a Row-shaped wrapper.
    Breadcrumb { children: Vec<ViewNode> },
    /// `<BreadcrumbItem label=… current? onPress?=…>` (§7.1): one
    /// crumb — pressable (a link) unless `current` (the trailing,
    /// non-pressable, bold crumb). `onPress` supports the item-scoped
    /// `action(loopVar.field)` form for `<for>`-generated trails.
    BreadcrumbItem {
        label: String,
        current: bool,
        on_press: Option<String>,
        on_press_arg: Option<String>,
        test_id: Option<String>,
    },
    /// `<Pagination current={u32} total={u32} onPage={action} testId=…>`
    /// (§7.1): a fully generated page-nav row (Prev/Next + a windowed
    /// run of page buttons around `current`, ellipsis for the rest).
    /// `onPage` is a String-payload action; each page button dispatches
    /// through the existing item-scoped mechanism (its own page number
    /// as the payload) — no new dispatch machinery.
    Pagination { current: u32, total: u32, on_page: String, test_id: Option<String> },
    /// `<Stepper current={u32}>` (§7.1): progress stages — each
    /// `StepperItem` child's completed/active/pending status is
    /// derived from its position relative to `current` (0-based); a
    /// connector line is auto-inserted between steps.
    Stepper { current: u32, children: Vec<ViewNode> },
    /// `<StepperItem label=… testId?=…>` (§7.1): one stage; status
    /// comes from the parent `<Stepper>`'s index math, not an attr.
    StepperItem { label: String, test_id: Option<String> },
    /// `<Timeline>` (§7.1): a ledger list with per-row indicators and a
    /// connecting line between `TimelineItem` children.
    Timeline { children: Vec<ViewNode> },
    /// `<TimelineItem title=… meta? done?>` (§7.1): one entry; `done`
    /// fills its indicator dot (vs. a hollow ring).
    TimelineItem { title: String, meta: String, done: bool, test_id: Option<String> },
    /// `<Slider value={0..1} onChange={action} testId=…>` (§7.1,
    /// PLAN-CATALOG): a continuous control, **model-owned** like
    /// Split/Switch — `value` binds a typed State f32, track
    /// press/drag dispatches `onChange` with the new fraction's
    /// Display, the app echoes it back.
    Slider { value: f32, on_change: String, test_id: String },
    /// `<Tooltip text=…>child</Tooltip>` (§7.1): a floating label over
    /// its child while the **hover input** is inside it — hover is
    /// already deterministic (shell cursor / harness `hover(sel)`), so
    /// tooltip frames are snapshot-exact. Painted in a second pass, on
    /// top of everything.
    Tooltip { text: String, children: Vec<ViewNode> },
    /// `<Drawer side="left|right|bottom" onDismiss?=… testId?=…>` (§7.1):
    /// an edge-anchored modal on the Dialog overlay machinery — scrim +
    /// panel pinned to a side (`bottom` is the reference's Sheet).
    Drawer {
        side: DrawerSide,
        on_dismiss: Option<String>,
        test_id: Option<String>,
        children: Vec<ViewNode>,
    },
    /// `<ContextMenu onDismiss?=… testId=…>` (§7.1, PLAN-CATALOG /
    /// PLAN-NOTES-APP M7): an anchored floating menu — the Dialog
    /// overlay machinery with an invisible full-viewport barrier and a
    /// small panel positioned at the **menu anchor**, a deterministic
    /// input like focus/hover/scroll (the shell records the right-click
    /// point; the harness's `right_click` pins the item's center;
    /// default (0,0)). Mount/unmount is the app's `<if>`; children are
    /// ordinary markup (ghost Buttons as items). Clicking the barrier
    /// dispatches `onDismiss` (or swallows without it).
    ContextMenu { on_dismiss: Option<String>, test_id: String, children: Vec<ViewNode> },
    /// `<Dialog title=… onDismiss?=… testId?=…>` (§7.1, PLAN-CATALOG /
    /// PLAN-NOTES-APP M6): an **in-tree modal** — full-viewport scrim +
    /// centered panel holding ordinary markup children. Deterministic
    /// and snapshot-able (unlike OS dialogs, which are §12.1 effects).
    /// Mount/unmount is the app's `<if test={…}>`; the component has no
    /// open/close state of its own. Detached from the parent flex tree
    /// and laid out as its own viewport-sized overlay root, so it
    /// paints on top and its scrim hit-tests as a modal barrier
    /// wherever it appears in the markup.
    Dialog {
        title: String,
        on_dismiss: Option<String>,
        test_id: Option<String>,
        children: Vec<ViewNode>,
    },
    /// `<Frames onTick={name}>` (§12.5): an invisible, zero-layout
    /// source element — declares "subscribe to vsync-coalesced frame
    /// ticks while this is mounted," diffed by the runtime like
    /// `App::subscriptions`. Renders nothing; excluded from the
    /// AccessKit projection (its `Role::Source` semantics are
    /// test-query-only).
    SourceFrames { on_tick: String },
    /// `<Interval every={ms} onTick={name}>` (§12.5): a recurring
    /// timer while mounted — identity is `(every_ms, on_tick)`, the
    /// moral equivalent of `SourceFrames`'s single identity but keyed
    /// per-cadence so several distinct intervals can coexist. Same
    /// zero-layout, test-query-only treatment.
    SourceInterval { every_ms: u64, on_tick: String },
    /// `<Stream id={feed} onEvent={name}>` (§12.5): grammar + mount +
    /// query in v1 — `id` is the resolved binding's Display text,
    /// `on_event` is a String-payload handler name that will carry the
    /// event body as text once delivery (Ws/Spawn, §12.1) lands. Same
    /// zero-layout, test-query-only treatment as the other sources.
    SourceStream { id: String, on_event: String },
}

// ---------------------------------------------------------------------------
// Semantics — the tree that a11y and test queries share (§17)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Container,
    Text,
    Button,
    Chart,
    TextInput,
    /// A visual divider (`<Separator>`); maps to a non-interactive
    /// generic node in the AccessKit projection.
    Separator,
    /// A `<ContextMenu>` panel — maps to AccessKit's menu role.
    Menu,
    /// Model-owned controls (§7.1) — AccessKit Switch/CheckBox/RadioButton.
    Switch,
    Checkbox,
    Radio,
    /// A `<Dialog>` panel — maps to AccessKit's dialog role; its title
    /// is the semantics text.
    Dialog,
    /// A mounted source element (§12.5) — test-query-only; the a11y
    /// layer excludes this role from the AccessKit projection.
    Source,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Semantics {
    pub role: Role,
    pub text: String,
    pub test_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Pointer affordance — the derived `cursor:` analog (§19)
// ---------------------------------------------------------------------------

/// What the pointer should look like over a point in the laid tree
/// (§19) — Libra's answer to CSS `cursor:`.
///
/// It is **derived, never authored**: the shape follows from what an
/// item *is* (pressable, text-editable, draggable), so it cannot drift
/// from the item's behavior the way a hand-written `cursor:` declaration
/// can, and it stays out of the closed grammar (§6.2 — markup gains no
/// decorative attribute for it). `LaidTree::cursor_at` is the one
/// mapping; the windowed shell translates this closed set to the OS
/// icon vocabulary, and the harness asserts it as ordinary semantics.
///
/// The OS draws the pointer, so this never enters the display list —
/// it is off the determinism contract (Appendix A) by construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Cursor {
    /// Nothing interactive under the pointer.
    #[default]
    Default,
    /// Anything pressable: Button, ListItem, Switch/Checkbox/Radio,
    /// Slider, a crumb, a page button, a dismissible scrim.
    Pointer,
    /// I-beam — an `Input`/`Textarea` (`Role::TextInput`), where a press
    /// places a caret instead of dispatching (§10.1).
    Text,
    /// Horizontal resize — a `<Split>` divider gutter, whose press
    /// starts a drag rather than a click.
    ColResize,
    /// A present-but-inert control (a disabled Pagination arrow).
    NotAllowed,
}

// ---------------------------------------------------------------------------
// Text measurement contract — implemented by libra-raster
// ---------------------------------------------------------------------------

pub trait TextMeasurer {
    /// Returns (width, height) of a single-line run at `px`.
    fn measure(&self, text: &str, px: f32, style: FontStyle) -> (f32, f32);
}

// ---------------------------------------------------------------------------
// Laid tree — geometry + semantics + interactivity, flattened in paint order
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum ItemKind {
    Box, // Column/Row/Spacer (no visual; containers are transparent)
    /// `<Separator>` — paints a `theme.hairline` rect over its rect.
    Separator,
    Text { content: String, px: f32, style: FontStyle, color: Color, wrap: bool },
    Button { label: String, px: f32, variant: Variant },
    /// Status pill — `tone` picks a `theme.<tone>_bg`/`_fg` pair.
    Badge { label: String, tone: Tone, px: f32 },
    /// Determinate progress bar — `value` 0..=1 fills the track. (Heading has
    /// no ItemKind: it builds to a bold `Text`.)
    Progress { value: f32 },
    /// A scroll viewport (PLAN-SCROLL) — paints nothing; it clips + offsets its
    /// descendants (which carry `Item::clip` / `Item::scroll`).
    Scroll,
    Chart { handle: SeriesHandle, panel: bool },
    Input { value: String, placeholder: String, px: f32 },
    /// Multi-line sibling of Input (§10.1 stage 1) — same focus
    /// machinery, line-aware paint.
    Textarea { value: String, placeholder: String, px: f32 },
    /// Pressable list row; `selected`/hover paint a `surface_raised`
    /// wash behind the text lines.
    ListItem {
        title: String,
        subtitle: String,
        meta: String,
        selected: bool,
        px_md: f32,
        px_sm: f32,
        /// M7 right-click handler `(name, optional item-scoped arg)`.
        on_menu: Option<(String, Option<String>)>,
    },
    /// `<Alert>` — tinted callout painting its own title/body.
    Alert { tone: Tone, title: String, body: String, px_md: f32, px_sm: f32 },
    /// `<Card>`/`<Panel>` — a surface container behind its children
    /// (`raised` adds the hairline frame + elevated fill).
    Surface { raised: bool },
    /// `<Skeleton>` — static placeholder block.
    Skeleton,
    /// `<Avatar>` — initials in a circle.
    Avatar { initials: String, px: f32 },
    /// `<Switch>` — model-owned on/off control.
    SwitchCtl { checked: bool },
    /// `<Checkbox>` — model-owned binary choice + label.
    CheckboxCtl { checked: bool, label: String, px: f32 },
    /// `<Radio>` — model-owned single choice + label.
    RadioCtl { selected: bool, label: String, px: f32 },
    /// `<Slider>` — model-owned continuous control; carries its
    /// handler for the shell's track drag / harness `set_slider`.
    SliderCtl { value: f32, on_change: String },
    /// `<Tooltip>` wrapper — invisible container; the second paint
    /// pass draws `text` above it while hovered.
    TooltipHost { text: String },
    /// `<Breadcrumb>` — invisible Row-shaped wrapper.
    BreadcrumbBox,
    /// One crumb — pressable unless current (handler lives in the
    /// press slot, like Button).
    BreadcrumbLabel { label: String, current: bool },
    /// Auto-inserted "›" between crumbs.
    Chevron,
    /// `<Pagination>` — invisible; its page/prev/next buttons are
    /// ordinary `ItemKind::Button`-shaped leaves it builds itself.
    PaginationBox,
    /// One Pagination button: a page number, or the ‹/› nav glyphs.
    /// `active` bolds the current page; `enabled` dims + disables Prev
    /// at the start / Next at the end.
    PageButton { label: String, active: bool, enabled: bool },
    /// The "…" gap in a windowed page run — not pressable.
    PageEllipsis,
    /// `<Stepper>` — invisible; paints the connector between steps.
    StepperBox,
    /// One stage — status derived by the container from index vs
    /// `current`.
    StepperDot { label: String, status: StepStatus, px: f32 },
    /// Line segment between two Stepper steps (horizontal) or two
    /// Timeline entries (vertical) — same primitive, different axis.
    Connector { vertical: bool, filled: bool },
    /// `<Timeline>` — invisible; paints the connector between entries.
    TimelineBox,
    /// One entry — dot + title/meta, connector drawn by the container.
    TimelineDot { title: String, meta: String, done: bool, px_md: f32, px_sm: f32 },
    /// A `<Split>` container — invisible; carries the split's identity
    /// (semantics test_id) and its `onResize` handler for the shell's
    /// divider drag and the harness's `resize_split`. The handler lives
    /// here, NOT in the item's press slot — the container is never a
    /// click target.
    Split { on_resize: String },
    /// A `<Split>`'s divider gutter — paints a centered hairline;
    /// `split` names the owning `<Split>`'s test_id so a pointer press
    /// on the gutter knows which split it resizes.
    SplitDivider { split: String },
    /// A Dialog's full-viewport backdrop: paints `theme.scrim` and acts
    /// as the modal barrier in `hit()` — with a handler it dispatches
    /// `onDismiss`, without one it swallows the click.
    Scrim,
    /// A `<ContextMenu>`'s full-viewport barrier — paints nothing (no
    /// dim for menus) but blocks/dismisses like `Scrim`.
    MenuBarrier,
    /// A `<ContextMenu>`'s floating panel — `surface_raised` card at
    /// the menu anchor; its semantics carry the menu's test_id.
    MenuPanel,
    /// A Dialog's centered panel: `surface_raised` card + hairline
    /// frame. The title renders as an ordinary bold Text child; the
    /// panel's semantics carry it for queries/a11y.
    DialogPanel,
    /// §12.5 source element — zero-size, paints nothing. Carries which
    /// kind mounted (see `SourceDecl`) so the shell/harness can
    /// dispatch/query by kind without re-deriving it from `ViewNode`.
    Source(SourceDecl),
}

/// What a mounted §12.5 source element declares — one variant per
/// element kind (`Frames`/`Interval`/`Stream`), mirroring the
/// `ViewNode::Source*` variants it was built from.
#[derive(Debug, Clone, PartialEq)]
pub enum SourceDecl {
    Frames { on_tick: String },
    Interval { every_ms: u64, on_tick: String },
    Stream { id: String, on_event: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub kind: ItemKind,
    pub rect: Rect,
    pub semantics: Semantics,
    /// Handler name to dispatch on press (Buttons only in Phase 0).
    pub on_press: Option<String>,
    /// Resolved `String` payload for an item-scoped handler
    /// (`onPress={toggle(item)}`, §6.2). `Some` ⇒ dispatch via
    /// `action_with_str(on_press, arg)`; `None` ⇒ `action_by_name`.
    pub on_press_arg: Option<String>,
    /// PLAN-SCROLL: the clip rect (an enclosing `<Scroll>` viewport) this item
    /// paints within, if any. `paint` wraps it in `PushClip`/`PopClip`.
    pub clip: Option<Rect>,
    /// PLAN-SCROLL: the `test_id` of the enclosing `<Scroll>`, if any —
    /// `apply_scroll` offsets this item's rect by that viewport's position.
    pub scroll: Option<String>,
    /// M7: the `test_id` of the enclosing `<ContextMenu>` panel, if any —
    /// `apply_menu_anchor` translates tagged items to the anchor point.
    pub menu: Option<String>,
}

/// Scroll positions by `<Scroll>` test_id (PLAN-SCROLL). A deterministic input
/// like focus/hover: the shell fills it from the wheel, the harness from
/// `scroll()`, and it's empty (offset 0) under snapshots. `BTreeMap` for
/// deterministic iteration.
pub type ScrollState = std::collections::BTreeMap<String, f32>;

pub struct LaidTree {
    pub items: Vec<Item>,
    pub width: f32,
    pub height: f32,
}

impl LaidTree {
    /// Offset each scrolled item by its viewport's position (PLAN-SCROLL),
    /// clamped to `[0, content_overflow]`. Call once on a freshly-laid tree
    /// (rects are natural); the shell/harness re-lay-out before re-applying.
    /// M7: translate every `<ContextMenu>` panel (and its children) to
    /// the menu anchor — a deterministic input like scroll offsets. The
    /// panel is clamped so it stays fully inside the viewport.
    pub fn apply_menu_anchor(&mut self, anchor: (f32, f32)) {
        let ids: Vec<String> = self
            .items
            .iter()
            .filter(|i| matches!(i.kind, ItemKind::MenuPanel))
            .filter_map(|i| i.semantics.test_id.clone())
            .collect();
        for id in ids {
            let Some(panel) = self
                .items
                .iter()
                .find(|i| matches!(i.kind, ItemKind::MenuPanel) && i.semantics.test_id.as_deref() == Some(&id))
                .map(|i| i.rect)
            else {
                continue;
            };
            let dx = anchor.0.min(self.width - panel.w).max(0.0) - panel.x;
            let dy = anchor.1.min(self.height - panel.h).max(0.0) - panel.y;
            if dx == 0.0 && dy == 0.0 {
                continue;
            }
            for item in self.items.iter_mut().filter(|i| i.menu.as_deref() == Some(id.as_str())) {
                item.rect.x += dx;
                item.rect.y += dy;
            }
        }
    }

    pub fn apply_scroll(&mut self, scroll: &ScrollState) {
        for (id, &raw) in scroll {
            let Some(vp) = self
                .items
                .iter()
                .find(|i| matches!(i.kind, ItemKind::Scroll) && i.semantics.test_id.as_deref() == Some(id))
                .map(|i| i.rect)
            else {
                continue;
            };
            let content_bottom = self
                .items
                .iter()
                .filter(|i| i.scroll.as_deref() == Some(id))
                .fold(vp.y, |acc, i| acc.max(i.rect.y + i.rect.h));
            let overflow = (content_bottom - (vp.y + vp.h)).max(0.0);
            let offset = raw.clamp(0.0, overflow);
            if offset <= 0.0 {
                continue;
            }
            for item in self.items.iter_mut().filter(|i| i.scroll.as_deref() == Some(id)) {
                item.rect.y -= offset;
            }
        }
    }
}

#[derive(Debug)]
pub struct LayoutError(pub String);

impl std::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "layout error: {}", self.0)
    }
}
impl std::error::Error for LayoutError {}

/// Vertical gap between Textarea lines (added to the measured line
/// height). A fixed constant — deterministic, theme-independent.
pub const TEXTAREA_LEADING: f32 = 4.0;

/// Focus state for painting the caret/selection of the focused Input.
/// Owned by the shell/harness (component state is retained outside the
/// tree until the reconciler); byte offsets index into the input's value.
#[derive(Debug, Clone, PartialEq)]
pub struct InputFocus {
    pub test_id: String,
    pub caret: usize,
    pub selection: std::ops::Range<usize>,
}
