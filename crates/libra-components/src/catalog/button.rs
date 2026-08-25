//! Button — a pressable label with token-driven variants (§7.1). `on_press` is
//! the handler name; `on_press_arg` carries an item-scoped `String` payload
//! (`onPress={toggle(item)}`, §6.2). Optional `icon` paints a closed
//! [`IconName`] glyph (Gloss rail: icon-only ghost buttons).

use crate::catalog::icon::{self, ICON};
use crate::layout::BuildCx;
use crate::node::*;
use crate::paint::PaintCx;
use libra_draw::{Color, DrawCmd, FontStyle, Rect};
use taffy::prelude::*;

pub(crate) fn build(
    cx: &mut BuildCx,
    label: &str,
    variant: Variant,
    icon: Option<IconName>,
    on_press: &str,
    on_press_arg: &Option<String>,
    test_id: &Option<String>,
) -> Result<NodeId, taffy::TaffyError> {
    let theme = cx.theme;
    let px = theme.type_md;
    let (tw, th) = if label.is_empty() {
        (0.0, 0.0)
    } else {
        cx.m.measure(label, px, FontStyle::Regular)
    };
    let icon_s = if icon.is_some() { theme.dense(ICON) } else { 0.0 };
    let gap = if icon.is_some() && !label.is_empty() { theme.space(2) } else { 0.0 };
    let inner_w = icon_s + gap + tw;
    let inner_h = th.max(icon_s);
    let (w, h) = (inner_w + 2.0 * theme.btn_pad_x, inner_h + 2.0 * theme.btn_pad_y);
    let id = cx.tree.new_leaf(Style {
        size: Size { width: length(w), height: length(h) },
        flex_shrink: 0.0,
        ..Default::default()
    })?;
    let text = if label.is_empty() {
        icon.map(IconName::as_str).unwrap_or("").to_string()
    } else {
        label.to_string()
    };
    cx.meta.push((
        id,
        ItemKind::Button { label: label.to_string(), px, variant, icon },
        Semantics { role: Role::Button, text, test_id: test_id.clone() },
        Some((on_press.to_string(), on_press_arg.clone())),
    ));
    Ok(id)
}

pub(crate) fn paint(
    cx: &mut PaintCx,
    item: &Item,
    label: &str,
    px: f32,
    variant: Variant,
    icon: Option<IconName>,
) {
    let theme = cx.theme;
    let (fill, text_color) = match variant {
        Variant::Primary => (theme.accent, theme.accent_text),
        Variant::Secondary => (theme.secondary, theme.text),
        Variant::Ghost => (Color::TRANSPARENT, theme.text_muted),
    };
    if fill != Color::TRANSPARENT {
        cx.cmds.push(DrawCmd::RRect { rect: item.rect, radius: theme.radius_md, color: fill });
    }
    let (tw, th) = if label.is_empty() {
        (0.0, 0.0)
    } else {
        cx.measurer.measure(label, px, FontStyle::Regular)
    };
    let icon_s = if icon.is_some() { theme.dense(ICON) } else { 0.0 };
    let gap = if icon.is_some() && !label.is_empty() { theme.space(2) } else { 0.0 };
    let inner_w = icon_s + gap + tw;
    let mut x = item.rect.x + (item.rect.w - inner_w) / 2.0;
    if let Some(name) = icon {
        let ir = Rect::new(x, item.rect.y + (item.rect.h - icon_s) / 2.0, icon_s, icon_s);
        icon::paint_name(cx.cmds, ir, theme, name, text_color);
        x += icon_s + gap;
    }
    if !label.is_empty() {
        cx.cmds.push(DrawCmd::Text {
            x,
            y: item.rect.y + (item.rect.h - th) / 2.0,
            content: label.to_string(),
            px,
            style: FontStyle::Regular,
            color: text_color,
        });
    }
}
