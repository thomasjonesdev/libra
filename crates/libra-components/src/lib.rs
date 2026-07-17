//! libra-components — the component layer.
//!
//! ViewTree (typed output of a view engine) → taffy layout → a flattened,
//! laid-out tree carrying geometry + semantics, which can paint itself
//! into a DrawCmd list and hit-test interactions back to handler names.
//!
//! Catalog: Column, Row, Text, Button (Phase 0); Chart, Sparkline
//! (Phase 2 — data-plane components that read their series from the
//! `SeriesStore` at paint time, per §14.2: State holds handles,
//! samples never transit the loop). The reconciler (keyed diffing,
//! retained component state, focus) is still pending; the laid tree
//! rebuilds per dispatch, which damage tracking absorbs at raster time.

use libra_core::SeriesHandle;
use libra_data::{m4_downsample, SeriesStore};
use libra_draw::{Color, DrawCmd, FontStyle, Rect, Theme};
use taffy::prelude::*;

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
    Chart { handle: SeriesHandle, panel: bool },
    Input { value: String, placeholder: String, px: f32 },
    /// Multi-line sibling of Input (§10.1 stage 1) — same focus
    /// machinery, line-aware paint.
    Textarea { value: String, placeholder: String, px: f32 },
    /// Pressable list row; `selected`/hover paint a `surface_raised`
    /// wash behind the text lines.
    ListItem { title: String, subtitle: String, meta: String, selected: bool, px_md: f32, px_sm: f32 },
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
}

pub struct LaidTree {
    pub items: Vec<Item>,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug)]
pub struct LayoutError(pub String);

impl std::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "layout error: {}", self.0)
    }
}
impl std::error::Error for LayoutError {}

pub fn layout(
    view: &ViewNode,
    viewport: (f32, f32),
    measurer: &dyn TextMeasurer,
    theme: &Theme,
) -> Result<LaidTree, LayoutError> {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let mut meta: Vec<(NodeId, ItemKind, Semantics, Option<(String, Option<String>)>)> = Vec::new();

    let root = build(&mut tree, &mut meta, view, measurer, theme)
        .map_err(|e| LayoutError(format!("{e:?}")))?;

    // Root fills the viewport.
    let mut root_style = tree.style(root).map_err(|e| LayoutError(format!("{e:?}")))?.clone();
    root_style.size = Size { width: length(viewport.0), height: length(viewport.1) };
    tree.set_style(root, root_style).map_err(|e| LayoutError(format!("{e:?}")))?;

    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(viewport.0),
            height: AvailableSpace::Definite(viewport.1),
        },
    )
    .map_err(|e| LayoutError(format!("{e:?}")))?;

    // Flatten with accumulated absolute positions, in paint (pre-)order.
    let mut items = Vec::with_capacity(meta.len());
    let mut seen = std::collections::HashSet::new();
    flatten(&tree, root, (0.0, 0.0), &meta, &mut items, &mut seen)?;

    // §12.5 source leaves are intentionally not flex children — attach
    // them with a zero rect so harness/`by::source` still see them.
    for (id, kind, sem, _) in &meta {
        if matches!(kind, ItemKind::Source(_)) && !seen.contains(id) {
            items.push(Item {
                kind: kind.clone(),
                rect: Rect::new(0.0, 0.0, 0.0, 0.0),
                semantics: sem.clone(),
                on_press: None,
                on_press_arg: None,
            });
        }
    }

    Ok(LaidTree { items, width: viewport.0, height: viewport.1 })
}

fn flatten(
    tree: &TaffyTree<()>,
    node: NodeId,
    origin: (f32, f32),
    meta: &[(NodeId, ItemKind, Semantics, Option<(String, Option<String>)>)],
    out: &mut Vec<Item>,
    seen: &mut std::collections::HashSet<NodeId>,
) -> Result<(), LayoutError> {
    let l = tree.layout(node).map_err(|e| LayoutError(format!("{e:?}")))?;
    let abs = (origin.0 + l.location.x, origin.1 + l.location.y);
    let rect = Rect::new(abs.0, abs.1, l.size.width, l.size.height);

    if let Some((id, kind, sem, press)) = meta.iter().find(|(id, ..)| *id == node) {
        seen.insert(*id);
        out.push(Item {
            kind: kind.clone(),
            rect,
            semantics: sem.clone(),
            on_press: press.as_ref().map(|(name, _)| name.clone()),
            on_press_arg: press.as_ref().and_then(|(_, arg)| arg.clone()),
        });
    }
    for child in tree.children(node).map_err(|e| LayoutError(format!("{e:?}")))? {
        flatten(tree, child, abs, meta, out, seen)?;
    }
    Ok(())
}

fn box_style(s: &BoxStyle, dir: FlexDirection) -> Style {
    Style {
        display: Display::Flex,
        flex_direction: dir,
        gap: Size { width: length(s.gap), height: length(s.gap) },
        padding: taffy::Rect {
            left: length(s.padding),
            right: length(s.padding),
            top: length(s.padding),
            bottom: length(s.padding),
        },
        flex_grow: s.grow,
        // A fixed pane width must not be flexed away by siblings.
        flex_shrink: if s.width.is_some() { 0.0 } else { 1.0 },
        size: Size {
            width: s.width.map(length).unwrap_or(auto()),
            height: auto(),
        },
        align_items: if s.cross_center { Some(AlignItems::Center) } else { None },
        justify_content: if s.main_center { Some(JustifyContent::Center) } else { None },
        ..Default::default()
    }
}

/// Vertical gap between Textarea lines (added to the measured line
/// height). A fixed constant — deterministic, theme-independent.
pub const TEXTAREA_LEADING: f32 = 4.0;

/// §12.5: is this child a mounted source element? Used to keep all
/// three kinds (`Frames`/`Interval`/`Stream`) out of the flex tree —
/// they're registered in `meta` but never a taffy child.
fn is_source_node(n: &ViewNode) -> bool {
    matches!(
        n,
        ViewNode::SourceFrames { .. } | ViewNode::SourceInterval { .. } | ViewNode::SourceStream { .. }
    )
}

fn build(
    tree: &mut TaffyTree<()>,
    meta: &mut Vec<(NodeId, ItemKind, Semantics, Option<(String, Option<String>)>)>,
    view: &ViewNode,
    m: &dyn TextMeasurer,
    theme: &Theme,
) -> Result<NodeId, taffy::TaffyError> {
    match view {
        ViewNode::Column { style, children } => {
            let mut kids = Vec::new();
            for c in children {
                // §12.5: source elements occupy no layout — register them
                // in meta but keep them out of the flex tree so gap/grow
                // of siblings stay snapshot-stable.
                if is_source_node(c) {
                    let _ = build(tree, meta, c, m, theme)?;
                } else {
                    kids.push(build(tree, meta, c, m, theme)?);
                }
            }
            let id = tree.new_with_children(box_style(style, FlexDirection::Column), &kids)?;
            meta.push((
                id,
                ItemKind::Box,
                Semantics { role: Role::Container, text: String::new(), test_id: None },
                None,
            ));
            Ok(id)
        }
        ViewNode::Row { style, children } => {
            let mut kids = Vec::new();
            for c in children {
                if is_source_node(c) {
                    let _ = build(tree, meta, c, m, theme)?;
                } else {
                    kids.push(build(tree, meta, c, m, theme)?);
                }
            }
            let id = tree.new_with_children(box_style(style, FlexDirection::Row), &kids)?;
            meta.push((
                id,
                ItemKind::Box,
                Semantics { role: Role::Container, text: String::new(), test_id: None },
                None,
            ));
            Ok(id)
        }
        ViewNode::Text { content, size, bold, muted, grow, test_id } => {
            let px = size.px(theme);
            let style = if *bold { FontStyle::Bold } else { FontStyle::Regular };
            let (w, h) = m.measure(content, px, style);
            let id = tree.new_leaf(Style {
                size: Size { width: length(w), height: length(h) },
                flex_grow: if *grow { 1.0 } else { 0.0 },
                flex_shrink: 0.0,
                ..Default::default()
            })?;
            let color = if *muted { theme.text_muted } else { theme.text };
            meta.push((
                id,
                ItemKind::Text { content: content.clone(), px, style, color },
                Semantics { role: Role::Text, text: content.clone(), test_id: test_id.clone() },
                None,
            ));
            Ok(id)
        }
        ViewNode::Button { label, variant, on_press, on_press_arg, test_id } => {
            let px = theme.type_md;
            let (tw, th) = m.measure(label, px, FontStyle::Regular);
            let (w, h) = (tw + 2.0 * theme.btn_pad_x, th + 2.0 * theme.btn_pad_y);
            let id = tree.new_leaf(Style {
                size: Size { width: length(w), height: length(h) },
                flex_shrink: 0.0,
                ..Default::default()
            })?;
            meta.push((
                id,
                ItemKind::Button { label: label.clone(), px, variant: *variant },
                Semantics { role: Role::Button, text: label.clone(), test_id: test_id.clone() },
                Some((on_press.clone(), on_press_arg.clone())),
            ));
            Ok(id)
        }
        ViewNode::Chart { handle, height, test_id } => {
            let id = tree.new_leaf(Style {
                size: Size { width: auto(), height: length(*height) },
                flex_grow: 1.0,
                flex_shrink: 0.0,
                ..Default::default()
            })?;
            meta.push((
                id,
                ItemKind::Chart { handle: *handle, panel: true },
                Semantics { role: Role::Chart, text: handle.to_string(), test_id: test_id.clone() },
                None,
            ));
            Ok(id)
        }
        ViewNode::Sparkline { handle, width, height, test_id } => {
            let id = tree.new_leaf(Style {
                size: Size { width: length(*width), height: length(*height) },
                flex_shrink: 0.0,
                ..Default::default()
            })?;
            meta.push((
                id,
                ItemKind::Chart { handle: *handle, panel: false },
                Semantics { role: Role::Chart, text: handle.to_string(), test_id: test_id.clone() },
                None,
            ));
            Ok(id)
        }
        ViewNode::Input { value, placeholder, on_change, test_id } => {
            let px = theme.type_md;
            let (_, th) = m.measure("Ag", px, FontStyle::Regular);
            let h = th + 2.0 * theme.btn_pad_y;
            let id = tree.new_leaf(Style {
                size: Size { width: auto(), height: length(h) },
                flex_grow: 1.0,
                flex_shrink: 1.0,
                ..Default::default()
            })?;
            meta.push((
                id,
                ItemKind::Input { value: value.clone(), placeholder: placeholder.clone(), px },
                Semantics {
                    role: Role::TextInput,
                    text: value.clone(),
                    test_id: Some(test_id.clone()),
                },
                // Clicking focuses rather than dispatching; the shell/
                // harness special-case TextInput hits. on_change rides
                // in `on_press` storage (it is "the handler name"); no
                // markup-bound payload (the control supplies the text).
                Some((on_change.clone(), None)),
            ));
            Ok(id)
        }
        ViewNode::ListItem {
            title,
            subtitle,
            meta: meta_line,
            selected,
            on_press,
            on_press_arg,
            test_id,
        } => {
            let px_md = theme.type_md;
            let px_sm = theme.type_sm;
            let (_, th_md) = m.measure("Ag", px_md, FontStyle::Regular);
            let (_, th_sm) = m.measure("Ag", px_sm, FontStyle::Regular);
            let lines = 1 + usize::from(!subtitle.is_empty()) + usize::from(!meta_line.is_empty());
            let h = 2.0 * theme.btn_pad_y
                + th_md
                + if lines > 1 { (lines - 1) as f32 * (th_sm + 2.0) } else { 0.0 };
            let id = tree.new_leaf(Style {
                size: Size { width: auto(), height: length(h) },
                flex_shrink: 0.0,
                ..Default::default()
            })?;
            meta.push((
                id,
                ItemKind::ListItem {
                    title: title.clone(),
                    subtitle: subtitle.clone(),
                    meta: meta_line.clone(),
                    selected: *selected,
                    px_md,
                    px_sm,
                },
                Semantics { role: Role::Button, text: title.clone(), test_id: test_id.clone() },
                Some((on_press.clone(), on_press_arg.clone())),
            ));
            Ok(id)
        }
        ViewNode::Textarea { value, placeholder, on_change, test_id } => {
            let px = theme.type_md;
            let (_, th) = m.measure("Ag", px, FontStyle::Regular);
            // Minimum three lines; grows to fill the column.
            let min_h = 3.0 * (th + TEXTAREA_LEADING) + 2.0 * theme.btn_pad_y;
            let id = tree.new_leaf(Style {
                size: Size { width: auto(), height: auto() },
                min_size: Size { width: auto(), height: length(min_h) },
                flex_grow: 1.0,
                flex_shrink: 1.0,
                ..Default::default()
            })?;
            meta.push((
                id,
                ItemKind::Textarea { value: value.clone(), placeholder: placeholder.clone(), px },
                Semantics {
                    role: Role::TextInput,
                    text: value.clone(),
                    test_id: Some(test_id.clone()),
                },
                Some((on_change.clone(), None)),
            ));
            Ok(id)
        }
        ViewNode::Spacer { size } => {
            // Pure layout: fixed room along the parent's main axis
            // (flex-basis is main-axis-relative, so one node works in
            // both Column and Row), or a flex spring when sizeless.
            let style = match size {
                Some(s) => Style { flex_basis: length(*s), flex_shrink: 0.0, ..Default::default() },
                None => Style { flex_grow: 1.0, ..Default::default() },
            };
            let id = tree.new_leaf(style)?;
            meta.push((
                id,
                ItemKind::Box,
                Semantics { role: Role::Container, text: String::new(), test_id: None },
                None,
            ));
            Ok(id)
        }
        ViewNode::Separator { vertical } => {
            // 1px along the parent's main axis, stretched across the
            // cross axis (taffy's default align-items: stretch).
            let style = if *vertical {
                Style {
                    size: Size { width: length(1.0), height: auto() },
                    flex_shrink: 0.0,
                    ..Default::default()
                }
            } else {
                Style {
                    size: Size { width: auto(), height: length(1.0) },
                    flex_shrink: 0.0,
                    ..Default::default()
                }
            };
            let id = tree.new_leaf(style)?;
            meta.push((
                id,
                ItemKind::Separator,
                Semantics { role: Role::Separator, text: String::new(), test_id: None },
                None,
            ));
            Ok(id)
        }
        ViewNode::SourceFrames { on_tick } => {
            let id = tree.new_leaf(Style {
                size: Size { width: length(0.0), height: length(0.0) },
                ..Default::default()
            })?;
            meta.push((
                id,
                ItemKind::Source(SourceDecl::Frames { on_tick: on_tick.clone() }),
                // `text: "frames"` is the source *kind*, matched by
                // `by::source("frames")` — not the handler name.
                Semantics { role: Role::Source, text: "frames".to_string(), test_id: None },
                None,
            ));
            Ok(id)
        }
        ViewNode::SourceInterval { every_ms, on_tick } => {
            let id = tree.new_leaf(Style {
                size: Size { width: length(0.0), height: length(0.0) },
                ..Default::default()
            })?;
            meta.push((
                id,
                ItemKind::Source(SourceDecl::Interval { every_ms: *every_ms, on_tick: on_tick.clone() }),
                Semantics { role: Role::Source, text: "interval".to_string(), test_id: None },
                None,
            ));
            Ok(id)
        }
        ViewNode::SourceStream { id: stream_id, on_event } => {
            let id = tree.new_leaf(Style {
                size: Size { width: length(0.0), height: length(0.0) },
                ..Default::default()
            })?;
            meta.push((
                id,
                ItemKind::Source(SourceDecl::Stream { id: stream_id.clone(), on_event: on_event.clone() }),
                Semantics { role: Role::Source, text: "stream".to_string(), test_id: None },
                None,
            ));
            Ok(id)
        }
    }
}

// ---------------------------------------------------------------------------
// Paint & hit testing
// ---------------------------------------------------------------------------

/// Focus state for painting the caret/selection of the focused Input.
/// Owned by the shell/harness (component state is retained outside the
/// tree until the reconciler); byte offsets index into the input's value.
#[derive(Debug, Clone, PartialEq)]
pub struct InputFocus {
    pub test_id: String,
    pub caret: usize,
    pub selection: std::ops::Range<usize>,
}

impl LaidTree {
    /// Emit the display list. `store` is the data plane: Chart items
    /// read their ring here, at paint time (§14.2) — this is the one
    /// place samples meet the frame. `focus` paints the focused
    /// Input's ring, selection band, and caret.
    ///
    /// Delegates to `paint_at_density` at density 1.0 — the canonical
    /// frame (§9) snapshot tests assert against is density 1.0 by
    /// definition; every caller except the windowed shell (§21, off the
    /// determinism contract) wants exactly this.
    pub fn paint(
        &self,
        measurer: &dyn TextMeasurer,
        store: &SeriesStore,
        focus: Option<&InputFocus>,
        theme: &Theme,
    ) -> Vec<DrawCmd> {
        self.paint_full(measurer, store, focus, None, theme, 1.0)
    }

    /// `paint` with an explicit hover point (logical px). Hover is a
    /// *deterministic input* like focus, not ambient pointer state:
    /// the shell feeds its cursor, the harness feeds `None` by default
    /// (snapshots unchanged) or an explicit point for hover snapshots.
    /// Only `ListItem` reacts today (a `surface_raised` wash).
    pub fn paint_at_density(
        &self,
        measurer: &dyn TextMeasurer,
        store: &SeriesStore,
        focus: Option<&InputFocus>,
        theme: &Theme,
        density: f32,
    ) -> Vec<DrawCmd> {
        self.paint_full(measurer, store, focus, None, theme, density)
    }

    /// The full paint entry point: `density` is the PLAN-DASHBOARD-PERF
    /// Stage 6 presentation hint (charts compute M4 columns at
    /// `density`× resolution; the windowed shell passes its scale
    /// factor — never the canonical density-1.0 snapshot path), and
    /// `hover` is the optional pointer point (see `paint_at_density`'s
    /// doc for the determinism argument).
    pub fn paint_full(
        &self,
        measurer: &dyn TextMeasurer,
        store: &SeriesStore,
        focus: Option<&InputFocus>,
        hover: Option<(f32, f32)>,
        theme: &Theme,
        density: f32,
    ) -> Vec<DrawCmd> {
        let mut cmds = vec![DrawCmd::Clear(theme.bg)];
        for item in &self.items {
            match &item.kind {
                ItemKind::Box => {}
                ItemKind::Source(_) => {}
                ItemKind::Separator => {
                    cmds.push(DrawCmd::Rect { rect: item.rect, color: theme.hairline });
                }
                ItemKind::ListItem { title, subtitle, meta, selected, px_md, px_sm } => {
                    let hovered = hover.is_some_and(|(hx, hy)| item.rect.contains(hx, hy));
                    if *selected || hovered {
                        cmds.push(DrawCmd::RRect {
                            rect: item.rect,
                            radius: theme.radius_md,
                            color: theme.surface_raised,
                        });
                    }
                    let (tx, mut ty) = (item.rect.x + theme.btn_pad_x, item.rect.y + theme.btn_pad_y);
                    let (_, th_md) = measurer.measure("Ag", *px_md, FontStyle::Regular);
                    let (_, th_sm) = measurer.measure("Ag", *px_sm, FontStyle::Regular);
                    cmds.push(DrawCmd::Text {
                        x: tx,
                        y: ty,
                        content: title.clone(),
                        px: *px_md,
                        style: FontStyle::Regular,
                        color: theme.text,
                    });
                    ty += th_md + 2.0;
                    for line in [subtitle, meta] {
                        if line.is_empty() {
                            continue;
                        }
                        cmds.push(DrawCmd::Text {
                            x: tx,
                            y: ty,
                            content: line.clone(),
                            px: *px_sm,
                            style: FontStyle::Regular,
                            color: theme.text_muted,
                        });
                        ty += th_sm + 2.0;
                    }
                }
                ItemKind::Textarea { value, placeholder, px } => {
                    let focused = focus
                        .filter(|f| item.semantics.test_id.as_deref() == Some(f.test_id.as_str()));
                    paint_textarea(&mut cmds, item.rect, value, placeholder, *px, focused, measurer, theme);
                }
                ItemKind::Chart { handle, panel } => {
                    paint_chart(&mut cmds, item.rect, handle, *panel, store, theme, density);
                }
                ItemKind::Input { value, placeholder, px } => {
                    let focused = focus
                        .filter(|f| item.semantics.test_id.as_deref() == Some(f.test_id.as_str()));
                    paint_input(&mut cmds, item.rect, value, placeholder, *px, focused, measurer, theme);
                }
                ItemKind::Text { content, px, style, color } => {
                    // Vertically center within the (measured-height) rect.
                    cmds.push(DrawCmd::Text {
                        x: item.rect.x,
                        y: item.rect.y,
                        content: content.clone(),
                        px: *px,
                        style: *style,
                        color: *color,
                    });
                }
                ItemKind::Button { label, px, variant } => {
                    let (fill, text_color) = match variant {
                        Variant::Primary => (theme.accent, theme.accent_text),
                        Variant::Secondary => (theme.secondary, theme.text),
                        Variant::Ghost => (Color::TRANSPARENT, theme.text_muted),
                    };
                    if fill != Color::TRANSPARENT {
                        cmds.push(DrawCmd::RRect { rect: item.rect, radius: theme.radius_md, color: fill });
                    }
                    let (tw, th) = measurer.measure(label, *px, FontStyle::Regular);
                    cmds.push(DrawCmd::Text {
                        x: item.rect.x + (item.rect.w - tw) / 2.0,
                        y: item.rect.y + (item.rect.h - th) / 2.0,
                        content: label.clone(),
                        px: *px,
                        style: FontStyle::Regular,
                        color: text_color,
                    });
                }
            }
        }
        cmds
    }

    /// Topmost interactive item containing the point.
    pub fn hit(&self, x: f32, y: f32) -> Option<&Item> {
        self.items
            .iter()
            .rev()
            .find(|i| i.on_press.is_some() && i.rect.contains(x, y))
    }
}

/// Input painting (§10.1): field chrome, value or muted placeholder,
/// and — when focused — a focus ring, the selection band, and a caret.
/// Caret/selection x-positions come from prefix advance widths, so the
/// same measurer that laid the text positions the caret.
#[allow(clippy::too_many_arguments)]
fn paint_input(
    cmds: &mut Vec<DrawCmd>,
    rect: Rect,
    value: &str,
    placeholder: &str,
    px: f32,
    focus: Option<&InputFocus>,
    m: &dyn TextMeasurer,
    theme: &Theme,
) {
    cmds.push(DrawCmd::RRect { rect, radius: theme.radius_md, color: theme.secondary });
    if focus.is_some() {
        // Focus ring: a 1px accent frame (four thin rects — no stroke
        // rrect in the Phase 3 display list).
        let c = theme.accent;
        cmds.push(DrawCmd::Rect { rect: Rect::new(rect.x, rect.y, rect.w, 1.0), color: c });
        cmds.push(DrawCmd::Rect { rect: Rect::new(rect.x, rect.y + rect.h - 1.0, rect.w, 1.0), color: c });
        cmds.push(DrawCmd::Rect { rect: Rect::new(rect.x, rect.y, 1.0, rect.h), color: c });
        cmds.push(DrawCmd::Rect { rect: Rect::new(rect.x + rect.w - 1.0, rect.y, 1.0, rect.h), color: c });
    }
    let (tx, ty) = (rect.x + theme.btn_pad_x, rect.y + theme.btn_pad_y);
    let (_, th) = m.measure("Ag", px, FontStyle::Regular);

    if let Some(f) = focus {
        // Selection band behind the text.
        let sel = &f.selection;
        if !sel.is_empty() && sel.end <= value.len() {
            let x0 = tx + m.measure(&value[..sel.start], px, FontStyle::Regular).0;
            let x1 = tx + m.measure(&value[..sel.end], px, FontStyle::Regular).0;
            cmds.push(DrawCmd::Rect {
                rect: Rect::new(x0, ty, (x1 - x0).max(1.0), th),
                color: theme.selection,
            });
        }
    }
    if value.is_empty() {
        if !placeholder.is_empty() {
            cmds.push(DrawCmd::Text {
                x: tx,
                y: ty,
                content: placeholder.to_string(),
                px,
                style: FontStyle::Regular,
                color: theme.text_muted,
            });
        }
    } else {
        cmds.push(DrawCmd::Text {
            x: tx,
            y: ty,
            content: value.to_string(),
            px,
            style: FontStyle::Regular,
            color: theme.text,
        });
    }
    if let Some(f) = focus {
        let caret = f.caret.min(value.len());
        let cx = tx + m.measure(&value[..caret], px, FontStyle::Regular).0;
        cmds.push(DrawCmd::Rect {
            rect: Rect::new(cx, ty, 1.0, th),
            color: theme.text,
        });
    }
}

/// Textarea painting (§10.1 stage 1): field chrome, hard-newline lines
/// (no soft wrap yet), muted placeholder when empty, and — when
/// focused — the focus ring, per-line selection bands, and a
/// line-positioned caret. Line height is measured height +
/// [`TEXTAREA_LEADING`]; x positions come from prefix advances within
/// the caret's line, same measurer that laid the text.
#[allow(clippy::too_many_arguments)]
fn paint_textarea(
    cmds: &mut Vec<DrawCmd>,
    rect: Rect,
    value: &str,
    placeholder: &str,
    px: f32,
    focus: Option<&InputFocus>,
    m: &dyn TextMeasurer,
    theme: &Theme,
) {
    cmds.push(DrawCmd::RRect { rect, radius: theme.radius_md, color: theme.secondary });
    if focus.is_some() {
        let c = theme.accent;
        cmds.push(DrawCmd::Rect { rect: Rect::new(rect.x, rect.y, rect.w, 1.0), color: c });
        cmds.push(DrawCmd::Rect { rect: Rect::new(rect.x, rect.y + rect.h - 1.0, rect.w, 1.0), color: c });
        cmds.push(DrawCmd::Rect { rect: Rect::new(rect.x, rect.y, 1.0, rect.h), color: c });
        cmds.push(DrawCmd::Rect { rect: Rect::new(rect.x + rect.w - 1.0, rect.y, 1.0, rect.h), color: c });
    }
    let (tx, ty) = (rect.x + theme.btn_pad_x, rect.y + theme.btn_pad_y);
    let (_, th) = m.measure("Ag", px, FontStyle::Regular);
    let line_h = th + TEXTAREA_LEADING;

    if value.is_empty() {
        if !placeholder.is_empty() {
            cmds.push(DrawCmd::Text {
                x: tx,
                y: ty,
                content: placeholder.to_string(),
                px,
                style: FontStyle::Regular,
                color: theme.text_muted,
            });
        }
    }

    // Walk lines with their byte ranges so selection/caret map exactly.
    let mut line_start = 0usize;
    let mut row = 0usize;
    loop {
        let rest = &value[line_start..];
        let line_len = rest.find('\n').unwrap_or(rest.len());
        let line = &value[line_start..line_start + line_len];
        let ly = ty + row as f32 * line_h;

        if let Some(f) = focus {
            // Selection band: this line's overlap with the selection.
            let sel = &f.selection;
            let (ls, le) = (line_start, line_start + line_len);
            let s = sel.start.max(ls);
            let e = sel.end.min(le);
            if s < e {
                let x0 = tx + m.measure(&value[ls..s], px, FontStyle::Regular).0;
                let x1 = tx + m.measure(&value[ls..e], px, FontStyle::Regular).0;
                cmds.push(DrawCmd::Rect {
                    rect: Rect::new(x0, ly, (x1 - x0).max(1.0), th),
                    color: theme.selection,
                });
            }
        }
        if !line.is_empty() {
            cmds.push(DrawCmd::Text {
                x: tx,
                y: ly,
                content: line.to_string(),
                px,
                style: FontStyle::Regular,
                color: theme.text,
            });
        }
        if let Some(f) = focus {
            let caret = f.caret.min(value.len());
            if caret >= line_start && caret <= line_start + line_len {
                let cx = tx + m.measure(&value[line_start..caret], px, FontStyle::Regular).0;
                cmds.push(DrawCmd::Rect { rect: Rect::new(cx, ly, 1.0, th), color: theme.text });
            }
        }

        match rest.find('\n') {
            Some(nl) => {
                line_start += nl + 1;
                row += 1;
            }
            None => break,
        }
    }
}

/// Chart painting (§14.2 read path): M4 the ring's window to pixel
/// columns, autoscale y to the window's extremes, emit either column
/// strips (dense data — cheap opaque runs, the M4 paper's own rendering)
/// or a stroked polyline (sparse data — smooth). Pure function of
/// (ring contents, rect, density) — a fed tape renders the same bytes
/// every time at a given density.
///
/// `density` (Stage 6): column count scales with it so a Retina caller
/// gets one column per physical pixel; `col_w` is computed from
/// `inner.w` (logical) and the *unscaled* strip count, so after the
/// caller scales the resulting cmds by `density`, each column lands
/// exactly 1 physical pixel wide rather than 1/density.
#[allow(clippy::too_many_arguments)]
fn paint_chart(
    cmds: &mut Vec<DrawCmd>,
    rect: Rect,
    handle: &SeriesHandle,
    panel: bool,
    store: &SeriesStore,
    theme: &Theme,
    density: f32,
) {
    const PAD: f32 = 4.0;
    if panel {
        cmds.push(DrawCmd::RRect { rect, radius: theme.radius_md, color: theme.surface });
    }
    let inner = Rect::new(
        rect.x + PAD,
        rect.y + PAD,
        (rect.w - 2.0 * PAD).max(1.0),
        (rect.h - 2.0 * PAD).max(1.0),
    );

    let width_px = ((inner.w * density.max(0.01)).round() as u32).max(1);
    let cols = store.chart_columns(handle.id, handle.window_ms, width_px);
    if cols.samples < 2 {
        return; // no data: chrome only
    }

    let (mut vmin, mut vmax) = (f64::INFINITY, f64::NEG_INFINITY);
    for s in cols.strips.iter().flatten() {
        vmin = vmin.min(s.0);
        vmax = vmax.max(s.1);
    }
    let span = if vmax > vmin { vmax - vmin } else { 1.0 };
    let to_y = |v: f64| inner.y + (1.0 - ((v - vmin) / span) as f32) * inner.h;

    if cols.samples >= 2 * width_px as usize {
        // Dense: column min/max envelope as opaque strips (the M4
        // paper's own rendering — cheap by construction).
        let strips: Vec<Option<(f32, f32)>> = cols
            .strips
            .iter()
            .map(|o| o.map(|(lo, hi)| (to_y(hi), to_y(lo)))) // y inverts
            .collect();
        let col_w = inner.w / cols.strips.len().max(1) as f32;
        cmds.push(DrawCmd::Strips { x: inner.x, col_w, strips, color: theme.accent });
    } else {
        // Sparse: exact points, stroked smooth. (M4 passthrough keeps
        // them all; the snapshot copy is small by definition here.)
        let ring = store.ring(handle.id);
        let (samples, _) = ring.snapshot_from(cols.t_from);
        let pts = m4_downsample(&samples, cols.t_from, cols.t_to, width_px);
        if pts.len() < 2 {
            return;
        }
        let points: Vec<(f32, f32)> = pts
            .iter()
            .map(|p| {
                let fx = ((p.t_ms - cols.t_from) / (cols.t_to - cols.t_from)) as f32;
                (inner.x + fx * inner.w, to_y(p.v))
            })
            .collect();
        cmds.push(DrawCmd::Polyline { points, width: 1.25, color: theme.accent });
    }
}
