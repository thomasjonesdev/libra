//! `<Icon name="plus">` — closed vector registry (§7.1, PLAN-CATALOG).
//!
//! Glyphs are `DrawCmd::Polyline` (and full-radius `RRect` dots). No
//! icon font, no SVG files, no path/arc command — the same trick
//! Checkbox and Search already use. Tokens only (`theme.text` /
//! `theme.text_muted`).
//!
//! Gloss chrome uses the same names via `Button icon="…"`.

use crate::layout::BuildCx;
use crate::node::*;
use crate::paint::PaintCx;
use libra_draw::{Color, DrawCmd, Rect, Theme};
use taffy::prelude::*;

/// Logical glyph box; density-scaled via [`Theme::dense`].
pub(crate) const ICON: f32 = 16.0;

pub(crate) fn build(
    cx: &mut BuildCx,
    name: IconName,
    muted: bool,
    test_id: &Option<String>,
) -> Result<NodeId, taffy::TaffyError> {
    let s = cx.theme.dense(ICON);
    let id = cx.tree.new_leaf(Style {
        size: Size { width: length(s), height: length(s) },
        flex_shrink: 0.0,
        ..Default::default()
    })?;
    cx.meta.push((
        id,
        ItemKind::Icon { name, muted },
        Semantics {
            role: Role::Text,
            text: name.as_str().to_string(),
            test_id: test_id.clone(),
        },
        None,
    ));
    Ok(id)
}

pub(crate) fn paint(cx: &mut PaintCx, item: &Item, name: IconName, muted: bool) {
    let color = if muted { cx.theme.text_muted } else { cx.theme.text };
    paint_name(cx.cmds, item.rect, cx.theme, name, color);
}

/// Shared painter for `<Icon>` and `Button icon="…"`.
pub(crate) fn paint_name(
    cmds: &mut Vec<DrawCmd>,
    rect: Rect,
    theme: &Theme,
    name: IconName,
    color: Color,
) {
    let s = theme.dense(ICON).min(rect.w).min(rect.h);
    let x0 = rect.x + (rect.w - s) / 2.0;
    let y0 = rect.y + (rect.h - s) / 2.0;
    let w = theme.dense(1.5).max(1.0);
    let p = |u: f32, v: f32| (x0 + u * s, y0 + v * s);
    let stroke = |cmds: &mut Vec<DrawCmd>, pts: Vec<(f32, f32)>| {
        cmds.push(DrawCmd::Polyline { points: pts, width: w, color });
    };
    match name {
        IconName::Plus => {
            stroke(cmds, vec![p(0.20, 0.50), p(0.80, 0.50)]);
            stroke(cmds, vec![p(0.50, 0.20), p(0.50, 0.80)]);
        }
        IconName::Minus => {
            stroke(cmds, vec![p(0.22, 0.50), p(0.78, 0.50)]);
        }
        IconName::Close => {
            stroke(cmds, vec![p(0.26, 0.26), p(0.74, 0.74)]);
            stroke(cmds, vec![p(0.74, 0.26), p(0.26, 0.74)]);
        }
        IconName::Check => {
            stroke(cmds, vec![p(0.20, 0.52), p(0.42, 0.74), p(0.80, 0.26)]);
        }
        IconName::Search => {
            let (cx, cy) = p(0.42, 0.42);
            let r = s * 0.22;
            let mut lens = Vec::with_capacity(13);
            for i in 0..=12 {
                let a = (i as f32 / 12.0) * std::f32::consts::TAU;
                lens.push((cx + r * a.cos(), cy + r * a.sin()));
            }
            stroke(cmds, lens);
            stroke(cmds, vec![
                (cx + r * 0.72, cy + r * 0.72),
                (cx + r * 1.85, cy + r * 1.85),
            ]);
        }
        IconName::Settings => {
            let (cx, cy) = p(0.50, 0.50);
            let r = s * 0.18;
            let mut ring = Vec::with_capacity(13);
            for i in 0..=12 {
                let a = (i as f32 / 12.0) * std::f32::consts::TAU;
                ring.push((cx + r * a.cos(), cy + r * a.sin()));
            }
            stroke(cmds, ring);
            for i in 0..6 {
                let a = (i as f32 / 6.0) * std::f32::consts::TAU;
                let (c, sn) = (a.cos(), a.sin());
                stroke(cmds, vec![
                    (cx + r * 1.15 * c, cy + r * 1.15 * sn),
                    (cx + r * 1.85 * c, cy + r * 1.85 * sn),
                ]);
            }
        }
        IconName::Undo => {
            let mut arc = Vec::with_capacity(9);
            for i in 0..=8 {
                let a = std::f32::consts::PI * (0.15 + 0.95 * (i as f32 / 8.0));
                arc.push(p(0.50 + 0.28 * a.cos(), 0.52 - 0.22 * a.sin()));
            }
            stroke(cmds, arc);
            stroke(cmds, vec![p(0.22, 0.42), p(0.22, 0.22), p(0.40, 0.22)]);
        }
        IconName::Calendar => {
            stroke(cmds, vec![
                p(0.22, 0.32),
                p(0.78, 0.32),
                p(0.78, 0.80),
                p(0.22, 0.80),
                p(0.22, 0.32),
            ]);
            stroke(cmds, vec![p(0.34, 0.20), p(0.34, 0.40)]);
            stroke(cmds, vec![p(0.66, 0.20), p(0.66, 0.40)]);
            stroke(cmds, vec![p(0.22, 0.48), p(0.78, 0.48)]);
        }
        IconName::Graph => {
            let a = p(0.22, 0.72);
            let b = p(0.50, 0.24);
            let c = p(0.78, 0.58);
            stroke(cmds, vec![a, b, c]);
            dot(cmds, theme, a, color);
            dot(cmds, theme, b, color);
            dot(cmds, theme, c, color);
        }
        IconName::Folder => {
            stroke(cmds, vec![
                p(0.16, 0.36),
                p(0.16, 0.78),
                p(0.84, 0.78),
                p(0.84, 0.40),
                p(0.50, 0.40),
                p(0.42, 0.28),
                p(0.16, 0.28),
                p(0.16, 0.36),
            ]);
        }
        IconName::Sort => {
            stroke(cmds, vec![p(0.22, 0.32), p(0.78, 0.32)]);
            stroke(cmds, vec![p(0.22, 0.50), p(0.64, 0.50)]);
            stroke(cmds, vec![p(0.22, 0.68), p(0.50, 0.68)]);
        }
        IconName::Back => {
            stroke(cmds, vec![p(0.58, 0.22), p(0.30, 0.50), p(0.58, 0.78)]);
        }
        IconName::Forward => {
            stroke(cmds, vec![p(0.42, 0.22), p(0.70, 0.50), p(0.42, 0.78)]);
        }
        IconName::Command => {
            stroke(cmds, vec![p(0.22, 0.38), p(0.22, 0.22), p(0.38, 0.22)]);
            stroke(cmds, vec![p(0.62, 0.22), p(0.78, 0.22), p(0.78, 0.38)]);
            stroke(cmds, vec![p(0.78, 0.62), p(0.78, 0.78), p(0.62, 0.78)]);
            stroke(cmds, vec![p(0.38, 0.78), p(0.22, 0.78), p(0.22, 0.62)]);
        }
    }
}

fn dot(cmds: &mut Vec<DrawCmd>, theme: &Theme, (x, y): (f32, f32), color: Color) {
    let d = theme.dense(3.5);
    cmds.push(DrawCmd::RRect {
        rect: Rect::new(x - d / 2.0, y - d / 2.0, d, d),
        radius: theme.radius_full,
        color,
    });
}
