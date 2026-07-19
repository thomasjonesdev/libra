//! ContextMenu — the anchored floating menu (§7.1, PLAN-CATALOG /
//! PLAN-NOTES-APP M7): the Dialog overlay machinery with an invisible
//! full-viewport barrier and a small `surface_raised` panel that
//! `LaidTree::apply_menu_anchor` moves to the **menu anchor** — a
//! deterministic input like focus/hover/scroll (shell: the right-click
//! point; harness: `right_click`'s item center; default (0,0)).
//! Menu items are ordinary markup children (ghost `<Button>`s); the
//! barrier dismisses via `onDismiss` or swallows the click.

use crate::catalog;
use crate::layout::BuildCx;
use crate::node::*;
use crate::paint::PaintCx;
use libra_draw::Rect;
use taffy::prelude::*;

pub(crate) fn build(
    cx: &mut BuildCx,
    on_dismiss: &Option<String>,
    test_id: &str,
    children: &[ViewNode],
) -> Result<NodeId, taffy::TaffyError> {
    let mut kids = Vec::new();
    for c in children {
        if crate::layout::is_detached_node(c) {
            let _ = cx.child(c)?;
        } else {
            kids.push(cx.child(c)?);
        }
    }

    let pad = cx.theme.space(2);
    let panel = cx.tree.new_with_children(
        Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            gap: Size { width: length(cx.theme.space(1)), height: length(cx.theme.space(1)) },
            padding: taffy::Rect {
                left: length(pad),
                right: length(pad),
                top: length(pad),
                bottom: length(pad),
            },
            min_size: Size { width: length(160.0), height: auto() },
            ..Default::default()
        },
        &kids,
    )?;
    cx.meta.push((
        panel,
        ItemKind::MenuPanel,
        Semantics { role: Role::Menu, text: String::new(), test_id: Some(test_id.to_string()) },
        None,
    ));

    // Barrier: the overlay root. Panel lays out at the top-left; the
    // anchor pass moves it. Align start so the panel keeps natural size.
    let barrier = cx.tree.new_with_children(
        Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            align_items: Some(AlignItems::Start),
            ..Default::default()
        },
        &[panel],
    )?;
    cx.meta.push((
        barrier,
        ItemKind::MenuBarrier,
        Semantics { role: Role::Container, text: String::new(), test_id: None },
        on_dismiss.as_ref().map(|d| (d.clone(), None)),
    ));
    cx.overlays.push(barrier);
    Ok(barrier)
}

pub(crate) fn paint_panel(cx: &mut PaintCx, rect: Rect) {
    // Same card chrome as the Dialog panel — one visual language.
    catalog::dialog::paint_panel(cx, rect);
}
