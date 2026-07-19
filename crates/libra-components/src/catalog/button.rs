//! Button — a pressable label with token-driven variants (§7.1). `on_press` is
//! the handler name; `on_press_arg` carries an item-scoped `String` payload
//! (`onPress={toggle(item)}`, §6.2).

use crate::layout::BuildCx;
use crate::node::*;
use crate::paint::PaintCx;
use libra_draw::{Color, DrawCmd, FontStyle};
use taffy::prelude::*;

pub(crate) fn build(
    cx: &mut BuildCx,
    label: &str,
    variant: Variant,
    on_press: &str,
    on_press_arg: &Option<String>,
    test_id: &Option<String>,
) -> Result<NodeId, taffy::TaffyError> {
    let theme = cx.theme;
    let px = theme.type_md;
    let (tw, th) = cx.m.measure(label, px, FontStyle::Regular);
    let (w, h) = (tw + 2.0 * theme.btn_pad_x, th + 2.0 * theme.btn_pad_y);
    let id = cx.tree.new_leaf(Style {
        size: Size { width: length(w), height: length(h) },
        flex_shrink: 0.0,
        ..Default::default()
    })?;
    cx.meta.push((
        id,
        ItemKind::Button { label: label.to_string(), px, variant },
        Semantics { role: Role::Button, text: label.to_string(), test_id: test_id.clone() },
        Some((on_press.to_string(), on_press_arg.clone())),
    ));
    Ok(id)
}

pub(crate) fn paint(cx: &mut PaintCx, item: &Item, label: &str, px: f32, variant: Variant) {
    let theme = cx.theme;
    let (fill, text_color) = match variant {
        Variant::Primary => (theme.accent, theme.accent_text),
        Variant::Secondary => (theme.secondary, theme.text),
        Variant::Ghost => (Color::TRANSPARENT, theme.text_muted),
    };
    if fill != Color::TRANSPARENT {
        cx.cmds.push(DrawCmd::RRect { rect: item.rect, radius: theme.radius_md, color: fill });
    }
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
