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
    keyboard: Keyboard,
    markdown: bool,
    on_wikilink: &Option<String>,
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
        ItemKind::Textarea {
            value: value.to_string(),
            placeholder: placeholder.to_string(),
            px,
            markdown,
            on_wikilink: on_wikilink.clone(),
            keyboard,
        },
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
    markdown: bool,
    focus: Option<(usize, usize, usize)>,
) -> f32 {
    if markdown {
        return super::markdown_textarea::wrapped_height(value, max_w, px, m, theme, focus);
    }
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

pub(crate) fn wrap_hard_line(
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

pub(crate) fn fit_prefix(
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
    markdown: bool,
    focus: Option<(usize, usize, usize)>,
) -> usize {
    let max_w = (rect.w - 2.0 * theme.btn_pad_x).max(0.0);
    let tx = rect.x + theme.btn_pad_x;
    let ty = rect.y + theme.btn_pad_y;
    if markdown {
        return super::markdown_textarea::caret_byte(
            value, tx, ty, px, max_w, x, y, m, theme, focus,
        );
    }
    if value.is_empty() {
        return 0;
    }
    let (_, th) = m.measure("Ag", px, FontStyle::Regular);
    let line_h = th + TEXTAREA_LEADING;
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

pub(crate) fn paint(
    cx: &mut PaintCx,
    item: &Item,
    value: &str,
    placeholder: &str,
    px: f32,
    markdown: bool,
) {
    let focused = cx
        .focus
        .filter(|f| item.semantics.test_id.as_deref() == Some(f.test_id.as_str()));
    paint_textarea(
        cx.cmds,
        item.rect,
        value,
        placeholder,
        px,
        focused,
        cx.measurer,
        cx.theme,
        markdown,
    );
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
    markdown: bool,
) {
    cmds.push(DrawCmd::PushClip { rect });
    let (tx, ty) = (rect.x + theme.btn_pad_x, rect.y + theme.btn_pad_y);
    let max_w = (rect.w - 2.0 * theme.btn_pad_x).max(0.0);
    let shown;
    let value = if let Some(p) = focus.and_then(|f| f.preedit.as_ref()) {
        shown = p.splice(value);
        shown.as_str()
    } else {
        value
    };

    if value.is_empty() && !placeholder.is_empty() {
        cmds.push(DrawCmd::Text {
            x: tx,
            y: ty,
            content: placeholder.to_string(),
            px,
            style: FontStyle::Regular,
            color: theme.text_muted,
        });
    } else if markdown {
        let focus_tuple = focus.map(|f| (f.caret, f.selection.start, f.selection.end));
        super::markdown_textarea::paint_rows(cmds, tx, ty, max_w, value, px, m, theme, focus_tuple);
    } else {
        paint_plain_textarea(cmds, tx, ty, max_w, value, px, focus, m, theme);
    }
    cmds.push(DrawCmd::PopClip);
}

fn paint_plain_textarea(
    cmds: &mut Vec<DrawCmd>,
    tx: f32,
    ty: f32,
    max_w: f32,
    value: &str,
    px: f32,
    focus: Option<&InputFocus>,
    m: &dyn TextMeasurer,
    theme: &Theme,
) {
    let (_, th) = m.measure("Ag", px, FontStyle::Regular);
    let line_h = th + TEXTAREA_LEADING;

    for (row, (ls, le)) in wrap_visual_lines(value, max_w, px, m).into_iter().enumerate() {
        let line = &value[ls..le];
        let ly = ty + row as f32 * line_h;

        if let Some(f) = focus {
            let sel = &f.selection;
            let s = sel.start.max(ls);
            let e = sel.end.min(le);
            if f.preedit.is_none() && s < e {
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
            if let Some(p) = &f.preedit {
                let ps = p.at;
                let pe = p.at + p.text.len();
                let s = ps.max(ls);
                let e = pe.min(le);
                if s < e {
                    let x0 = tx + m.measure(&value[ls..s], px, FontStyle::Regular).0;
                    let x1 = tx + m.measure(&value[ls..e], px, FontStyle::Regular).0;
                    let y = (ly + th).floor();
                    cmds.push(DrawCmd::Rect {
                        rect: Rect::new(x0.floor(), y, (x1 - x0).max(1.0), 1.0),
                        color: theme.accent,
                    });
                }
            }
            if f.show_caret {
                let caret = f.caret.min(value.len());
                if caret >= ls && caret <= le {
                    let cx = tx + m.measure(&value[ls..caret], px, FontStyle::Regular).0;
                    cmds.push(DrawCmd::Rect {
                        rect: Rect::new(cx, ly, 1.0, th),
                        color: theme.text,
                    });
                }
            }
            if f.handles && !f.selection.is_empty() {
                for byte in [f.selection.start, f.selection.end] {
                    if byte >= ls && byte <= le {
                        let cx = tx + m.measure(&value[ls..byte.min(le)], px, FontStyle::Regular).0;
                        let size = 6.0;
                        cmds.push(DrawCmd::RRect {
                            rect: Rect::new(cx - size / 2.0, ly + (th - size) / 2.0, size, size),
                            radius: size / 2.0,
                            color: theme.accent,
                        });
                    }
                }
            }
        }
    }
}

/// Caret after moving `dir` visual rows (−1 / +1). `sticky_x` is the
/// remembered distance from the text origin; `None` measures it from
/// the caret. Hard newlines and soft wraps are both rows. The editing
/// core stays font-free — this is the layout side of sticky Up/Down.
pub(crate) fn vertical_caret(
    value: &str,
    max_w: f32,
    px: f32,
    caret: usize,
    dir: i32,
    sticky_x: Option<f32>,
    m: &dyn TextMeasurer,
) -> (usize, Option<f32>) {
    if value.is_empty() || dir == 0 {
        return (caret.min(value.len()), sticky_x);
    }
    let lines = wrap_visual_lines(value, max_w, px, m);
    let caret = caret.min(value.len());
    let row = visual_row_index(&lines, caret);
    let (ls, le) = lines[row];
    let x = sticky_x.unwrap_or_else(|| {
        let end = caret.clamp(ls, le);
        m.measure(&value[ls..end], px, FontStyle::Regular).0
    });
    let next = row as i32 + dir;
    if next < 0 {
        return (lines[0].0, Some(x));
    }
    if next as usize >= lines.len() {
        return (value.len(), Some(x));
    }
    let (ns, ne) = lines[next as usize];
    (byte_at_x(value, ns, ne, x, px, m), Some(x))
}

fn visual_row_index(lines: &[(usize, usize)], caret: usize) -> usize {
    let last = lines.len() - 1;
    for (i, &(s, e)) in lines.iter().enumerate() {
        let soft_break = caret == e && lines.get(i + 1).is_some_and(|&(ns, _)| ns == e);
        if soft_break {
            continue;
        }
        if caret >= s && caret <= e {
            return i;
        }
    }
    last
}

fn byte_at_x(
    value: &str,
    ls: usize,
    le: usize,
    rel: f32,
    px: f32,
    m: &dyn TextMeasurer,
) -> usize {
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

    #[test]
    fn vertical_move_follows_soft_wrap_and_keeps_x() {
        let text = "hello world";
        let (byte, x) = vertical_caret(text, 60.0, 14.0, 1, 1, None, &Fixed);
        assert_eq!(byte, 7, "one column into the wrapped line");
        let (back, _) = vertical_caret(text, 60.0, 14.0, byte, -1, x, &Fixed);
        assert_eq!(back, 1);
    }
}
