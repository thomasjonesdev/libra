//! Column / Row — flex containers (§8). Build only: layout is their whole job,
//! they paint nothing (their items are `ItemKind::Box`).

use crate::layout::{box_style, is_source_node, BuildCx};
use crate::node::*;
use taffy::prelude::*;

pub(crate) fn build_column(
    cx: &mut BuildCx,
    style: &BoxStyle,
    children: &[ViewNode],
) -> Result<NodeId, taffy::TaffyError> {
    let kids = build_children(cx, children)?;
    let id = cx.tree.new_with_children(box_style(style, FlexDirection::Column), &kids)?;
    push_box(cx, id);
    Ok(id)
}

pub(crate) fn build_row(
    cx: &mut BuildCx,
    style: &BoxStyle,
    children: &[ViewNode],
) -> Result<NodeId, taffy::TaffyError> {
    let kids = build_children(cx, children)?;
    let id = cx.tree.new_with_children(box_style(style, FlexDirection::Row), &kids)?;
    push_box(cx, id);
    Ok(id)
}

fn build_children(cx: &mut BuildCx, children: &[ViewNode]) -> Result<Vec<NodeId>, taffy::TaffyError> {
    let mut kids = Vec::new();
    for c in children {
        // §12.5: source elements occupy no layout — register them in meta
        // but keep them out of the flex tree so gap/grow of siblings stay
        // snapshot-stable.
        if is_source_node(c) {
            let _ = cx.child(c)?;
        } else {
            kids.push(cx.child(c)?);
        }
    }
    Ok(kids)
}

fn push_box(cx: &mut BuildCx, id: NodeId) {
    cx.meta.push((
        id,
        ItemKind::Box,
        Semantics { role: Role::Container, text: String::new(), test_id: None },
        None,
    ));
}
