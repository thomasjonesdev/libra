//! Dialog — the in-tree modal (§7.1, PLAN-CATALOG / PLAN-NOTES-APP M6): a
//! full-viewport scrim + centered panel, deterministic and snapshot-able
//! (OS dialogs are §12.1 effects; this one is inside the paint contract).
//!
//! The dialog subtree builds into the shared taffy tree but is **detached**
//! from its markup parent (the source-element precedent): `layout` collects
//! it as an overlay root, computes it at the full viewport after the main
//! pass, and appends its items — so it paints on top and its scrim is the
//! modal barrier in `hit()`, wherever the `<Dialog>` sat in the markup.
//! Mount/unmount is the app's `<if test={…}>` — the component holds no
//! open/close state.

use crate::catalog;
use crate::layout::BuildCx;
use crate::node::*;
use crate::paint::PaintCx;
use libra_draw::{DrawCmd, Rect};
use taffy::prelude::*;

pub(crate) fn build(
    cx: &mut BuildCx,
    title: &str,
    on_dismiss: &Option<String>,
    test_id: &Option<String>,
    children: &[ViewNode],
) -> Result<NodeId, taffy::TaffyError> {
    // Panel content: bold title line, then the markup children.
    let mut kids = vec![catalog::text::build(cx, title, TextSize::Md, true, false, false, &None)?];
    for c in children {
        if crate::layout::is_detached_node(c) {
            let _ = cx.child(c)?;
        } else {
            kids.push(cx.child(c)?);
        }
    }

    let pad = cx.theme.space(6);
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
            size: Size { width: length(400.0), height: auto() },
            max_size: Size { width: percent(0.9), height: percent(0.9) },
            flex_shrink: 1.0,
            ..Default::default()
        },
        &kids,
    )?;
    cx.meta.push((
        panel,
        ItemKind::DialogPanel,
        Semantics { role: Role::Dialog, text: title.to_string(), test_id: test_id.clone() },
        None,
    ));

    // Scrim: the overlay root — sized to the viewport by `layout`'s overlay
    // pass, centering the panel. With `onDismiss` it's pressable (click
    // outside the panel dismisses); without, `hit()` swallows the click.
    let scrim = cx.tree.new_with_children(
        Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            align_items: Some(AlignItems::Center),
            justify_content: Some(JustifyContent::Center),
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
    Ok(scrim)
}

pub(crate) fn paint_scrim(cx: &mut PaintCx, rect: Rect) {
    cx.cmds.push(DrawCmd::Rect { rect, color: cx.theme.scrim });
}

pub(crate) fn paint_panel(cx: &mut PaintCx, rect: Rect) {
    cx.cmds.push(DrawCmd::RRect { rect, radius: cx.theme.radius_lg, color: cx.theme.surface_raised });
    // Hairline frame: four 1px edges (no stroked rrect in the display list).
    let c = cx.theme.hairline;
    cx.cmds.push(DrawCmd::Rect { rect: Rect::new(rect.x, rect.y, rect.w, 1.0), color: c });
    cx.cmds.push(DrawCmd::Rect { rect: Rect::new(rect.x, rect.y + rect.h - 1.0, rect.w, 1.0), color: c });
    cx.cmds.push(DrawCmd::Rect { rect: Rect::new(rect.x, rect.y, 1.0, rect.h), color: c });
    cx.cmds.push(DrawCmd::Rect { rect: Rect::new(rect.x + rect.w - 1.0, rect.y, 1.0, rect.h), color: c });
}
