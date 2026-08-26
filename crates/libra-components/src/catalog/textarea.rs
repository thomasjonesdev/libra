//! Textarea — multi-line controlled text entry (§10.1, PLAN-TEXTAREA
//! stage 4 wrap) on the same pure editing core as Input. Soft-wraps to
//! the laid-out width; Up/Down stay hard-line.

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
    // At least three lines; never shorter than the hard-line source so a
    // parent Scroll can actually scroll a long note. Soft-wrap height is
    // applied in `reflow_textareas` once the width is known.
    let min_h = 3.0 * (th + TEXTAREA_LEADING) + 2.0 * theme.btn_pad_y;
    let lines = value.split('\n').count().max(1) as f32;
    let content_h = lines * (th + TEXTAREA_LEADING) + 2.0 * theme.btn_pad_y;
    let h = content_h.max(min_h);
    let id = cx.tree.new_leaf(Style {
        size: Size { width: auto(), height: auto() },
        min_size: Size { width: auto(), height: length(h) },
        flex_grow: 1.0,
        flex_shrink: 0.0,
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

pub(crate) fn wrapped_height(
    value: &str,
    max_w: f32,
    px: f32,
    m: &dyn TextMeasurer,
    theme: &Theme,
) -> f32 {
    let (_, th) = m.measure("Ag", px, FontStyle::Regular);
    let line_h = th + TEXTAREA_LEADING;
    let min_h = 3.0 * line_h + 2.0 * theme.btn_pad_y;
    let n = wrap_visual_lines(value, max_w, px, m).len().max(1) as f32;
    (n * line_h + 2.0 * theme.btn_pad_y).max(min_h)
}

/// Visual lines of a textarea value: byte ranges into `text`, exclusive
/// of hard `\n`. Soft-wrap prefers spaces; a single overlong token is
/// hard-broken at the character that still fits.
pub(crate) fn wrap_visual_lines(
    text: &str,
    max_w: f32,
    px: f32,
    m: &dyn TextMeasurer,
) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut line_start = 0usize;
    loop {
        let rest = &text[line_start..];
        let line_len = rest.find('\n').unwrap_or(rest.len());
        wrap_hard_line(text, line_start, line_start + line_len, max_w, px, m, &mut out);
        match rest.find('\n') {
            Some(nl) => line_start += nl + 1,
            None => break,
        }
    }
    if out.is_empty() {
        out.push((0, 0));
    }
    out
}

fn wrap_hard_line(
    text: &str,
    start: usize,
    end: usize,
    max_w: f32,
    px: f32,
    m: &dyn TextMeasurer,
    out: &mut Vec<(usize, usize)>,
) {
    if start == end || max_w <= 0.0 {
        out.push((start, end));
        return;
    }
    let mut vis = start;
    while vis < end {
        let (cut, next) = fit_prefix(text, vis, end, max_w, px, m);
        out.push((vis, cut));
        if next <= vis {
            break;
        }
        vis = next;
    }
}

fn fit_prefix(
    text: &str,
    vis: usize,
    end: usize,
    max_w: f32,
    px: f32,
    m: &dyn TextMeasurer,
) -> (usize, usize) {
    let mut last_break = None;
    let mut i = vis;
    while i < end {
        let ch = text[i..].chars().next().unwrap();
        let next = i + ch.len_utf8();
        let (w, _) = m.measure(&text[vis..next], px, FontStyle::Regular);
        if w > max_w && i > vis {
            if let Some(b) = last_break {
                return (b, b);
            }
            return (i, i);
        }
        if w > max_w && i == vis {
            return (next, next);
        }
        if ch == ' ' || ch == '\t' {
            last_break = Some(next);
        }
        i = next;
    }
    (end, end)
}

/// Byte offset nearest `(x, y)` in a soft-wrapped textarea.
pub(crate) fn caret_byte(
    value: &str,
    rect: Rect,
    px: f32,
    x: f32,
    y: f32,
    m: &dyn TextMeasurer,
    theme: &Theme,
) -> usize {
    if value.is_empty() {
        return 0;
    }
    let (tx, ty) = (rect.x + theme.btn_pad_x, rect.y + theme.btn_pad_y);
    let (_, th) = m.measure("Ag", px, FontStyle::Regular);
    let line_h = th + TEXTAREA_LEADING;
    let max_w = (rect.w - 2.0 * theme.btn_pad_x).max(0.0);
    let lines = wrap_visual_lines(value, max_w, px, m);
    if y < ty {
        return lines[0].0;
    }
    let row = ((y - ty) / line_h).floor().max(0.0) as usize;
    if row >= lines.len() {
        return value.len();
    }
    let (ls, le) = lines[row];
    let rel = x - tx;
    if rel <= 0.0 {
        return ls;
    }
    let mut prev_i = ls;
    let mut prev_w = 0.0f32;
    let mut i = ls;
    while i < le {
        let ch = value[i..].chars().next().unwrap();
        i += ch.len_utf8();
        let w = m.measure(&value[ls..i], px, FontStyle::Regular).0;
        if w >= rel {
            return if (rel - prev_w) <= (w - rel) { prev_i } else { i };
        }
        prev_i = i;
        prev_w = w;
    }
    le
}

pub(crate) fn paint(cx: &mut PaintCx, item: &Item, value: &str, placeholder: &str, px: f32) {
    let focused = cx
        .focus
        .filter(|f| item.semantics.test_id.as_deref() == Some(f.test_id.as_str()));
    paint_textarea(cx.cmds, item.rect, value, placeholder, px, focused, cx.measurer, cx.theme);
}

/// Textarea painting (§10.1 / PLAN-TEXTAREA stage 4): **no field
/// chrome** — the editor dissolves into the pane background (Input
/// keeps the boxed `secondary` fill). Soft-wrapped visual lines, muted
/// placeholder when empty, and — when focused — the accent focus ring,
/// per-line selection bands, and a line-positioned caret.
/// Line height is measured height + [`TEXTAREA_LEADING`]; x positions
/// come from prefix advances within the caret's visual line.
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
    cmds.push(DrawCmd::PushClip { rect });
    let (tx, ty) = (rect.x + theme.btn_pad_x, rect.y + theme.btn_pad_y);
    let (_, th) = m.measure("Ag", px, FontStyle::Regular);
    let line_h = th + TEXTAREA_LEADING;
    let max_w = (rect.w - 2.0 * theme.btn_pad_x).max(0.0);

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

    for (row, (ls, le)) in wrap_visual_lines(value, max_w, px, m).into_iter().enumerate() {
        let line = &value[ls..le];
        let ly = ty + row as f32 * line_h;

        if let Some(f) = focus {
            let sel = &f.selection;
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
            if caret >= ls && caret <= le {
                let cx = tx + m.measure(&value[ls..caret], px, FontStyle::Regular).0;
                cmds.push(DrawCmd::Rect { rect: Rect::new(cx, ly, 1.0, th), color: theme.text });
            }
        }
    }
    cmds.push(DrawCmd::PopClip);
}

#[cfg(test)]
mod tests {
    use super::*;
    use libra_draw::FontStyle;

    struct Fixed;

    impl TextMeasurer for Fixed {
        fn measure(&self, text: &str, _px: f32, _style: FontStyle) -> (f32, f32) {
            (text.chars().count() as f32 * 10.0, 14.0)
        }
    }

    #[test]
    fn wraps_on_spaces_and_keeps_byte_ranges() {
        let text = "hello world";
        let lines = wrap_visual_lines(text, 60.0, 14.0, &Fixed);
        assert_eq!(lines, vec![(0, 6), (6, 11)]);
        assert_eq!(&text[0..6], "hello ");
        assert_eq!(&text[6..11], "world");
    }

    #[test]
    fn hard_newlines_stay_visual_breaks() {
        let text = "aa\nbb";
        let lines = wrap_visual_lines(text, 1000.0, 14.0, &Fixed);
        assert_eq!(lines, vec![(0, 2), (3, 5)]);
    }
}
