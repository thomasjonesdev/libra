//! Pagination — a fully generated page-nav row (§7.1, PLAN-CATALOG):
//! Prev/Next + a windowed run of page buttons around `current`
//! (ellipsis for the rest). The windowing lives here once, shared by
//! both engines (they only carry `current`/`total`/`on_page` in the
//! `ViewNode` — no duplicated logic to drift).

use crate::layout::BuildCx;
use crate::node::*;
use crate::paint::PaintCx;
use libra_draw::{Color, DrawCmd, FontStyle, Rect};
use taffy::prelude::*;

enum Page {
    Num(u32),
    Ellipsis,
}

/// Which pages to show: all of them if `total <= 7`; otherwise the
/// first, the last, an up-to-3-wide window around `current`, and
/// ellipses filling the gaps. Pure function of `(current, total)` —
/// deterministic, so both engines' identical `ViewNode` produces an
/// identical generated row.
fn windowed_pages(current: u32, total: u32) -> Vec<Page> {
    if total == 0 {
        return vec![];
    }
    if total <= 7 {
        return (0..total).map(Page::Num).collect();
    }
    let mut out = vec![Page::Num(0)];
    let lo = current.saturating_sub(1).max(1);
    let hi = (current + 1).min(total - 2);
    if lo > 1 {
        out.push(Page::Ellipsis);
    }
    for n in lo..=hi {
        out.push(Page::Num(n));
    }
    if hi + 1 < total - 1 {
        out.push(Page::Ellipsis);
    }
    out.push(Page::Num(total - 1));
    out
}

pub(crate) fn build(
    cx: &mut BuildCx,
    current: u32,
    total: u32,
    on_page: &str,
    test_id: &Option<String>,
) -> Result<NodeId, taffy::TaffyError> {
    let mut kids = Vec::new();
    kids.push(build_nav(cx, "‹", current > 0, on_page, current.saturating_sub(1))?);
    for p in windowed_pages(current, total) {
        kids.push(match p {
            Page::Num(n) => build_page(cx, n, n == current, on_page)?,
            Page::Ellipsis => build_ellipsis(cx)?,
        });
    }
    let next_enabled = current + 1 < total;
    kids.push(build_nav(cx, "›", next_enabled, on_page, (current + 1).min(total.saturating_sub(1)))?);

    let id = cx.tree.new_with_children(
        Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            gap: Size { width: length(cx.theme.space(1)), height: length(0.0) },
            align_items: Some(AlignItems::Center),
            ..Default::default()
        },
        &kids,
    )?;
    cx.meta.push((
        id,
        ItemKind::PaginationBox,
        Semantics { role: Role::Container, text: String::new(), test_id: test_id.clone() },
        None,
    ));
    Ok(id)
}

fn button_leaf(
    cx: &mut BuildCx,
    label: &str,
    active: bool,
    enabled: bool,
    handler: Option<(String, Option<String>)>,
) -> Result<NodeId, taffy::TaffyError> {
    let px = cx.theme.type_sm;
    let (tw, th) = cx.m.measure(label, px, FontStyle::Regular);
    let (w, h) = (tw + 2.0 * cx.theme.btn_pad_x, th + 2.0 * cx.theme.btn_pad_y);
    let id = cx.tree.new_leaf(Style {
        size: Size { width: length(w), height: length(h) },
        flex_shrink: 0.0,
        ..Default::default()
    })?;
    cx.meta.push((
        id,
        ItemKind::PageButton { label: label.to_string(), active, enabled },
        Semantics { role: Role::Button, text: label.to_string(), test_id: None },
        handler,
    ));
    Ok(id)
}

fn build_nav(
    cx: &mut BuildCx,
    glyph: &str,
    enabled: bool,
    on_page: &str,
    target: u32,
) -> Result<NodeId, taffy::TaffyError> {
    let handler = enabled.then(|| (on_page.to_string(), Some(target.to_string())));
    button_leaf(cx, glyph, false, enabled, handler)
}

fn build_page(cx: &mut BuildCx, n: u32, active: bool, on_page: &str) -> Result<NodeId, taffy::TaffyError> {
    let label = (n + 1).to_string(); // 1-based display
    let handler = (!active).then(|| (on_page.to_string(), Some(n.to_string())));
    button_leaf(cx, &label, active, true, handler)
}

fn build_ellipsis(cx: &mut BuildCx) -> Result<NodeId, taffy::TaffyError> {
    let px = cx.theme.type_sm;
    let (w, h) = cx.m.measure("…", px, FontStyle::Regular);
    let id = cx.tree.new_leaf(Style {
        size: Size { width: length(w + cx.theme.space(2)), height: length(h) },
        flex_shrink: 0.0,
        ..Default::default()
    })?;
    cx.meta.push((
        id,
        ItemKind::PageEllipsis,
        Semantics { role: Role::Container, text: String::new(), test_id: None },
        None,
    ));
    Ok(id)
}

pub(crate) fn paint_button(cx: &mut PaintCx, item: &Item, label: &str, active: bool, enabled: bool) {
    let (fill, text_color): (Option<Color>, Color) = if active {
        (Some(cx.theme.accent), cx.theme.accent_text)
    } else if enabled {
        (None, cx.theme.text)
    } else {
        (None, cx.theme.text_muted)
    };
    if let Some(fill) = fill {
        cx.cmds.push(DrawCmd::RRect { rect: item.rect, radius: cx.theme.radius_sm, color: fill });
    }
    let px = cx.theme.type_sm;
    let (tw, th) = cx.measurer.measure(label, px, FontStyle::Regular);
    cx.cmds.push(DrawCmd::Text {
        x: item.rect.x + (item.rect.w - tw) / 2.0,
        y: item.rect.y + (item.rect.h - th) / 2.0,
        content: label.to_string(),
        px,
        style: FontStyle::Regular,
        color: text_color,
    });
}

pub(crate) fn paint_ellipsis(cx: &mut PaintCx, rect: Rect) {
    cx.cmds.push(DrawCmd::Text {
        x: rect.x,
        y: rect.y,
        content: "…".to_string(),
        px: cx.theme.type_sm,
        style: FontStyle::Regular,
        color: cx.theme.text_muted,
    });
}
