//! Progress — a determinate progress bar (§7.1, PLAN-CATALOG wave 2). `value`
//! 0..=1 fills an accent bar over a `surface_raised` track; pill-rounded.
//! Tokens only (C3 guard). Grows to fill its row; use inside a `<Row>`.

use crate::layout::BuildCx;
use crate::node::*;
use crate::paint::PaintCx;
use libra_draw::{DrawCmd, Rect};
use taffy::prelude::*;

pub(crate) fn build(
    cx: &mut BuildCx,
    value: f32,
    test_id: &Option<String>,
) -> Result<NodeId, taffy::TaffyError> {
    let v = value.clamp(0.0, 1.0);
    let h = cx.theme.space(4); // ~8px track (density-scaled)
    let id = cx.tree.new_leaf(Style {
        size: Size { width: auto(), height: length(h) },
        flex_grow: 1.0,
        flex_shrink: 1.0,
        ..Default::default()
    })?;
    cx.meta.push((
        id,
        ItemKind::Progress { value: v },
        // Announced as its percentage — a dedicated progressbar role with
        // value-now is future a11y polish (§17).
        Semantics { role: Role::Text, text: format!("{:.0}%", v * 100.0), test_id: test_id.clone() },
        None,
    ));
    Ok(id)
}

pub(crate) fn paint(cx: &mut PaintCx, item: &Item, value: f32) {
    let theme = cx.theme;
    let r = item.rect;
    let radius = (r.h / 2.0).min(theme.radius_full);
    cx.cmds.push(DrawCmd::RRect { rect: r, radius, color: theme.surface_raised });
    let fill_w = r.w * value.clamp(0.0, 1.0);
    if fill_w > 0.5 {
        cx.cmds.push(DrawCmd::RRect {
            rect: Rect::new(r.x, r.y, fill_w, r.h),
            radius,
            color: theme.accent,
        });
    }
}
