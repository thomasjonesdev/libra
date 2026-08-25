//! `<StatusBar>` — footer chrome (§7.1, PLAN-CATALOG wave 7).
//!
//! A dressed Row: token padding/gap, cross centered, a 1px
//! `theme.hairline` on the **top** edge (the footer sits under a
//! pane). Children are ordinary markup — Gloss uses muted `Text`
//! slots for status / word count / save label. Replaces the homemade
//! `Separator` + `Row` footer.
//!
//! Floor the hairline (tiny-skia AA hairline gotcha).

use crate::layout::{is_detached_node, BuildCx};
use crate::node::*;
use crate::paint::PaintCx;
use libra_draw::{DrawCmd, Rect};
use taffy::prelude::*;

pub(crate) fn build(
    cx: &mut BuildCx,
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
    let id = cx.tree.new_with_children(
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
        },
        &kids,
    )?;
    cx.meta.push((
        id,
        ItemKind::StatusBarBox,
        Semantics {
            role: Role::Container,
            text: String::new(),
            test_id: test_id.clone(),
        },
        None,
    ));
    Ok(id)
}

pub(crate) fn paint(cx: &mut PaintCx, item: &Item) {
    let r = item.rect;
    let y = r.y.floor();
    cx.cmds.push(DrawCmd::Rect {
        rect: Rect::new(r.x.floor(), y, r.w.max(1.0), 1.0),
        color: cx.theme.hairline,
    });
}
