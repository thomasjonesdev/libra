//! Breadcrumb — a hierarchy trail (§7.1, PLAN-CATALOG): `BreadcrumbItem`
//! children render with an auto-inserted "›" chevron between them; the
//! trailing `current` crumb is bold and non-pressable.

use crate::layout::{box_style, is_detached_node, BuildCx};
use crate::node::*;
use crate::paint::PaintCx;
use libra_draw::{DrawCmd, FontStyle};
use taffy::prelude::*;

pub(crate) fn build_breadcrumb(
    cx: &mut BuildCx,
    children: &[ViewNode],
) -> Result<NodeId, taffy::TaffyError> {
    let mut kids = Vec::new();
    let mut first = true;
    for c in children {
        if is_detached_node(c) {
            let _ = cx.child(c)?;
            continue;
        }
        if !first {
            kids.push(build_chevron(cx)?);
        }
        kids.push(cx.child(c)?);
        first = false;
    }
    let style = BoxStyle { gap: cx.theme.space(2), cross_center: true, ..Default::default() };
    let id = cx.tree.new_with_children(box_style(&style, FlexDirection::Row), &kids)?;
    cx.meta.push((
        id,
        ItemKind::BreadcrumbBox,
        Semantics { role: Role::Container, text: String::new(), test_id: None },
        None,
    ));
    Ok(id)
}

fn build_chevron(cx: &mut BuildCx) -> Result<NodeId, taffy::TaffyError> {
    let px = cx.theme.type_sm;
    let (w, h) = cx.m.measure("›", px, FontStyle::Regular);
    let id = cx.tree.new_leaf(Style {
        size: Size { width: length(w), height: length(h) },
        flex_shrink: 0.0,
        ..Default::default()
    })?;
    cx.meta.push((
        id,
        ItemKind::Chevron,
        Semantics { role: Role::Container, text: String::new(), test_id: None },
        None,
    ));
    Ok(id)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_item(
    cx: &mut BuildCx,
    label: &str,
    current: bool,
    on_press: &Option<String>,
    on_press_arg: &Option<String>,
    test_id: &Option<String>,
) -> Result<NodeId, taffy::TaffyError> {
    let px = cx.theme.type_sm;
    let style = if current { FontStyle::Bold } else { FontStyle::Regular };
    let (w, h) = cx.m.measure(label, px, style);
    let id = cx.tree.new_leaf(Style {
        size: Size { width: length(w), height: length(h) },
        flex_shrink: 0.0,
        ..Default::default()
    })?;
    let press = if current { None } else { on_press.as_ref().map(|n| (n.clone(), on_press_arg.clone())) };
    cx.meta.push((
        id,
        ItemKind::BreadcrumbLabel { label: label.to_string(), current },
        Semantics {
            role: if current { Role::Text } else { Role::Button },
            text: label.to_string(),
            test_id: test_id.clone(),
        },
        press,
    ));
    Ok(id)
}

pub(crate) fn paint_chevron(cx: &mut PaintCx, item: &Item) {
    cx.cmds.push(DrawCmd::Text {
        x: item.rect.x,
        y: item.rect.y,
        content: "›".to_string(),
        px: cx.theme.type_sm,
        style: FontStyle::Regular,
        color: cx.theme.text_muted,
    });
}

pub(crate) fn paint_label(cx: &mut PaintCx, item: &Item, label: &str, current: bool) {
    let (color, style) =
        if current { (cx.theme.text, FontStyle::Bold) } else { (cx.theme.accent, FontStyle::Regular) };
    cx.cmds.push(DrawCmd::Text {
        x: item.rect.x,
        y: item.rect.y,
        content: label.to_string(),
        px: cx.theme.type_sm,
        style,
        color,
    });
}
