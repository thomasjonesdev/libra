//! `<Chip>` — a pressable pill with optional dismiss (§7.1, PLAN-CATALOG
//! wave 5). `selected` is model-owned; `onPress` selects the chip;
//! `onRemove` (when present) is a trailing × with its own handler.
//! Tokens only: idle `surface_raised` / `text`, selected `accent` /
//! `accent_text`, hover idle `secondary`.

use crate::layout::BuildCx;
use crate::node::*;
use crate::paint::PaintCx;
use libra_draw::{DrawCmd, FontStyle};
use taffy::prelude::*;

#[allow(clippy::too_many_arguments)]
pub(crate) fn build(
    cx: &mut BuildCx,
    label: &str,
    selected: bool,
    on_press: &Option<String>,
    on_press_arg: &Option<String>,
    on_remove: &Option<String>,
    on_remove_arg: &Option<String>,
    test_id: &Option<String>,
) -> Result<NodeId, taffy::TaffyError> {
    let theme = cx.theme;
    let px = theme.type_sm;
    let (tw, th) = cx.m.measure(label, px, FontStyle::Regular);
    let remove_s = th;

    let label_id = cx.tree.new_leaf(Style {
        size: Size { width: length(tw.max(1.0)), height: length(th) },
        flex_shrink: 0.0,
        ..Default::default()
    })?;
    cx.meta.push((
        label_id,
        ItemKind::ChipLabel { label: label.to_string(), px, selected },
        Semantics {
            role: Role::Text,
            text: label.to_string(),
            test_id: None,
        },
        None,
    ));

    let mut kids = vec![label_id];
    if let Some(remove) = on_remove {
        let rid = cx.tree.new_leaf(Style {
            size: Size { width: length(remove_s), height: length(remove_s) },
            flex_shrink: 0.0,
            ..Default::default()
        })?;
        let remove_tid = test_id.as_ref().map(|t| format!("{t}-remove"));
        cx.meta.push((
            rid,
            ItemKind::ChipRemove { selected },
            Semantics {
                role: Role::Button,
                text: "remove".to_string(),
                test_id: remove_tid,
            },
            Some((remove.clone(), on_remove_arg.clone())),
        ));
        kids.push(rid);
    }

    let parent = cx.tree.new_with_children(
        Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            align_items: Some(AlignItems::Center),
            gap: Size { width: length(theme.space(2)), height: length(0.0) },
            padding: taffy::Rect {
                left: length(theme.space(3)),
                right: length(theme.space(3)),
                top: length(theme.space(1)),
                bottom: length(theme.space(1)),
            },
            flex_shrink: 0.0,
            ..Default::default()
        },
        &kids,
    )?;
    let press = on_press.as_ref().map(|p| (p.clone(), on_press_arg.clone()));
    cx.meta.push((
        parent,
        ItemKind::ChipBox { selected },
        Semantics {
            role: if press.is_some() { Role::Button } else { Role::Container },
            text: label.to_string(),
            test_id: test_id.clone(),
        },
        press,
    ));
    Ok(parent)
}

pub(crate) fn paint_box(cx: &mut PaintCx, item: &Item, selected: bool) {
    let theme = cx.theme;
    let hovered = cx.hover.is_some_and(|(hx, hy)| item.rect.contains(hx, hy));
    let bg = if selected {
        theme.accent
    } else if hovered {
        theme.secondary
    } else {
        theme.surface_raised
    };
    let radius = theme.radius_full.min(item.rect.h / 2.0);
    cx.cmds.push(DrawCmd::RRect { rect: item.rect, radius, color: bg });
}

pub(crate) fn paint_label(cx: &mut PaintCx, item: &Item, label: &str, px: f32, selected: bool) {
    let fg = if selected { cx.theme.accent_text } else { cx.theme.text };
    let (tw, th) = cx.measurer.measure(label, px, FontStyle::Regular);
    cx.cmds.push(DrawCmd::Text {
        x: item.rect.x + (item.rect.w - tw) / 2.0,
        y: item.rect.y + (item.rect.h - th) / 2.0,
        content: label.to_string(),
        px,
        style: FontStyle::Regular,
        color: fg,
    });
}

pub(crate) fn paint_remove(cx: &mut PaintCx, item: &Item, selected: bool) {
    let fg = if selected { cx.theme.accent_text } else { cx.theme.text_muted };
    let r = item.rect;
    let inset = (r.w.min(r.h) * 0.28).max(2.0);
    let x0 = r.x + inset;
    let y0 = r.y + inset;
    let x1 = r.x + r.w - inset;
    let y1 = r.y + r.h - inset;
    cx.cmds.push(DrawCmd::Polyline {
        points: vec![(x0, y0), (x1, y1)],
        width: 1.5,
        color: fg,
    });
    cx.cmds.push(DrawCmd::Polyline {
        points: vec![(x1, y0), (x0, y1)],
        width: 1.5,
        color: fg,
    });
}
