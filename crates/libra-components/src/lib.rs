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
    pub grow: bool,
    /// align-items: center
    pub cross_center: bool,
    /// justify-content: center
    pub main_center: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ViewNode {
    Column { style: BoxStyle, children: Vec<ViewNode> },
    Row { style: BoxStyle, children: Vec<ViewNode> },
    Text { content: String, size: TextSize, bold: bool, muted: bool, grow: bool, test_id: Option<String> },
    Button { label: String, variant: Variant, on_press: String, test_id: Option<String> },
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
    /// `<Frames onTick={name}>` (§12.5): an invisible, zero-layout
    /// source element — declares "subscribe to vsync-coalesced frame
    /// ticks while this is mounted," diffed by the runtime like
    /// `App::subscriptions`. Renders nothing; excluded from the
    /// AccessKit projection (its `Role::Source` semantics are
    /// test-query-only). `Interval`/`Stream` are the same pattern,
    /// not yet implemented.
    SourceFrames { on_tick: String },
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
    Box, // Column/Row (no visual in Phase 0; containers are transparent)
    Text { content: String, px: f32, style: FontStyle, color: Color },
    Button { label: String, px: f32, variant: Variant },
    Chart { handle: SeriesHandle, panel: bool },
    Input { value: String, placeholder: String, px: f32 },
    /// §12.5 source element — zero-size, paints nothing (see
    /// `ViewNode::SourceFrames`).
    Source { on_tick: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub kind: ItemKind,
    pub rect: Rect,
    pub semantics: Semantics,
    /// Handler name to dispatch on press (Buttons only in Phase 0).
    pub on_press: Option<String>,
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
    let mut meta: Vec<(NodeId, ItemKind, Semantics, Option<String>)> = Vec::new();

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
    flatten(&tree, root, (0.0, 0.0), &meta, &mut items)?;

    Ok(LaidTree { items, width: viewport.0, height: viewport.1 })
}

fn flatten(
    tree: &TaffyTree<()>,
    node: NodeId,
    origin: (f32, f32),
    meta: &[(NodeId, ItemKind, Semantics, Option<String>)],
    out: &mut Vec<Item>,
) -> Result<(), LayoutError> {
    let l = tree.layout(node).map_err(|e| LayoutError(format!("{e:?}")))?;
    let abs = (origin.0 + l.location.x, origin.1 + l.location.y);
    let rect = Rect::new(abs.0, abs.1, l.size.width, l.size.height);

    if let Some((_, kind, sem, on_press)) = meta.iter().find(|(id, ..)| *id == node) {
        out.push(Item { kind: kind.clone(), rect, semantics: sem.clone(), on_press: on_press.clone() });
    }
    for child in tree.children(node).map_err(|e| LayoutError(format!("{e:?}")))? {
        flatten(tree, child, abs, meta, out)?;
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
        flex_grow: if s.grow { 1.0 } else { 0.0 },
        align_items: if s.cross_center { Some(AlignItems::Center) } else { None },
        justify_content: if s.main_center { Some(JustifyContent::Center) } else { None },
        ..Default::default()
    }
}

fn build(
    tree: &mut TaffyTree<()>,
    meta: &mut Vec<(NodeId, ItemKind, Semantics, Option<String>)>,
    view: &ViewNode,
    m: &dyn TextMeasurer,
    theme: &Theme,
) -> Result<NodeId, taffy::TaffyError> {
    match view {
        ViewNode::Column { style, children } => {
            let kids = children
                .iter()
                .map(|c| build(tree, meta, c, m, theme))
                .collect::<Result<Vec<_>, _>>()?;
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
            let kids = children
                .iter()
                .map(|c| build(tree, meta, c, m, theme))
                .collect::<Result<Vec<_>, _>>()?;
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
        ViewNode::Button { label, variant, on_press, test_id } => {
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
                Some(on_press.clone()),
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
                // in `on_press` storage (it is "the handler name").
                Some(on_change.clone()),
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
                ItemKind::Source { on_tick: on_tick.clone() },
                // `text: "frames"` is the source *kind*, matched by
                // `by::source("frames")` — not the handler name.
                Semantics { role: Role::Source, text: "frames".to_string(), test_id: None },
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
    pub fn paint(
        &self,
        measurer: &dyn TextMeasurer,
        store: &SeriesStore,
        focus: Option<&InputFocus>,
        theme: &Theme,
    ) -> Vec<DrawCmd> {
        let mut cmds = vec![DrawCmd::Clear(theme.bg)];
        for item in &self.items {
            match &item.kind {
                ItemKind::Box => {}
                ItemKind::Source { .. } => {}
                ItemKind::Chart { handle, panel } => {
                    paint_chart(&mut cmds, item.rect, handle, *panel, store, theme);
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

/// Chart painting (§14.2 read path): M4 the ring's window to pixel
/// columns, autoscale y to the window's extremes, emit either column
/// strips (dense data — cheap opaque runs, the M4 paper's own rendering)
/// or a stroked polyline (sparse data — smooth). Pure function of
/// (ring contents, rect) — a fed tape renders the same bytes every time.
fn paint_chart(cmds: &mut Vec<DrawCmd>, rect: Rect, handle: &SeriesHandle, panel: bool, store: &SeriesStore, theme: &Theme) {
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

    let width_px = inner.w as u32;
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
