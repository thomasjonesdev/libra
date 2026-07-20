//! Drawer — edge-anchored modal (§7.1, PLAN-CATALOG): the Dialog
//! overlay machinery (scrim + detached overlay root + modal barrier)
//! with the panel pinned to a side. `side="bottom"` is the reference
//! SDK's Sheet, folded in per the catalog audit.

use crate::catalog;
use crate::layout::{is_detached_node, BuildCx};
use crate::node::*;
use taffy::prelude::*;

pub(crate) fn build(
    cx: &mut BuildCx,
    side: DrawerSide,
    on_dismiss: &Option<String>,
    test_id: &Option<String>,
    children: &[ViewNode],
) -> Result<NodeId, taffy::TaffyError> {
    let mut kids = Vec::new();
    for c in children {
        if is_detached_node(c) {
            let _ = cx.child(c)?;
        } else {
            kids.push(cx.child(c)?);
        }
    }

    let pad = cx.theme.space(6);
    let panel_size = match side {
        // Side drawers: fixed width, full height.
        DrawerSide::Left | DrawerSide::Right => Size {
            width: length(320.0),
            height: percent(1.0),
        },
        // Bottom sheet: full width, natural height.
        DrawerSide::Bottom => Size { width: percent(1.0), height: auto() },
    };
    let panel = cx.tree.new_with_children(
        Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            gap: Size { width: length(cx.theme.space(4)), height: length(cx.theme.space(4)) },
            padding: taffy::Rect {
                left: length(pad),
                right: length(pad),
                top: length(pad),
                bottom: length(pad),
            },
            size: panel_size,
            max_size: Size { width: percent(0.9), height: percent(1.0) },
            flex_shrink: 0.0,
            ..Default::default()
        },
        &kids,
    )?;
    cx.meta.push((
        panel,
        ItemKind::DialogPanel,
        Semantics { role: Role::Dialog, text: String::new(), test_id: test_id.clone() },
        None,
    ));

    let (dir, justify, align) = match side {
        DrawerSide::Left => (FlexDirection::Row, JustifyContent::Start, AlignItems::Stretch),
        DrawerSide::Right => (FlexDirection::Row, JustifyContent::End, AlignItems::Stretch),
        DrawerSide::Bottom => (FlexDirection::Column, JustifyContent::End, AlignItems::Stretch),
    };
    let scrim = cx.tree.new_with_children(
        Style {
            display: Display::Flex,
            flex_direction: dir,
            justify_content: Some(justify),
            align_items: Some(align),
            ..Default::default()
        },
        &[panel],
    )?;
    cx.meta.push((
        scrim,
        ItemKind::Scrim,
        Semantics { role: Role::Container, text: String::new(), test_id: None },
        on_dismiss.as_ref().map(|d| (d.clone(), None)),
    ));
    cx.overlays.push(scrim);
    let _ = catalog::dialog::paint_panel; // shared panel chrome (paint arm)
    Ok(scrim)
}
