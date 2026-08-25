//! Surface containers + static display (§7.1, PLAN-CATALOG): `Card`
//! `Card` (raised, elevated fill), `Panel` (plain), `Alert` (status-token
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
    flat: bool,
    apply_defaults: bool,
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
    // token values instead of 0. `flush` on `<Panel>` skips this so
    // chrome rows can sit at y=0 under a frameless title bar.
    let mut st = *style;
    if apply_defaults {
        if st.padding == 0.0 {
            st.padding = cx.theme.space(5);
        }
        if st.gap == 0.0 {
            st.gap = cx.theme.space(4);
        }
    }
    let id = cx.tree.new_with_children(box_style(&st, FlexDirection::Column), &kids)?;
    cx.meta.push((
        id,
        ItemKind::Surface { raised, flat },
        Semantics { role: Role::Container, text: String::new(), test_id: None },
        None,
    ));
    Ok(id)
}

/// Total laid height for an alert at `max_w`, with title/body soft-wrapped.
pub(crate) fn alert_height(
    title: &str,
    body: &str,
    max_w: f32,
    px_md: f32,
    px_sm: f32,
    pad: f32,
    line_gap: f32,
    m: &dyn TextMeasurer,
) -> f32 {
    let content_w = (max_w - 2.0 * pad).max(0.0);
    let (_, th_title) = m.measure("Ag", px_md, FontStyle::Bold);
    let title_lines = catalog::text::wrap_lines(title, content_w, px_md, FontStyle::Bold, m);
    let mut h = 2.0 * pad + title_lines.len() as f32 * th_title;
    if !body.is_empty() {
        let (_, th_sm) = m.measure("Ag", px_sm, FontStyle::Regular);
        let body_lines = catalog::text::wrap_lines(body, content_w, px_sm, FontStyle::Regular, m);
        h += line_gap + body_lines.len() as f32 * th_sm;
    }
    h
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
    let pad = cx.theme.space(4);
    let line_gap = cx.theme.space(1);
    // Single-line estimate — reflow_alert_heights fixes this after width is known.
    let h = alert_height(title, body, 320.0, px_md, px_sm, pad, line_gap, cx.m);
    let id = cx.tree.new_leaf(Style {
        size: Size { width: percent(1.0), height: length(h) },
        flex_shrink: 0.0,
        min_size: Size { width: length(0.0), height: auto() },
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

pub(crate) fn paint_surface(cx: &mut PaintCx, rect: Rect, raised: bool, flat: bool) {
    if !raised {
        cx.cmds.push(DrawCmd::RRect { rect, radius: cx.theme.radius_md, color: cx.theme.surface });
    } else if flat {
        cx.cmds.push(DrawCmd::Rect { rect, color: cx.theme.surface_raised });
    } else {
        // Card: elevated fill only — hairline framing is Dialog chrome.
        cx.cmds.push(DrawCmd::RRect {
            rect,
            radius: cx.theme.radius_lg,
            color: cx.theme.surface_raised,
        });
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
    let line_gap = cx.theme.space(1);
    let content_w = (rect.w - 2.0 * pad).max(0.0);
    let (tx, mut ty) = (rect.x + pad, rect.y + pad);
    let (_, th_title) = cx.measurer.measure("Ag", px_md, FontStyle::Bold);
    for line in catalog::text::wrap_lines(title, content_w, px_md, FontStyle::Bold, cx.measurer) {
        cx.cmds.push(DrawCmd::Text {
            x: tx,
            y: ty,
            content: line,
            px: px_md,
            style: FontStyle::Bold,
            color: fg,
        });
        ty += th_title;
    }
    if !body.is_empty() {
        ty += line_gap;
        let (_, th_sm) = cx.measurer.measure("Ag", px_sm, FontStyle::Regular);
        for line in catalog::text::wrap_lines(body, content_w, px_sm, FontStyle::Regular, cx.measurer) {
            cx.cmds.push(DrawCmd::Text {
                x: tx,
                y: ty,
                content: line,
                px: px_sm,
                style: FontStyle::Regular,
                color: fg,
            });
            ty += th_sm;
        }
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
