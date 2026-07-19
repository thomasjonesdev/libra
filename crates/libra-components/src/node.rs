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
    Text { content: String, size: TextSize, bold: bool, muted: bool, grow: bool, test_id: Option<String> },
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
    Text { content: String, px: f32, style: FontStyle, color: Color },
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
    ListItem { title: String, subtitle: String, meta: String, selected: bool, px_md: f32, px_sm: f32 },
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
