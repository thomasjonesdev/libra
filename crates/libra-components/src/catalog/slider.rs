//! Slider — model-owned continuous control (§7.1, PLAN-CATALOG): the
//! Split-drag machinery generalized to a track. `value` binds a State
//! f32; pressing or dragging the track dispatches `onChange` with the
//! new fraction (`{:.4}`, same as Split); the app echoes it back.

use crate::layout::BuildCx;
use crate::node::*;
use crate::paint::PaintCx;
use libra_draw::{DrawCmd, Rect};
use taffy::prelude::*;

pub const HEIGHT: f32 = 20.0;
const TRACK_H: f32 = 4.0;
const KNOB_D: f32 = 14.0;

pub(crate) fn build(
    cx: &mut BuildCx,
    value: f32,
    on_change: &str,
    test_id: &str,
) -> Result<NodeId, taffy::TaffyError> {
    let id = cx.tree.new_leaf(Style {
        size: Size { width: auto(), height: length(HEIGHT) },
        flex_grow: 1.0,
        flex_shrink: 1.0,
        ..Default::default()
    })?;
    cx.meta.push((
        id,
        ItemKind::SliderCtl { value: value.clamp(0.0, 1.0), on_change: on_change.to_string() },
        Semantics {
            role: Role::Container,
            text: format!("{:.4}", value.clamp(0.0, 1.0)),
            test_id: Some(test_id.to_string()),
        },
        None,
    ));
    Ok(id)
}

pub(crate) fn paint(cx: &mut PaintCx, rect: Rect, value: f32) {
    let ty = rect.y + (rect.h - TRACK_H) / 2.0;
    cx.cmds.push(DrawCmd::RRect {
        rect: Rect::new(rect.x, ty, rect.w, TRACK_H),
        radius: cx.theme.radius_full,
        color: cx.theme.secondary,
    });
    let fill_w = (rect.w * value).max(0.0);
    if fill_w > 0.0 {
        cx.cmds.push(DrawCmd::RRect {
            rect: Rect::new(rect.x, ty, fill_w, TRACK_H),
            radius: cx.theme.radius_full,
            color: cx.theme.accent,
        });
    }
    let kx = rect.x + (rect.w - KNOB_D) * value;
    cx.cmds.push(DrawCmd::RRect {
        rect: Rect::new(kx, rect.y + (rect.h - KNOB_D) / 2.0, KNOB_D, KNOB_D),
        radius: cx.theme.radius_full,
        color: cx.theme.accent_text,
    });
}
