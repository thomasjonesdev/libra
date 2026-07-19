//! Spacer — pure layout (§7.1, PLAN-CATALOG wave 1). Fixed room on the parent's
//! main axis (`size: Some(px)`) or a flex spring (`None`). Paints nothing.

use crate::layout::BuildCx;
use crate::node::*;
use taffy::prelude::*;

pub(crate) fn build(cx: &mut BuildCx, size: Option<f32>) -> Result<NodeId, taffy::TaffyError> {
    // Pure layout: fixed room along the parent's main axis (flex-basis is
    // main-axis-relative, so one node works in both Column and Row), or a
    // flex spring when sizeless.
    let style = match size {
        Some(s) => Style { flex_basis: length(s), flex_shrink: 0.0, ..Default::default() },
        None => Style { flex_grow: 1.0, ..Default::default() },
    };
    let id = cx.tree.new_leaf(style)?;
    cx.meta.push((
        id,
        ItemKind::Box,
        Semantics { role: Role::Container, text: String::new(), test_id: None },
        None,
    ));
    Ok(id)
}
