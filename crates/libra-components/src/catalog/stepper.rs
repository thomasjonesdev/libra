//! Stepper — progress stages (§7.1, PLAN-CATALOG): each `StepperItem`
//! child's completed/active/pending status is derived by the container
//! from its position relative to `current` (0-based) — never an attr
//! on the item itself. A connector line is auto-inserted between steps.

use crate::layout::BuildCx;
use crate::node::*;
use crate::paint::PaintCx;
use libra_draw::{DrawCmd, FontStyle, Rect};
use taffy::prelude::*;

const DOT_D: f32 = 22.0;

pub(crate) fn build(
    cx: &mut BuildCx,
    current: u32,
    children: &[ViewNode],
) -> Result<NodeId, taffy::TaffyError> {
    let mut kids = Vec::new();
    let mut idx: u32 = 0;
    for c in children {
        let (label, test_id) = match c {
            ViewNode::StepperItem { label, test_id } => (label.clone(), test_id.clone()),
            other if crate::layout::is_detached_node(other) => {
                let _ = cx.child(other)?;
                continue;
            }
            _ => continue, // non-StepperItem content isn't a stage; skip defensively
        };
        if idx > 0 {
            kids.push(build_connector(cx, idx <= current)?);
        }
        let status = if idx < current {
            StepStatus::Completed
        } else if idx == current {
            StepStatus::Active
        } else {
            StepStatus::Pending
        };
        kids.push(build_item(cx, &label, status, &test_id)?);
        idx += 1;
    }
    let id = cx.tree.new_with_children(
        Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            align_items: Some(AlignItems::Center),
            flex_grow: 1.0,
            ..Default::default()
        },
        &kids,
    )?;
    cx.meta.push((
        id,
        ItemKind::StepperBox,
        Semantics { role: Role::Container, text: String::new(), test_id: None },
        None,
    ));
    Ok(id)
}

fn build_item(
    cx: &mut BuildCx,
    label: &str,
    status: StepStatus,
    test_id: &Option<String>,
) -> Result<NodeId, taffy::TaffyError> {
    let px = cx.theme.type_sm;
    let (tw, th) = cx.m.measure(label, px, FontStyle::Regular);
    let w = DOT_D.max(tw) + cx.theme.space(2);
    let h = DOT_D + cx.theme.space(1) + th;
    let id = cx.tree.new_leaf(Style {
        size: Size { width: length(w), height: length(h) },
        flex_shrink: 0.0,
        ..Default::default()
    })?;
    cx.meta.push((
        id,
        ItemKind::StepperDot { label: label.to_string(), status, px },
        Semantics {
            role: Role::Text,
            text: format!("{label} ({})", status_text(status)),
            test_id: test_id.clone(),
        },
        None,
    ));
    Ok(id)
}

fn build_connector(cx: &mut BuildCx, filled: bool) -> Result<NodeId, taffy::TaffyError> {
    let id = cx.tree.new_leaf(Style {
        size: Size { width: auto(), height: length(2.0) },
        flex_grow: 1.0,
        flex_shrink: 1.0,
        ..Default::default()
    })?;
    cx.meta.push((
        id,
        ItemKind::Connector { vertical: false, filled },
        Semantics { role: Role::Container, text: String::new(), test_id: None },
        None,
    ));
    Ok(id)
}

fn status_text(s: StepStatus) -> &'static str {
    match s {
        StepStatus::Completed => "completed",
        StepStatus::Active => "active",
        StepStatus::Pending => "pending",
    }
}

pub(crate) fn paint_dot(cx: &mut PaintCx, item: &Item, label: &str, status: StepStatus, px: f32) {
    let dot = Rect::new(item.rect.x + (item.rect.w - DOT_D) / 2.0, item.rect.y, DOT_D, DOT_D);
    let (fill, text_color) = match status {
        StepStatus::Completed => (cx.theme.accent, cx.theme.accent_text),
        StepStatus::Active => (cx.theme.accent, cx.theme.accent_text),
        StepStatus::Pending => (cx.theme.secondary, cx.theme.text_muted),
    };
    cx.cmds.push(DrawCmd::RRect { rect: dot, radius: cx.theme.radius_full, color: fill });
    if matches!(status, StepStatus::Completed) {
        // A Polyline check glyph, not a Unicode "✓" Text: the bundled
        // Work Sans (fontdue, advance-only) doesn't cover it —
        // renders as a tofu box. Checkbox already draws its check this
        // way for the same reason; match it here.
        cx.cmds.push(DrawCmd::Polyline {
            points: vec![
                (dot.x + 0.25 * dot.w, dot.y + 0.55 * dot.h),
                (dot.x + 0.45 * dot.w, dot.y + 0.72 * dot.h),
                (dot.x + 0.78 * dot.w, dot.y + 0.30 * dot.h),
            ],
            width: 1.8,
            color: text_color,
        });
    }
    let (lw, _) = cx.measurer.measure(label, px, FontStyle::Regular);
    let label_color = if matches!(status, StepStatus::Pending) { cx.theme.text_muted } else { cx.theme.text };
    cx.cmds.push(DrawCmd::Text {
        x: item.rect.x + (item.rect.w - lw) / 2.0,
        y: dot.y + DOT_D + cx.theme.space(1),
        content: label.to_string(),
        px,
        style: FontStyle::Regular,
        color: label_color,
    });
}

pub(crate) fn paint_connector(cx: &mut PaintCx, item: &Item, filled: bool) {
    // Vertically centered on the dot row (DOT_D tall dots sit at the top).
    let y = item.rect.y + DOT_D / 2.0 - 1.0;
    let rect = Rect::new(item.rect.x, y, item.rect.w, 2.0);
    let color = if filled { cx.theme.accent } else { cx.theme.secondary };
    cx.cmds.push(DrawCmd::Rect { rect, color });
}
