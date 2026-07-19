//! Separator — a 1px `theme.hairline` rule (§7.1, PLAN-CATALOG wave 1;
//! PLAN-TOKENS T1). Horizontal across a `Column`, `vertical` for pane seams
//! in a `Row`.

use crate::layout::BuildCx;
use crate::node::*;
use crate::paint::PaintCx;
use libra_draw::DrawCmd;
use taffy::prelude::*;

pub(crate) fn build(cx: &mut BuildCx, vertical: bool) -> Result<NodeId, taffy::TaffyError> {
    // 1px along the parent's main axis, stretched across the cross axis
    // (taffy's default align-items: stretch).
    let style = if vertical {
        Style {
            size: Size { width: length(1.0), height: auto() },
            flex_shrink: 0.0,
            ..Default::default()
        }
    } else {
        Style {
            size: Size { width: auto(), height: length(1.0) },
            flex_shrink: 0.0,
            ..Default::default()
        }
    };
    let id = cx.tree.new_leaf(style)?;
    cx.meta.push((
        id,
        ItemKind::Separator,
        Semantics { role: Role::Separator, text: String::new(), test_id: None },
        None,
    ));
    Ok(id)
}

pub(crate) fn paint(cx: &mut PaintCx, item: &Item) {
    cx.cmds.push(DrawCmd::Rect { rect: item.rect, color: cx.theme.hairline });
}
