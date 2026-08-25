//! `<Toolbar>` — app chrome strip (§7.1, PLAN-CATALOG wave 7).
//!
//! A dressed Row (or Column when `vertical`): token padding/gap, cross
//! centered, a 1px `theme.hairline` on the trailing edge. Children are
//! ordinary markup (ghost Buttons, Tabs, Spacer). Gloss sites: the
//! left icon rail (`vertical`), the list header, and the editor chrome
//! row (Back / Fwd / Tabs / +).
//!
//! `background="surface_raised"` (default) paints the strip fill from a
//! theme token. `compact` on a vertical rail reserves a title-bar band
//! matching the compact horizontal row so the rail's top rule meets the
//! list toolbar, and the trailing hairline starts there instead of the
//! window top.
//!
//! Floor the hairline (tiny-skia AA hairline gotcha).

use crate::catalog::icon::ICON;
use crate::layout::{is_detached_node, BuildCx};
use crate::node::*;
use crate::paint::PaintCx;
use libra_draw::{Color, DrawCmd, Rect, Theme};
use taffy::prelude::*;

/// Icon-button row + compact bottom pad. Same math as a compact
/// horizontal toolbar of icon-only buttons (Gloss list header).
pub(crate) fn compact_chrome_height(theme: &Theme) -> f32 {
    theme.dense(ICON) + 2.0 * theme.btn_pad_y + theme.space(1)
}

fn toolbar_fill(theme: &Theme, background: Option<&str>) -> Color {
    match background {
        None => theme.surface_raised,
        Some(name) => theme.color_token(name).unwrap_or(theme.surface_raised),
    }
}

pub(crate) fn build(
    cx: &mut BuildCx,
    vertical: bool,
    compact: bool,
    background: &Option<String>,
    children: &[ViewNode],
    test_id: &Option<String>,
) -> Result<NodeId, taffy::TaffyError> {
    let mut kids = Vec::new();
    for c in children {
        if is_detached_node(c) {
            let _ = cx.child(c)?;
            continue;
        }
        kids.push(cx.child(c)?);
    }
    let pad = if compact { cx.theme.space(1) } else { cx.theme.space(2) };
    let gap = cx.theme.space(2);
    let chrome = if vertical && compact { compact_chrome_height(cx.theme) } else { 0.0 };
    let style = if vertical {
        Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            align_items: Some(AlignItems::Center),
            gap: Size { width: length(0.0), height: length(gap) },
            padding: taffy::Rect {
                left: length(pad),
                right: length(pad),
                top: length(chrome),
                bottom: length(pad),
            },
            size: Size { width: auto(), height: percent(1.0) },
            ..Default::default()
        }
    } else {
        Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            align_items: Some(AlignItems::Center),
            gap: Size { width: length(gap), height: length(0.0) },
            padding: taffy::Rect {
                left: length(pad),
                right: length(pad),
                top: length(if compact { 0.0 } else { pad }),
                bottom: length(pad),
            },
            size: Size {
                width: percent(1.0),
                height: if compact { length(compact_chrome_height(cx.theme)) } else { auto() },
            },
            ..Default::default()
        }
    };
    let id = cx.tree.new_with_children(style, &kids)?;
    cx.meta.push((
        id,
        ItemKind::ToolbarBox {
            vertical,
            compact,
            background: background.clone(),
        },
        Semantics {
            role: Role::Container,
            text: String::new(),
            test_id: test_id.clone(),
        },
        None,
    ));
    Ok(id)
}

pub(crate) fn paint(
    cx: &mut PaintCx,
    item: &Item,
    vertical: bool,
    compact: bool,
    background: Option<&str>,
) {
    let r = item.rect;
    let c = cx.theme.hairline;
    let fill = toolbar_fill(cx.theme, background);
    if fill != Color::TRANSPARENT {
        cx.cmds.push(DrawCmd::Rect { rect: r, color: fill });
    }
    if vertical {
        let band = if compact { compact_chrome_height(cx.theme) } else { 0.0 };
        let y0 = (r.y + band - if band > 0.0 { 1.0 } else { 0.0 }).floor();
        if band > 0.0 {
            cx.cmds.push(DrawCmd::Rect {
                rect: Rect::new(r.x.floor(), y0, r.w.max(1.0), 1.0),
                color: c,
            });
        }
        let x = (r.x + r.w - 1.0).floor();
        let h = (r.y + r.h - y0).max(1.0);
        cx.cmds.push(DrawCmd::Rect {
            rect: Rect::new(x, y0, 1.0, h),
            color: c,
        });
    } else {
        let y = (r.y + r.h - 1.0).floor();
        cx.cmds.push(DrawCmd::Rect {
            rect: Rect::new(r.x.floor(), y, r.w.max(1.0), 1.0),
            color: c,
        });
    }
}
