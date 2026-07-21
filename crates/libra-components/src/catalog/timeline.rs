//! Timeline — a ledger list with per-row indicators and connectors
//! (§7.1, PLAN-CATALOG): each `TimelineItem` carries its own `done`
//! flag directly (unlike Stepper, status here isn't position-derived);
//! the container draws a connecting line between consecutive entries.

use crate::layout::BuildCx;
use crate::node::*;
use crate::paint::PaintCx;
use libra_draw::{DrawCmd, FontStyle, Rect};
use taffy::prelude::*;

const DOT_D: f32 = 12.0;

pub(crate) fn build(cx: &mut BuildCx, children: &[ViewNode]) -> Result<NodeId, taffy::TaffyError> {
    let mut kids = Vec::new();
    let mut prev_done = false;
    let mut first = true;
    for c in children {
        let (title, meta, done, test_id) = match c {
            ViewNode::TimelineItem { title, meta, done, test_id } => {
                (title.clone(), meta.clone(), *done, test_id.clone())
            }
            other if crate::layout::is_detached_node(other) => {
                let _ = cx.child(other)?;
                continue;
            }
            _ => continue,
        };
        if !first {
            kids.push(build_connector(cx, prev_done)?);
        }
        kids.push(build_item(cx, &title, &meta, done, &test_id)?);
        prev_done = done;
        first = false;
    }
    let id = cx.tree.new_with_children(
        Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            ..Default::default()
        },
        &kids,
    )?;
    cx.meta.push((
        id,
        ItemKind::TimelineBox,
        Semantics { role: Role::Container, text: String::new(), test_id: None },
        None,
    ));
    Ok(id)
}

fn build_item(
    cx: &mut BuildCx,
    title: &str,
    meta: &str,
    done: bool,
    test_id: &Option<String>,
) -> Result<NodeId, taffy::TaffyError> {
    let px_md = cx.theme.type_md;
    let px_sm = cx.theme.type_sm;
    let (_, th_md) = cx.m.measure("Ag", px_md, FontStyle::Regular);
    let (_, th_sm) = cx.m.measure("Ag", px_sm, FontStyle::Regular);
    let h = th_md + if meta.is_empty() { 0.0 } else { th_sm + cx.theme.space(1) } + cx.theme.space(2);
    let id = cx.tree.new_leaf(Style {
        size: Size { width: auto(), height: length(h.max(DOT_D)) },
        flex_shrink: 0.0,
        ..Default::default()
    })?;
    cx.meta.push((
        id,
        ItemKind::TimelineDot { title: title.to_string(), meta: meta.to_string(), done, px_md, px_sm },
        Semantics { role: Role::Text, text: title.to_string(), test_id: test_id.clone() },
        None,
    ));
    Ok(id)
}

fn build_connector(cx: &mut BuildCx, filled: bool) -> Result<NodeId, taffy::TaffyError> {
    let id = cx.tree.new_leaf(Style {
        size: Size { width: length(2.0), height: length(cx.theme.space(3)) },
        flex_shrink: 0.0,
        ..Default::default()
    })?;
    cx.meta.push((
        id,
        ItemKind::Connector { vertical: true, filled },
        Semantics { role: Role::Container, text: String::new(), test_id: None },
        None,
    ));
    Ok(id)
}

pub(crate) fn paint_item(
    cx: &mut PaintCx,
    item: &Item,
    title: &str,
    meta: &str,
    done: bool,
    px_md: f32,
    px_sm: f32,
) {
    let dot = Rect::new(item.rect.x, item.rect.y + cx.theme.space(1), DOT_D, DOT_D);
    let (fill, ring) = if done {
        (Some(cx.theme.accent), cx.theme.accent)
    } else {
        (None, cx.theme.secondary)
    };
    if let Some(fill) = fill {
        cx.cmds.push(DrawCmd::RRect { rect: dot, radius: cx.theme.radius_full, color: fill });
    } else {
        cx.cmds.push(DrawCmd::RRect { rect: dot, radius: cx.theme.radius_full, color: cx.theme.bg });
        let c = ring;
        cx.cmds.push(DrawCmd::Rect { rect: Rect::new(dot.x, dot.y, dot.w, 1.0), color: c });
        cx.cmds.push(DrawCmd::Rect { rect: Rect::new(dot.x, dot.y + dot.h - 1.0, dot.w, 1.0), color: c });
        cx.cmds.push(DrawCmd::Rect { rect: Rect::new(dot.x, dot.y, 1.0, dot.h), color: c });
        cx.cmds.push(DrawCmd::Rect { rect: Rect::new(dot.x + dot.w - 1.0, dot.y, 1.0, dot.h), color: c });
    }
    let tx = dot.x + DOT_D + cx.theme.space(2);
    cx.cmds.push(DrawCmd::Text {
        x: tx,
        y: item.rect.y,
        content: title.to_string(),
        px: px_md,
        style: FontStyle::Regular,
        color: cx.theme.text,
    });
    if !meta.is_empty() {
        let (_, th_md) = cx.measurer.measure("Ag", px_md, FontStyle::Regular);
        cx.cmds.push(DrawCmd::Text {
            x: tx,
            y: item.rect.y + th_md + cx.theme.space(1),
            content: meta.to_string(),
            px: px_sm,
            style: FontStyle::Regular,
            color: cx.theme.text_muted,
        });
    }
}

pub(crate) fn paint_connector(cx: &mut PaintCx, item: &Item, filled: bool) {
    let x = item.rect.x.floor() + (DOT_D / 2.0).floor();
    let rect = Rect::new(x, item.rect.y, 2.0, item.rect.h);
    let color = if filled { cx.theme.accent } else { cx.theme.secondary };
    cx.cmds.push(DrawCmd::Rect { rect, color });
}
