//! Scroll — a vertical scroll viewport (§7.1, PLAN-SCROLL). The viewport is a
//! fixed- or grown-height node; its content lays out at natural height inside
//! an inner column that never shrinks, so it overflows. `flatten` tags the
//! content with this viewport's clip + test_id; `paint` clips it and
//! `LaidTree::apply_scroll` offsets it by the scroll position.

use crate::layout::{is_detached_node, BuildCx};
use crate::node::*;
use taffy::prelude::*;

pub(crate) fn build(
    cx: &mut BuildCx,
    height: Option<f32>,
    test_id: &str,
    children: &[ViewNode],
) -> Result<NodeId, taffy::TaffyError> {
    // Inner content column: natural height, `flex_shrink: 0` so a tall content
    // overflows the viewport instead of being squeezed to fit.
    let mut kids = Vec::new();
    for c in children {
        if is_detached_node(c) {
            let _ = cx.child(c)?;
        } else {
            kids.push(cx.child(c)?);
        }
    }
    let content = cx.tree.new_with_children(
        Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            flex_shrink: 0.0,
            ..Default::default()
        },
        &kids,
    )?;

    // Viewport: fixed `height`, or grow to fill the parent. `overflow.y:
    // Hidden` is the key — it stops the tall content from inflating the
    // viewport's size, so the content overflows (and we clip it) instead of the
    // viewport growing to fit. `min_size.height: 0` lets it shrink to available.
    let viewport = Style {
        display: Display::Flex,
        flex_direction: FlexDirection::Column,
        size: Size { width: auto(), height: height.map(length).unwrap_or_else(auto) },
        min_size: Size { width: auto(), height: length(0.0) },
        flex_grow: if height.is_some() { 0.0 } else { 1.0 },
        flex_shrink: 1.0,
        overflow: taffy::geometry::Point {
            x: taffy::style::Overflow::Visible,
            y: taffy::style::Overflow::Hidden,
        },
        ..Default::default()
    };
    let id = cx.tree.new_with_children(viewport, &[content])?;
    cx.meta.push((
        id,
        ItemKind::Scroll,
        Semantics { role: Role::Container, text: String::new(), test_id: Some(test_id.to_string()) },
        None,
    ));
    Ok(id)
}
