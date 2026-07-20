//! Tooltip — a floating label riding the deterministic hover input
//! (§7.1, PLAN-CATALOG): the wrapper is an invisible container; when
//! the hover point is inside it, the paint pass\'s second phase draws
//! the label above it, on top of everything, clamped to the viewport.

use crate::layout::{box_style, is_detached_node, BuildCx};
use crate::node::*;
use crate::paint::PaintCx;
use libra_draw::{DrawCmd, FontStyle, Rect};
use taffy::prelude::*;

pub(crate) fn build(
    cx: &mut BuildCx,
    text: &str,
    children: &[ViewNode],
) -> Result<NodeId, taffy::TaffyError> {
    let mut kids = Vec::new();
    for c in children {
        if is_detached_node(c) {
            let _ = cx.child(c)?;
        } else {
            kids.push(cx.child(c)?);
        }
    }
    let style = BoxStyle::default();
    let id = cx.tree.new_with_children(box_style(&style, FlexDirection::Column), &kids)?;
    cx.meta.push((
        id,
        ItemKind::TooltipHost { text: text.to_string() },
        Semantics { role: Role::Container, text: text.to_string(), test_id: None },
        None,
    ));
    Ok(id)
}

/// Second paint phase: the hovered host\'s label, drawn after every
/// item so it tops the z-order.
pub(crate) fn paint_label(cx: &mut PaintCx, host: Rect, text: &str, viewport: (f32, f32)) {
    let px = cx.theme.type_sm;
    let (tw, th) = cx.measurer.measure(text, px, FontStyle::Regular);
    let pad = cx.theme.space(2);
    let (w, h) = (tw + 2.0 * pad, th + 2.0 * pad);
    let x = (host.x + (host.w - w) / 2.0).clamp(0.0, (viewport.0 - w).max(0.0));
    let y = (host.y - h - 4.0).max(0.0);
    let r = Rect::new(x, y, w, h);
    cx.cmds.push(DrawCmd::RRect { rect: r, radius: cx.theme.radius_sm, color: cx.theme.surface_raised });
    let c = cx.theme.hairline;
    cx.cmds.push(DrawCmd::Rect { rect: Rect::new(r.x, r.y, r.w, 1.0), color: c });
    cx.cmds.push(DrawCmd::Rect { rect: Rect::new(r.x, r.y + r.h - 1.0, r.w, 1.0), color: c });
    cx.cmds.push(DrawCmd::Rect { rect: Rect::new(r.x, r.y, 1.0, r.h), color: c });
    cx.cmds.push(DrawCmd::Rect { rect: Rect::new(r.x + r.w - 1.0, r.y, 1.0, r.h), color: c });
    cx.cmds.push(DrawCmd::Text {
        x: r.x + pad,
        y: r.y + pad,
        content: text.to_string(),
        px,
        style: FontStyle::Regular,
        color: cx.theme.text,
    });
}
