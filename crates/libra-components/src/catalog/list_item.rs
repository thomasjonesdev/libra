//! ListItem — a pressable list row (§7.1, PLAN-CATALOG): title line, optional
//! muted subtitle/meta lines, and distinct token washes — `selection` when
//! selected, `surface_raised` on hover (PLAN-NOTES-APP M8). The whole row is
//! the press target; `on_press_arg` carries the item-scoped payload exactly
//! like Button (§6.2).

use crate::layout::BuildCx;
use crate::node::*;
use crate::paint::PaintCx;
use libra_draw::{DrawCmd, FontStyle};
use taffy::prelude::*;

#[allow(clippy::too_many_arguments)]
pub(crate) fn build(
    cx: &mut BuildCx,
    title: &str,
    subtitle: &str,
    meta_line: &str,
    selected: bool,
    on_press: &str,
    on_press_arg: &Option<String>,
    on_menu: &Option<String>,
    on_menu_arg: &Option<String>,
    test_id: &Option<String>,
) -> Result<NodeId, taffy::TaffyError> {
    let theme = cx.theme;
    let px_md = theme.type_md;
    let px_sm = theme.type_sm;
    let (_, th_md) = cx.m.measure("Ag", px_md, FontStyle::Regular);
    let (_, th_sm) = cx.m.measure("Ag", px_sm, FontStyle::Regular);
    let lines = 1 + usize::from(!subtitle.is_empty()) + usize::from(!meta_line.is_empty());
    let h = 2.0 * theme.btn_pad_y
        + th_md
        + if lines > 1 { (lines - 1) as f32 * (th_sm + 2.0) } else { 0.0 };
    let id = cx.tree.new_leaf(Style {
        size: Size { width: auto(), height: length(h) },
        flex_shrink: 0.0,
        ..Default::default()
    })?;
    cx.meta.push((
        id,
        ItemKind::ListItem {
            title: title.to_string(),
            subtitle: subtitle.to_string(),
            meta: meta_line.to_string(),
            selected,
            px_md,
            px_sm,
            on_menu: on_menu.as_ref().map(|m| (m.clone(), on_menu_arg.clone())),
        },
        Semantics { role: Role::Button, text: title.to_string(), test_id: test_id.clone() },
        Some((on_press.to_string(), on_press_arg.clone())),
    ));
    Ok(id)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn paint(
    cx: &mut PaintCx,
    item: &Item,
    title: &str,
    subtitle: &str,
    meta: &str,
    selected: bool,
    px_md: f32,
    px_sm: f32,
) {
    let theme = cx.theme;
    let hovered = cx.hover.is_some_and(|(hx, hy)| item.rect.contains(hx, hy));
    let wash = if selected {
        Some(theme.selection)
    } else if hovered {
        Some(theme.surface_raised)
    } else {
        None
    };
    if let Some(color) = wash {
        cx.cmds.push(DrawCmd::RRect {
            rect: item.rect,
            radius: theme.radius_md,
            color,
        });
    }
    let (tx, mut ty) = (item.rect.x + theme.btn_pad_x, item.rect.y + theme.btn_pad_y);
    let (_, th_md) = cx.measurer.measure("Ag", px_md, FontStyle::Regular);
    let (_, th_sm) = cx.measurer.measure("Ag", px_sm, FontStyle::Regular);
    cx.cmds.push(DrawCmd::Text {
        x: tx,
        y: ty,
        content: title.to_string(),
        px: px_md,
        style: FontStyle::Regular,
        color: theme.text,
    });
    ty += th_md + 2.0;
    for line in [subtitle, meta] {
        if line.is_empty() {
            continue;
        }
        cx.cmds.push(DrawCmd::Text {
            x: tx,
            y: ty,
            content: line.to_string(),
            px: px_sm,
            style: FontStyle::Regular,
            color: theme.text_muted,
        });
        ty += th_sm + 2.0;
    }
}
