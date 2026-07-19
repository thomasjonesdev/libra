//! Textarea — multi-line controlled text entry (§10.1, PLAN-TEXTAREA stage 1)
//! on the same pure editing core as Input. Hard newlines only (no soft wrap
//! yet); grows to fill its column.

use crate::layout::BuildCx;
use crate::node::*;
use crate::paint::PaintCx;
use libra_draw::{DrawCmd, FontStyle, Rect, Theme};
use taffy::prelude::*;

pub(crate) fn build(
    cx: &mut BuildCx,
    value: &str,
    placeholder: &str,
    on_change: &str,
    test_id: &str,
) -> Result<NodeId, taffy::TaffyError> {
    let theme = cx.theme;
    let px = theme.type_md;
    let (_, th) = cx.m.measure("Ag", px, FontStyle::Regular);
    // Minimum three lines; grows to fill the column.
    let min_h = 3.0 * (th + TEXTAREA_LEADING) + 2.0 * theme.btn_pad_y;
    let id = cx.tree.new_leaf(Style {
        size: Size { width: auto(), height: auto() },
        min_size: Size { width: auto(), height: length(min_h) },
        flex_grow: 1.0,
        flex_shrink: 1.0,
        ..Default::default()
    })?;
    cx.meta.push((
        id,
        ItemKind::Textarea { value: value.to_string(), placeholder: placeholder.to_string(), px },
        Semantics {
            role: Role::TextInput,
            text: value.to_string(),
            test_id: Some(test_id.to_string()),
        },
        Some((on_change.to_string(), None)),
    ));
    Ok(id)
}

pub(crate) fn paint(cx: &mut PaintCx, item: &Item, value: &str, placeholder: &str, px: f32) {
    let focused = cx
        .focus
        .filter(|f| item.semantics.test_id.as_deref() == Some(f.test_id.as_str()));
    paint_textarea(cx.cmds, item.rect, value, placeholder, px, focused, cx.measurer, cx.theme);
}

/// Textarea painting (§10.1 stage 1): field chrome, hard-newline lines
/// (no soft wrap yet), muted placeholder when empty, and — when
/// focused — the focus ring, per-line selection bands, and a
/// line-positioned caret. Line height is measured height +
/// [`TEXTAREA_LEADING`]; x positions come from prefix advances within
/// the caret's line, same measurer that laid the text.
#[allow(clippy::too_many_arguments)]
fn paint_textarea(
    cmds: &mut Vec<DrawCmd>,
    rect: Rect,
    value: &str,
    placeholder: &str,
    px: f32,
    focus: Option<&InputFocus>,
    m: &dyn TextMeasurer,
    theme: &Theme,
) {
    cmds.push(DrawCmd::RRect { rect, radius: theme.radius_md, color: theme.secondary });
    if focus.is_some() {
        let c = theme.accent;
        cmds.push(DrawCmd::Rect { rect: Rect::new(rect.x, rect.y, rect.w, 1.0), color: c });
        cmds.push(DrawCmd::Rect { rect: Rect::new(rect.x, rect.y + rect.h - 1.0, rect.w, 1.0), color: c });
        cmds.push(DrawCmd::Rect { rect: Rect::new(rect.x, rect.y, 1.0, rect.h), color: c });
        cmds.push(DrawCmd::Rect { rect: Rect::new(rect.x + rect.w - 1.0, rect.y, 1.0, rect.h), color: c });
    }
    let (tx, ty) = (rect.x + theme.btn_pad_x, rect.y + theme.btn_pad_y);
    let (_, th) = m.measure("Ag", px, FontStyle::Regular);
    let line_h = th + TEXTAREA_LEADING;

    if value.is_empty() {
        if !placeholder.is_empty() {
            cmds.push(DrawCmd::Text {
                x: tx,
                y: ty,
                content: placeholder.to_string(),
                px,
                style: FontStyle::Regular,
                color: theme.text_muted,
            });
        }
    }

    // Walk lines with their byte ranges so selection/caret map exactly.
    let mut line_start = 0usize;
    let mut row = 0usize;
    loop {
        let rest = &value[line_start..];
        let line_len = rest.find('\n').unwrap_or(rest.len());
        let line = &value[line_start..line_start + line_len];
        let ly = ty + row as f32 * line_h;

        if let Some(f) = focus {
            // Selection band: this line's overlap with the selection.
            let sel = &f.selection;
            let (ls, le) = (line_start, line_start + line_len);
            let s = sel.start.max(ls);
            let e = sel.end.min(le);
            if s < e {
                let x0 = tx + m.measure(&value[ls..s], px, FontStyle::Regular).0;
                let x1 = tx + m.measure(&value[ls..e], px, FontStyle::Regular).0;
                cmds.push(DrawCmd::Rect {
                    rect: Rect::new(x0, ly, (x1 - x0).max(1.0), th),
                    color: theme.selection,
                });
            }
        }
        if !line.is_empty() {
            cmds.push(DrawCmd::Text {
                x: tx,
                y: ly,
                content: line.to_string(),
                px,
                style: FontStyle::Regular,
                color: theme.text,
            });
        }
        if let Some(f) = focus {
            let caret = f.caret.min(value.len());
            if caret >= line_start && caret <= line_start + line_len {
                let cx = tx + m.measure(&value[line_start..caret], px, FontStyle::Regular).0;
                cmds.push(DrawCmd::Rect { rect: Rect::new(cx, ly, 1.0, th), color: theme.text });
            }
        }

        match rest.find('\n') {
            Some(nl) => {
                line_start += nl + 1;
                row += 1;
            }
            None => break,
        }
    }
}
