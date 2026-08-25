//! `<Toolbar>` — app chrome strip (§7.1, PLAN-CATALOG wave 7).
//!
//! A dressed Row (or Column when `vertical`): token padding/gap, cross
//! centered, a 1px `theme.hairline` on the trailing edge. Children are
//! ordinary markup (ghost Buttons, Tabs, Spacer). Gloss sites: the
//! left icon rail (`vertical`), the list header, and the editor chrome
//! row (Back / Fwd / Tabs / +).
//!
//! Floor the hairline (tiny-skia AA hairline gotcha).

use crate::layout::{is_detached_node, BuildCx};
use crate::node::*;
use crate::paint::PaintCx;
use libra_draw::{DrawCmd, Rect};
use taffy::prelude::*;

pub(crate) fn build(
    cx: &mut BuildCx,
    vertical: bool,
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
    let pad = cx.theme.space(2);
    let gap = cx.theme.space(2);
    let style = if vertical {
        Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            align_items: Some(AlignItems::Center),
            gap: Size { width: length(0.0), height: length(gap) },
            padding: taffy::Rect {
                left: length(pad),
                right: length(pad),
                top: length(pad),
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
                top: length(pad),
                bottom: length(pad),
            },
            size: Size { width: percent(1.0), height: auto() },
            ..Default::default()
        }
    };
    let id = cx.tree.new_with_children(style, &kids)?;
    cx.meta.push((
        id,
        ItemKind::ToolbarBox { vertical },
        Semantics {
            role: Role::Container,
            text: String::new(),
            test_id: test_id.clone(),
        },
        None,
    ));
    Ok(id)
}

pub(crate) fn paint(cx: &mut PaintCx, item: &Item, vertical: bool) {
    let r = item.rect;
    let c = cx.theme.hairline;
    if vertical {
        cx.cmds.push(DrawCmd::Rect { rect: r, color: cx.theme.surface_raised });
        let x = (r.x + r.w - 1.0).floor();
        cx.cmds.push(DrawCmd::Rect {
            rect: Rect::new(x, r.y.floor(), 1.0, r.h.max(1.0)),
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
