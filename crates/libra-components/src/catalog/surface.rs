//! Surface containers + static display (§7.1, PLAN-CATALOG): `Card`
//! (raised, hairline-framed), `Panel` (plain), `Alert` (status-token
//! callout), `Skeleton` (static placeholder), `Avatar` (initials).
//! All token-only paint.

use crate::catalog;
use crate::layout::{box_style, is_detached_node, BuildCx};
use crate::node::*;
use crate::paint::PaintCx;
use libra_draw::{DrawCmd, FontStyle, Rect};
use taffy::prelude::*;

pub(crate) fn build_surface(
    cx: &mut BuildCx,
    style: &BoxStyle,
    children: &[ViewNode],
    raised: bool,
) -> Result<NodeId, taffy::TaffyError> {
    let mut kids = Vec::new();
    for c in children {
        if is_detached_node(c) {
            let _ = cx.child(c)?;
        } else {
            kids.push(cx.child(c)?);
        }
    }
    // Column semantics with surface defaults: unset padding/gap get
    // token values instead of 0.
    let mut st = *style;
    if st.padding == 0.0 {
        st.padding = cx.theme.space(5);
    }
    if st.gap == 0.0 {
        st.gap = cx.theme.space(4);
    }
    let id = cx.tree.new_with_children(box_style(&st, FlexDirection::Column), &kids)?;
    cx.meta.push((
        id,
        ItemKind::Surface { raised },
        Semantics { role: Role::Container, text: String::new(), test_id: None },
        None,
    ));
    Ok(id)
}

pub(crate) fn build_alert(
    cx: &mut BuildCx,
    tone: Tone,
    title: &str,
    body: &str,
    test_id: &Option<String>,
) -> Result<NodeId, taffy::TaffyError> {
    let px_md = cx.theme.type_md;
    let px_sm = cx.theme.type_sm;
    let (_, th_md) = cx.m.measure("Ag", px_md, FontStyle::Regular);
    let (_, th_sm) = cx.m.measure("Ag", px_sm, FontStyle::Regular);
    let pad = cx.theme.space(4);
    let h = 2.0 * pad + th_md + if body.is_empty() { 0.0 } else { th_sm + cx.theme.space(1) };
    let id = cx.tree.new_leaf(Style {
        size: Size { width: auto(), height: length(h) },
        flex_shrink: 0.0,
        ..Default::default()
    })?;
    cx.meta.push((
        id,
        ItemKind::Alert { tone, title: title.to_string(), body: body.to_string(), px_md, px_sm },
        Semantics { role: Role::Text, text: title.to_string(), test_id: test_id.clone() },
        None,
    ));
    Ok(id)
}

pub(crate) fn build_skeleton(
    cx: &mut BuildCx,
    width: Option<f32>,
    height: f32,
) -> Result<NodeId, taffy::TaffyError> {
    let id = cx.tree.new_leaf(Style {
        size: Size { width: width.map(length).unwrap_or(auto()), height: length(height) },
        flex_shrink: 0.0,
        ..Default::default()
    })?;
    cx.meta.push((
        id,
        ItemKind::Skeleton,
        Semantics { role: Role::Container, text: String::new(), test_id: None },
        None,
    ));
    Ok(id)
}

/// First letters of up to two words, uppercased.
pub(crate) fn initials(name: &str) -> String {
    name.split_whitespace()
        .take(2)
        .filter_map(|w| w.chars().next())
        .flat_map(|c| c.to_uppercase())
        .collect()
}

pub(crate) fn build_avatar(
    cx: &mut BuildCx,
    name: &str,
    size: f32,
    test_id: &Option<String>,
) -> Result<NodeId, taffy::TaffyError> {
    let id = cx.tree.new_leaf(Style {
        size: Size { width: length(size), height: length(size) },
        flex_shrink: 0.0,
        ..Default::default()
    })?;
    let initials = initials(name);
    cx.meta.push((
        id,
        ItemKind::Avatar { initials: initials.clone(), px: (size * 0.42).round() },
        Semantics { role: Role::Text, text: initials, test_id: test_id.clone() },
        None,
    ));
    Ok(id)
}

pub(crate) fn paint_surface(cx: &mut PaintCx, rect: Rect, raised: bool) {
    if raised {
        // Card: the Dialog panel chrome — one surface language.
        catalog::dialog::paint_panel(cx, rect);
    } else {
        cx.cmds.push(DrawCmd::RRect { rect, radius: cx.theme.radius_md, color: cx.theme.surface });
    }
}

pub(crate) fn paint_alert(
    cx: &mut PaintCx,
    rect: Rect,
    tone: Tone,
    title: &str,
    body: &str,
    px_md: f32,
    px_sm: f32,
) {
    let (bg, fg) = catalog::badge::tone_colors(cx.theme, tone);
    cx.cmds.push(DrawCmd::RRect { rect, radius: cx.theme.radius_md, color: bg });
    let pad = cx.theme.space(4);
    let (tx, ty) = (rect.x + pad, rect.y + pad);
    cx.cmds.push(DrawCmd::Text {
        x: tx,
        y: ty,
        content: title.to_string(),
        px: px_md,
        style: FontStyle::Bold,
        color: fg,
    });
    if !body.is_empty() {
        let (_, th_md) = cx.measurer.measure("Ag", px_md, FontStyle::Regular);
        cx.cmds.push(DrawCmd::Text {
            x: tx,
            y: ty + th_md + cx.theme.space(1),
            content: body.to_string(),
            px: px_sm,
            style: FontStyle::Regular,
            color: fg,
        });
    }
}

pub(crate) fn paint_skeleton(cx: &mut PaintCx, rect: Rect) {
    cx.cmds.push(DrawCmd::RRect { rect, radius: cx.theme.radius_sm, color: cx.theme.surface_raised });
}

pub(crate) fn paint_avatar(cx: &mut PaintCx, rect: Rect, initials: &str, px: f32) {
    cx.cmds.push(DrawCmd::RRect {
        rect,
        radius: cx.theme.radius_full,
        color: cx.theme.secondary,
    });
    let (tw, th) = cx.measurer.measure(initials, px, FontStyle::Bold);
    cx.cmds.push(DrawCmd::Text {
        x: rect.x + (rect.w - tw) / 2.0,
        y: rect.y + (rect.h - th) / 2.0,
        content: initials.to_string(),
        px,
        style: FontStyle::Bold,
        color: cx.theme.text,
    });
}
