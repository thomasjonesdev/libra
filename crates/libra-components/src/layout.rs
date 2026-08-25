//! The layout pass (ARCHITECTURE.md §8): build a taffy tree from a `ViewNode`,
//! compute it at the viewport, and flatten it to paint-ordered `Item`s with
//! absolute rects.
//!
//! `build` is a thin dispatcher: each arm delegates to the component's own
//! `catalog::<component>::build`, which receives a [`BuildCx`] (the taffy tree,
//! the meta accumulator, the measurer, the theme). One file per component —
//! see PLAN-COMPONENTS.

use crate::catalog;
use crate::node::*;
use libra_draw::{Rect, Theme};
use taffy::prelude::*;

/// The per-node accumulator: `(taffy id, item kind, semantics, handler)`.
/// `handler` is `(on_press_name, optional item-scoped String arg)`.
pub(crate) type Meta = Vec<(NodeId, ItemKind, Semantics, Option<(String, Option<String>)>)>;

/// Everything a component's `build` needs. Passed by `&mut` so a component
/// can add taffy nodes, push semantics/handlers, and lay out its children.
pub(crate) struct BuildCx<'a> {
    pub tree: &'a mut TaffyTree<()>,
    pub meta: &'a mut Meta,
    pub m: &'a dyn TextMeasurer,
    pub theme: &'a Theme,
    /// Overlay roots (`<Dialog>` scrims): detached from their markup
    /// parents, laid out at the full viewport after the main pass, and
    /// flattened last so they paint on top.
    pub overlays: &'a mut Vec<NodeId>,
}

impl BuildCx<'_> {
    /// Lay out a child node (containers use this to recurse).
    pub(crate) fn child(&mut self, node: &ViewNode) -> Result<NodeId, taffy::TaffyError> {
        build(self, node)
    }
}

/// Dispatch a `ViewNode` to its component's `build`. Adding a component is a
/// new arm here + a `catalog/<component>.rs` module (+ the grammar in both
/// engines, §6.4).
pub(crate) fn build(cx: &mut BuildCx, view: &ViewNode) -> Result<NodeId, taffy::TaffyError> {
    match view {
        ViewNode::Column { style, children } => catalog::container::build_column(cx, style, children),
        ViewNode::Row { style, children } => catalog::container::build_row(cx, style, children),
        ViewNode::Text { content, size, bold, muted, grow, wrap, test_id } => {
            catalog::text::build(cx, content, *size, *bold, *muted, *grow, *wrap, test_id)
        }
        ViewNode::Button { label, variant, on_press, on_press_arg, test_id } => {
            catalog::button::build(cx, label, *variant, on_press, on_press_arg, test_id)
        }
        ViewNode::Badge { label, tone, test_id } => catalog::badge::build(cx, label, *tone, test_id),
        ViewNode::Chip {
            label,
            selected,
            on_press,
            on_press_arg,
            on_remove,
            on_remove_arg,
            test_id,
        } => catalog::chip::build(
            cx,
            label,
            *selected,
            on_press,
            on_press_arg,
            on_remove,
            on_remove_arg,
            test_id,
        ),
        ViewNode::Heading { content, level, test_id } => {
            catalog::heading::build(cx, content, *level, test_id)
        }
        ViewNode::Progress { value, test_id } => catalog::progress::build(cx, *value, test_id),
        ViewNode::Scroll { height, test_id, children } => {
            catalog::scroll::build(cx, *height, test_id, children)
        }
        ViewNode::Chart { handle, height, test_id } => {
            catalog::chart::build_chart(cx, *handle, *height, test_id)
        }
        ViewNode::Sparkline { handle, width, height, test_id } => {
            catalog::chart::build_sparkline(cx, *handle, *width, *height, test_id)
        }
        ViewNode::Input { value, placeholder, on_change, test_id } => {
            catalog::input::build(cx, value, placeholder, on_change, test_id)
        }
        ViewNode::Search { value, placeholder, on_change, on_submit, test_id } => {
            catalog::search::build(cx, value, placeholder, on_change, on_submit, test_id)
        }
        ViewNode::ListItem {
            title,
            subtitle,
            meta,
            selected,
            on_press,
            on_press_arg,
            on_menu,
            on_menu_arg,
            test_id,
        } => catalog::list_item::build(
            cx, title, subtitle, meta, *selected, on_press, on_press_arg, on_menu, on_menu_arg,
            test_id,
        ),
        ViewNode::Textarea { value, placeholder, on_change, test_id } => {
            catalog::textarea::build(cx, value, placeholder, on_change, test_id)
        }
        ViewNode::Dialog { title, on_dismiss, test_id, children } => {
            catalog::dialog::build(cx, title, on_dismiss, test_id, children)
        }
        ViewNode::ContextMenu { on_dismiss, test_id, children } => {
            catalog::context_menu::build(cx, on_dismiss, test_id, children)
        }
        ViewNode::Alert { tone, title, body, test_id } => {
            catalog::surface::build_alert(cx, *tone, title, body, test_id)
        }
        ViewNode::Card { style, children } => catalog::surface::build_surface(cx, style, children, true),
        ViewNode::Panel { style, children } => {
            catalog::surface::build_surface(cx, style, children, false)
        }
        ViewNode::Skeleton { width, height } => catalog::surface::build_skeleton(cx, *width, *height),
        ViewNode::Avatar { name, size, test_id } => {
            catalog::surface::build_avatar(cx, name, *size, test_id)
        }
        ViewNode::Switch { checked, on_toggle, test_id } => {
            catalog::toggle::build_switch(cx, *checked, on_toggle, test_id)
        }
        ViewNode::Checkbox { checked, on_toggle, label, test_id } => {
            catalog::toggle::build_checkbox(cx, *checked, on_toggle, label, test_id)
        }
        ViewNode::Radio { selected, on_select, on_select_arg, label, test_id } => {
            catalog::toggle::build_radio(cx, *selected, on_select, on_select_arg, label, test_id)
        }
        ViewNode::Drawer { side, on_dismiss, test_id, children } => {
            catalog::drawer::build(cx, *side, on_dismiss, test_id, children)
        }
        ViewNode::Slider { value, on_change, test_id } => {
            catalog::slider::build(cx, *value, on_change, test_id)
        }
        ViewNode::Tooltip { text, children } => catalog::tooltip::build(cx, text, children),
        ViewNode::Breadcrumb { children } => catalog::breadcrumb::build_breadcrumb(cx, children),
        ViewNode::BreadcrumbItem { label, current, on_press, on_press_arg, test_id } => {
            catalog::breadcrumb::build_item(cx, label, *current, on_press, on_press_arg, test_id)
        }
        ViewNode::Pagination { current, total, on_page, test_id } => {
            catalog::pagination::build(cx, *current, *total, on_page, test_id)
        }
        ViewNode::Stepper { current, children } => catalog::stepper::build(cx, *current, children),
        // Only reachable if a `<StepperItem>` appears outside `<Stepper>`
        // (a documented misuse) — Stepper's own build() constructs its
        // children directly, computing status from sibling position.
        ViewNode::StepperItem { label, test_id } => {
            catalog::stepper::build(cx, 0, std::slice::from_ref(&ViewNode::StepperItem {
                label: label.clone(),
                test_id: test_id.clone(),
            }))
        }
        ViewNode::Timeline { children } => catalog::timeline::build(cx, children),
        ViewNode::TimelineItem { title, meta, done, test_id } => catalog::timeline::build(
            cx,
            std::slice::from_ref(&ViewNode::TimelineItem {
                title: title.clone(),
                meta: meta.clone(),
                done: *done,
                test_id: test_id.clone(),
            }),
        ),
        ViewNode::Split { fraction, on_resize, test_id, children } => {
            catalog::split::build(cx, *fraction, on_resize, test_id, children)
        }
        ViewNode::Spacer { size } => catalog::spacer::build(cx, *size),
        ViewNode::Separator { vertical } => catalog::separator::build(cx, *vertical),
        ViewNode::SourceFrames { on_tick } => catalog::source::build_frames(cx, on_tick),
        ViewNode::SourceInterval { every_ms, on_tick } => {
            catalog::source::build_interval(cx, *every_ms, on_tick)
        }
        ViewNode::SourceStream { id, on_event } => catalog::source::build_stream(cx, id, on_event),
    }
}

pub fn layout(
    view: &ViewNode,
    viewport: (f32, f32),
    measurer: &dyn TextMeasurer,
    theme: &Theme,
) -> Result<LaidTree, LayoutError> {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let mut meta: Meta = Vec::new();
    let mut overlays: Vec<NodeId> = Vec::new();

    let root = {
        let mut cx =
            BuildCx { tree: &mut tree, meta: &mut meta, m: measurer, theme, overlays: &mut overlays };
        build(&mut cx, view).map_err(|e| LayoutError(format!("{e:?}")))?
    };

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

    reflow_wrapped_text(&mut tree, root, &meta, measurer, viewport)?;
    reflow_alert_heights(&mut tree, root, &meta, measurer, theme, viewport)?;

    // Flatten with accumulated absolute positions, in paint (pre-)order.
    let mut items = Vec::with_capacity(meta.len());
    let mut seen = std::collections::HashSet::new();
    flatten(&tree, root, (0.0, 0.0), &meta, &mut items, &mut seen, None, None)?;

    // Overlay pass (`<Dialog>`): each detached scrim root computes at the
    // full viewport and flattens *after* the main items — top of the paint
    // order, first in reverse hit order (the modal barrier).
    for overlay in overlays {
        let mut style =
            tree.style(overlay).map_err(|e| LayoutError(format!("{e:?}")))?.clone();
        style.size = Size { width: length(viewport.0), height: length(viewport.1) };
        tree.set_style(overlay, style).map_err(|e| LayoutError(format!("{e:?}")))?;
        tree.compute_layout(
            overlay,
            Size {
                width: AvailableSpace::Definite(viewport.0),
                height: AvailableSpace::Definite(viewport.1),
            },
        )
        .map_err(|e| LayoutError(format!("{e:?}")))?;
        flatten(&tree, overlay, (0.0, 0.0), &meta, &mut items, &mut seen, None, None)?;
    }

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
                clip: None,
                scroll: None,
                menu: None,
            });
        }
    }

    Ok(LaidTree { items, width: viewport.0, height: viewport.1 })
}

/// After the first layout pass, `<Text wrap>` nodes get a height from their
/// assigned width so wrapped copy doesn't clip.
fn reflow_wrapped_text(
    tree: &mut TaffyTree<()>,
    root: NodeId,
    meta: &Meta,
    measurer: &dyn TextMeasurer,
    viewport: (f32, f32),
) -> Result<(), LayoutError> {
    let space = Size {
        width: AvailableSpace::Definite(viewport.0),
        height: AvailableSpace::Definite(viewport.1),
    };
    let mut changed = false;
    for (id, kind, _, _) in meta {
        let ItemKind::Text { content, px, style, wrap: true, .. } = kind else {
            continue;
        };
        let laid = tree.layout(*id).map_err(|e| LayoutError(format!("{e:?}")))?;
        let max_w = laid.size.width;
        if max_w <= 0.0 {
            continue;
        }
        let h = catalog::text::wrapped_height(content, max_w, *px, *style, measurer);
        if (laid.size.height - h).abs() > 0.5 {
            let mut st = tree.style(*id).map_err(|e| LayoutError(format!("{e:?}")))?.clone();
            st.size.height = length(h);
            tree.set_style(*id, st).map_err(|e| LayoutError(format!("{e:?}")))?;
            changed = true;
        }
    }
    if changed {
        tree.compute_layout(root, space).map_err(|e| LayoutError(format!("{e:?}")))?;
    }
    Ok(())
}

/// After the first layout pass, `<Alert>` nodes get a height from their
/// assigned width so wrapped title/body copy doesn't clip.
fn reflow_alert_heights(
    tree: &mut TaffyTree<()>,
    root: NodeId,
    meta: &Meta,
    measurer: &dyn TextMeasurer,
    theme: &Theme,
    viewport: (f32, f32),
) -> Result<(), LayoutError> {
    let space = Size {
        width: AvailableSpace::Definite(viewport.0),
        height: AvailableSpace::Definite(viewport.1),
    };
    let pad = theme.space(4);
    let line_gap = theme.space(1);
    for _ in 0..4 {
        let mut changed = false;
        for (id, kind, _, _) in meta {
            let ItemKind::Alert { title, body, px_md, px_sm, .. } = kind else {
                continue;
            };
            let laid = tree.layout(*id).map_err(|e| LayoutError(format!("{e:?}")))?;
            let max_w = laid.size.width;
            if max_w <= 0.0 {
                continue;
            }
            let h =
                catalog::surface::alert_height(title, body, max_w, *px_md, *px_sm, pad, line_gap, measurer);
            if (laid.size.height - h).abs() > 0.5 {
                let mut st = tree.style(*id).map_err(|e| LayoutError(format!("{e:?}")))?.clone();
                st.size.height = length(h);
                tree.set_style(*id, st).map_err(|e| LayoutError(format!("{e:?}")))?;
                changed = true;
            }
        }
        if !changed {
            break;
        }
        tree.compute_layout(root, space).map_err(|e| LayoutError(format!("{e:?}")))?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn flatten(
    tree: &TaffyTree<()>,
    node: NodeId,
    origin: (f32, f32),
    meta: &[(NodeId, ItemKind, Semantics, Option<(String, Option<String>)>)],
    out: &mut Vec<Item>,
    seen: &mut std::collections::HashSet<NodeId>,
    // PLAN-SCROLL: the enclosing `<Scroll>` (test_id, viewport rect), if any —
    // descendants inherit it as their clip + scroll tag.
    scroll_ctx: Option<(String, Rect)>,
    // M7: the enclosing `<ContextMenu>` panel's test_id, if any — tagged
    // items translate together in `apply_menu_anchor`.
    menu_ctx: Option<String>,
) -> Result<(), LayoutError> {
    let l = tree.layout(node).map_err(|e| LayoutError(format!("{e:?}")))?;
    let abs = (origin.0 + l.location.x, origin.1 + l.location.y);
    let rect = Rect::new(abs.0, abs.1, l.size.width, l.size.height);

    let mut child_ctx = scroll_ctx.clone();
    let mut child_menu = menu_ctx.clone();
    if let Some((id, kind, sem, press)) = meta.iter().find(|(id, ..)| *id == node) {
        seen.insert(*id);
        let (clip, scroll) = match &scroll_ctx {
            Some((sid, vp)) => (Some(*vp), Some(sid.clone())),
            None => (None, None),
        };
        // The panel tags itself and its descendants (barrier stays untagged
        // — it must keep covering the viewport, not follow the anchor).
        let menu_tag = if matches!(kind, ItemKind::MenuPanel) {
            sem.test_id.clone()
        } else {
            menu_ctx.clone()
        };
        out.push(Item {
            kind: kind.clone(),
            rect,
            semantics: sem.clone(),
            on_press: press.as_ref().map(|(name, _)| name.clone()),
            on_press_arg: press.as_ref().and_then(|(_, arg)| arg.clone()),
            clip,
            scroll,
            menu: menu_tag.clone(),
        });
        // A `<Scroll>` viewport becomes the scroll context for its descendants
        // (the viewport itself keeps the outer context — unclipped in v1).
        if matches!(kind, ItemKind::Scroll) {
            child_ctx = sem.test_id.clone().map(|tid| (tid, rect));
        }
        if matches!(kind, ItemKind::MenuPanel) {
            child_menu = menu_tag;
        }
    }
    for child in tree.children(node).map_err(|e| LayoutError(format!("{e:?}")))? {
        flatten(tree, child, abs, meta, out, seen, child_ctx.clone(), child_menu.clone())?;
    }
    Ok(())
}

/// Flex `Style` for a `Column`/`Row` from its `BoxStyle`. Shared by the
/// container component.
pub(crate) fn box_style(s: &BoxStyle, dir: FlexDirection) -> Style {
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
        // The flexbox `min-height: 0` reset: without it, a flex container's
        // default `min-height: auto` sizes it to its content, so a definite
        // outer height never constrains a nested `<Scroll>` — its content
        // would inflate the whole chain instead of overflowing (PLAN-SCROLL).
        min_size: Size { width: auto(), height: length(0.0) },
        align_items: if s.cross_center { Some(AlignItems::Center) } else { None },
        justify_content: if s.main_center { Some(JustifyContent::Center) } else { None },
        ..Default::default()
    }
}

/// §12.5: is this child a mounted source element? Used by containers to keep
/// all three kinds (`Frames`/`Interval`/`Stream`) out of the flex tree —
/// they're registered in `meta` but never a taffy child.
pub(crate) fn is_source_node(n: &ViewNode) -> bool {
    matches!(
        n,
        ViewNode::SourceFrames { .. } | ViewNode::SourceInterval { .. } | ViewNode::SourceStream { .. }
    )
}

/// A child that builds but must NOT join its parent's flex tree: §12.5
/// sources (zero-layout mounts) and `<Dialog>` (a detached overlay root).
pub(crate) fn is_detached_node(n: &ViewNode) -> bool {
    is_source_node(n)
        || matches!(n, ViewNode::Dialog { .. } | ViewNode::ContextMenu { .. } | ViewNode::Drawer { .. })
}
