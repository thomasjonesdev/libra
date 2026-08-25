//! Model-owned form controls (§7.1, PLAN-CATALOG): `Switch`,
//! `Checkbox`, `Radio`. The bound bool is the truth; the press
//! dispatches; the app flips — the same controlled shape as Input.
//! The check glyph is a display-list Polyline; the circles are
//! full-radius RRects (no arc command needed).

use crate::layout::BuildCx;
use crate::node::*;
use crate::paint::PaintCx;
use libra_draw::{Color, DrawCmd, FontStyle, Rect};
use taffy::prelude::*;

const SWITCH_W: f32 = 36.0;
const SWITCH_H: f32 = 20.0;
const BOX_S: f32 = 16.0;

pub(crate) fn build_switch(
    cx: &mut BuildCx,
    checked: bool,
    on_toggle: &str,
    test_id: &Option<String>,
) -> Result<NodeId, taffy::TaffyError> {
    let id = cx.tree.new_leaf(Style {
        size: Size { width: length(SWITCH_W), height: length(SWITCH_H) },
        flex_shrink: 0.0,
        ..Default::default()
    })?;
    cx.meta.push((
        id,
        ItemKind::SwitchCtl { checked },
        Semantics {
            role: Role::Switch,
            text: if checked { "on" } else { "off" }.to_string(),
            test_id: test_id.clone(),
        },
        Some((on_toggle.to_string(), None)),
    ));
    Ok(id)
}

fn labeled_control_size(cx: &BuildCx, label: &str, px: f32) -> (f32, f32) {
    let (tw, th) = cx.m.measure(label, px, FontStyle::Regular);
    let w = BOX_S + if label.is_empty() { 0.0 } else { cx.theme.space(3) + tw };
    (w, th.max(BOX_S))
}

pub(crate) fn build_checkbox(
    cx: &mut BuildCx,
    checked: bool,
    on_toggle: &str,
    label: &str,
    test_id: &Option<String>,
) -> Result<NodeId, taffy::TaffyError> {
    let px = cx.theme.type_md;
    let (w, h) = labeled_control_size(cx, label, px);
    let id = cx.tree.new_leaf(Style {
        size: Size { width: length(w), height: length(h) },
        flex_shrink: 0.0,
        ..Default::default()
    })?;
    cx.meta.push((
        id,
        ItemKind::CheckboxCtl { checked, label: label.to_string(), px },
        Semantics { role: Role::Checkbox, text: label.to_string(), test_id: test_id.clone() },
        Some((on_toggle.to_string(), None)),
    ));
    Ok(id)
}

pub(crate) fn build_radio(
    cx: &mut BuildCx,
    selected: bool,
    on_select: &str,
    on_select_arg: &Option<String>,
    label: &str,
    test_id: &Option<String>,
) -> Result<NodeId, taffy::TaffyError> {
    let px = cx.theme.type_md;
    let (w, h) = labeled_control_size(cx, label, px);
    let id = cx.tree.new_leaf(Style {
        size: Size { width: length(w), height: length(h) },
        flex_shrink: 0.0,
        ..Default::default()
    })?;
    cx.meta.push((
        id,
        ItemKind::RadioCtl { selected, label: label.to_string(), px },
        Semantics { role: Role::Radio, text: label.to_string(), test_id: test_id.clone() },
        Some((on_select.to_string(), on_select_arg.clone())),
    ));
    Ok(id)
}

pub(crate) fn paint_switch(cx: &mut PaintCx, rect: Rect, checked: bool) {
    let track = if checked { cx.theme.accent } else { cx.theme.secondary };
    cx.cmds.push(DrawCmd::RRect { rect, radius: cx.theme.radius_full, color: track });
    let knob_d = rect.h - 4.0;
    let knob_x = if checked { rect.x + rect.w - knob_d - 2.0 } else { rect.x + 2.0 };
    cx.cmds.push(DrawCmd::RRect {
        rect: Rect::new(knob_x, rect.y + 2.0, knob_d, knob_d),
        radius: cx.theme.radius_full,
        color: cx.theme.accent_text,
    });
}

fn label_x(rect: Rect, theme: &libra_draw::Theme) -> f32 {
    rect.x + BOX_S + theme.space(3)
}

fn control_box(rect: Rect) -> Rect {
    Rect::new(rect.x, rect.y + (rect.h - BOX_S) / 2.0, BOX_S, BOX_S)
}

fn paint_label(cx: &mut PaintCx, rect: Rect, label: &str, px: f32) {
    if label.is_empty() {
        return;
    }
    let (_, th) = cx.measurer.measure("Ag", px, FontStyle::Regular);
    cx.cmds.push(DrawCmd::Text {
        x: label_x(rect, cx.theme),
        y: rect.y + (rect.h - th) / 2.0,
        content: label.to_string(),
        px,
        style: FontStyle::Regular,
        color: cx.theme.text,
    });
}

/// Hairline outline for the unchecked box/ring (four thin rects — the
fn outline(cx: &mut PaintCx, r: Rect, color: Color) {
    crate::catalog::stroke_rrect_inside(cx.cmds, r, cx.theme.radius_sm, 1.0, color);
}

pub(crate) fn paint_checkbox(cx: &mut PaintCx, rect: Rect, checked: bool, label: &str, px: f32) {
    let b = control_box(rect);
    if checked {
        cx.cmds.push(DrawCmd::RRect { rect: b, radius: cx.theme.radius_sm, color: cx.theme.accent });
        // Check glyph: a two-segment polyline in box-relative coords.
        cx.cmds.push(DrawCmd::Polyline {
            points: vec![
                (b.x + 0.25 * b.w, b.y + 0.55 * b.h),
                (b.x + 0.45 * b.w, b.y + 0.72 * b.h),
                (b.x + 0.78 * b.w, b.y + 0.30 * b.h),
            ],
            width: 1.6,
            color: cx.theme.accent_text,
        });
    } else {
        cx.cmds.push(DrawCmd::RRect { rect: b, radius: cx.theme.radius_sm, color: cx.theme.secondary });
        outline(cx, b, cx.theme.hairline);
    }
    paint_label(cx, rect, label, px);
}

pub(crate) fn paint_radio(cx: &mut PaintCx, rect: Rect, selected: bool, label: &str, px: f32) {
    let b = control_box(rect);
    let ring = if selected { cx.theme.accent } else { cx.theme.secondary };
    cx.cmds.push(DrawCmd::RRect { rect: b, radius: cx.theme.radius_full, color: ring });
    if selected {
        let d = 6.0;
        cx.cmds.push(DrawCmd::RRect {
            rect: Rect::new(b.x + (b.w - d) / 2.0, b.y + (b.h - d) / 2.0, d, d),
            radius: cx.theme.radius_full,
            color: cx.theme.accent_text,
        });
    } else {
        let inner = Rect::new(b.x + 2.0, b.y + 2.0, b.w - 4.0, b.h - 4.0);
        cx.cmds.push(DrawCmd::RRect {
            rect: inner,
            radius: cx.theme.radius_full,
            color: cx.theme.bg,
        });
    }
    paint_label(cx, rect, label, px);
}
