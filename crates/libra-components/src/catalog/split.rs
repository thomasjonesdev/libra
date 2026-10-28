//! Split — the two-pane draggable splitter (§7.1, PLAN-CATALOG /
//! PLAN-NOTES-APP M5), model-owned: `fraction` is a bound State f32; the
//! shell's divider drag (and the harness's `resize_split`) dispatch
//! `onResize` with the new fraction, and the app echoes it back into the
//! bound field. The reference SDK splits this into model-owned `Split` vs
//! engine-managed `Resizable`; Libra keeps only the model-owned shape —
//! pane layout is app state (replayable, rehydrated, snapshot-exact).
//!
//! Layout: a flex row `[left (width = fraction%), divider gutter, right
//! (grow)]`. The gutter is [`GUTTER`] px wide for grabbability and paints a
//! 1px `theme.hairline` flush with the left pane's trailing edge.

use crate::layout::{is_detached_node, BuildCx};
use crate::node::*;
use crate::paint::PaintCx;
use libra_draw::{DrawCmd, Rect};
use taffy::prelude::*;

/// Divider gutter width (logical px) — the pointer grab target.
pub const GUTTER: f32 = 5.0;
/// Floor so a pane cannot collapse under a narrow window.
pub const PANE_MIN: f32 = 160.0;

pub(crate) fn build(
    cx: &mut BuildCx,
    fraction: f32,
    left_min: Option<f32>,
    on_resize: &str,
    test_id: &str,
    children: &[ViewNode],
) -> Result<NodeId, taffy::TaffyError> {
    // Engines validate arity; guard here for hand-built trees.
    debug_assert_eq!(children.len(), 2, "<Split> takes exactly two children");
    let fraction = fraction.clamp(0.05, 0.95);
    let left_min = left_min.unwrap_or(PANE_MIN);

    let left = cx.child(&children[0])?;
    let right = cx.child(&children[1])?;
    for c in children {
        debug_assert!(!is_detached_node(c), "engines reject detached Split children");
    }

    // Size the panes at their slots: left takes `fraction` of the split's
    // width, right fills the remainder — overriding the children's own
    // grow/width so the fraction is authoritative.
    let mut ls = cx.tree.style(left)?.clone();
    ls.size.width = percent(fraction);
    ls.flex_grow = 0.0;
    ls.flex_shrink = 0.0;
    ls.min_size.width = length(left_min);
    cx.tree.set_style(left, ls)?;
    let mut rs = cx.tree.style(right)?.clone();
    rs.size.width = auto();
    rs.flex_grow = 1.0;
    rs.flex_shrink = 1.0;
    rs.min_size.width = length(PANE_MIN);
    cx.tree.set_style(right, rs)?;

    let divider = cx.tree.new_leaf(Style {
        size: Size { width: length(GUTTER), height: auto() },
        flex_shrink: 0.0,
        ..Default::default()
    })?;
    cx.meta.push((
        divider,
        ItemKind::SplitDivider { split: test_id.to_string() },
        Semantics { role: Role::Separator, text: String::new(), test_id: None },
        None,
    ));

    let container = cx.tree.new_with_children(
        Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            flex_grow: 1.0,
            min_size: Size { width: auto(), height: length(0.0) },
            ..Default::default()
        },
        &[left, divider, right],
    )?;
    cx.meta.push((
        container,
        ItemKind::Split { on_resize: on_resize.to_string() },
        Semantics { role: Role::Container, text: String::new(), test_id: Some(test_id.to_string()) },
        None,
    ));
    Ok(container)
}

pub(crate) fn paint_divider(cx: &mut PaintCx, rect: Rect) {
    // 1px hairline on the left gutter edge — the left pane's trailing
    // boundary. Floor to a whole-pixel x (tiny-skia AA hairline gotcha).
    let x = rect.x.floor();
    cx.cmds.push(DrawCmd::Rect { rect: Rect::new(x, rect.y, 1.0, rect.h), color: cx.theme.hairline });
}
