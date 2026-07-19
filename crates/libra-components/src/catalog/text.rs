//! Text — a measured single-line text leaf (§7.1). The smallest build+paint
//! component: a good template to copy.

use crate::layout::BuildCx;
use crate::node::*;
use crate::paint::PaintCx;
use libra_draw::{Color, DrawCmd, FontStyle};
use taffy::prelude::*;

pub(crate) fn build(
    cx: &mut BuildCx,
    content: &str,
    size: TextSize,
    bold: bool,
    muted: bool,
    grow: bool,
    test_id: &Option<String>,
) -> Result<NodeId, taffy::TaffyError> {
    let theme = cx.theme;
    let px = size.px(theme);
    let style = if bold { FontStyle::Bold } else { FontStyle::Regular };
    let (w, h) = cx.m.measure(content, px, style);
    let id = cx.tree.new_leaf(Style {
        size: Size { width: length(w), height: length(h) },
        flex_grow: if grow { 1.0 } else { 0.0 },
        flex_shrink: 0.0,
        ..Default::default()
    })?;
    let color = if muted { theme.text_muted } else { theme.text };
    cx.meta.push((
        id,
        ItemKind::Text { content: content.to_string(), px, style, color },
        Semantics { role: Role::Text, text: content.to_string(), test_id: test_id.clone() },
        None,
    ));
    Ok(id)
}

pub(crate) fn paint(cx: &mut PaintCx, item: &Item, content: &str, px: f32, style: FontStyle, color: Color) {
    // Vertically center within the (measured-height) rect.
    cx.cmds.push(DrawCmd::Text {
        x: item.rect.x,
        y: item.rect.y,
        content: content.to_string(),
        px,
        style,
        color,
    });
}
