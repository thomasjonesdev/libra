//! Heading — a semantic heading (§7.1, PLAN-CATALOG wave 2). `level` 1–4 maps
//! to the type ramp (xl/lg/md/sm), always bold. Builds to a bold `Text` item
//! (it has no ItemKind of its own), so it reuses the text paint path.

use crate::layout::BuildCx;
use crate::node::*;
use libra_draw::{FontStyle, Theme};
use taffy::prelude::*;

/// Level 1–4 → type ramp. Clamped: level 0 reads as 1, ≥4 as 4.
fn level_px(theme: &Theme, level: u8) -> f32 {
    match level {
        0 | 1 => theme.type_xl,
        2 => theme.type_lg,
        3 => theme.type_md,
        _ => theme.type_sm,
    }
}

pub(crate) fn build(
    cx: &mut BuildCx,
    content: &str,
    level: u8,
    test_id: &Option<String>,
) -> Result<NodeId, taffy::TaffyError> {
    let theme = cx.theme;
    let px = level_px(theme, level);
    let (w, h) = cx.m.measure(content, px, FontStyle::Bold);
    let id = cx.tree.new_leaf(Style {
        size: Size { width: length(w), height: length(h) },
        flex_shrink: 0.0,
        ..Default::default()
    })?;
    cx.meta.push((
        id,
        ItemKind::Text { content: content.to_string(), px, style: FontStyle::Bold, color: theme.text, wrap: false },
        Semantics { role: Role::Text, text: content.to_string(), test_id: test_id.clone() },
        None,
    ));
    Ok(id)
}
