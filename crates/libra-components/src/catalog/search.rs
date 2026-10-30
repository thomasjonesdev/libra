//! `<Search>` — Input chrome + submit (§7.1, PLAN-CATALOG wave 3).
//!
//! Controlled like Input: the model owns `value`, `test_id` is the
//! focus identity, typing dispatches `onChange`. Optional `onSubmit`
//! is a unit action the shell/harness fire on Enter. The leading
//! magnifying-glass is a token-colored polyline (no Icon/`DrawCmd`
//! arc yet).

use crate::catalog::input;
use crate::layout::BuildCx;
use crate::node::*;
use crate::paint::PaintCx;
use libra_draw::{DrawCmd, FontStyle, Theme};
use taffy::prelude::*;

/// Lens diameter in logical px — density-scaled via [`Theme::dense`].
const ICON: f32 = 12.0;

pub(crate) fn leading(theme: &Theme) -> f32 {
    theme.dense(ICON) + theme.space(2)
}

pub(crate) fn build(
    cx: &mut BuildCx,
    value: &str,
    placeholder: &str,
    on_change: &str,
    on_submit: &Option<String>,
    keyboard: Keyboard,
    test_id: &str,
) -> Result<NodeId, taffy::TaffyError> {
    let theme = cx.theme;
    let px = theme.type_md;
    let (_, th) = cx.m.measure("Ag", px, FontStyle::Regular);
    let h = th + 2.0 * theme.btn_pad_y;
    let id = cx.tree.new_leaf(Style {
        size: Size { width: auto(), height: length(h) },
        flex_grow: 1.0,
        flex_shrink: 1.0,
        ..Default::default()
    })?;
    cx.meta.push((
        id,
        ItemKind::Search {
            value: value.to_string(),
            placeholder: placeholder.to_string(),
            px,
            on_submit: on_submit.clone(),
            keyboard,
        },
        Semantics {
            role: Role::TextInput,
            text: value.to_string(),
            test_id: Some(test_id.to_string()),
        },
        // Same focus contract as Input: click focuses; onChange rides
        // in the press slot as the handler name.
        Some((on_change.to_string(), None)),
    ));
    Ok(id)
}

pub(crate) fn paint(
    cx: &mut PaintCx,
    item: &Item,
    value: &str,
    placeholder: &str,
    px: f32,
) {
    let lead = leading(cx.theme);
    input::paint_field(cx, item, value, placeholder, px, lead);
    paint_glyph(cx.cmds, item.rect, cx.theme);
}

/// Magnifying glass: a 12-segment polyline circle + a short handle.
/// Tokens only (`theme.text_muted`); no raw color.
fn paint_glyph(cmds: &mut Vec<DrawCmd>, rect: libra_draw::Rect, theme: &Theme) {
    let icon = theme.dense(ICON);
    let r = icon * 0.32;
    let cx = rect.x + theme.btn_pad_x + icon * 0.42;
    let cy = rect.y + rect.h * 0.5;
    let mut lens = Vec::with_capacity(13);
    for i in 0..=12 {
        let a = (i as f32 / 12.0) * std::f32::consts::TAU;
        lens.push((cx + r * a.cos(), cy + r * a.sin()));
    }
    cmds.push(DrawCmd::Polyline {
        points: lens,
        width: 1.5,
        color: theme.text_muted,
    });
    let hx = cx + r * 0.72;
    let hy = cy + r * 0.72;
    cmds.push(DrawCmd::Polyline {
        points: vec![(hx, hy), (hx + r * 0.95, hy + r * 0.95)],
        width: 1.5,
        color: theme.text_muted,
    });
}
