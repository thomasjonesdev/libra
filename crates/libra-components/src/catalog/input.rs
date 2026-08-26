//! Input — single-line controlled text entry (§10.1). The model owns the
//! value; `test_id` is the focus identity. Clicking focuses (the shell/harness
//! special-case TextInput hits) rather than dispatching; `on_change` rides in
//! handler storage.

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
    let h = th + 2.0 * theme.btn_pad_y;
    let id = cx.tree.new_leaf(Style {
        size: Size { width: auto(), height: length(h) },
        flex_grow: 1.0,
        flex_shrink: 1.0,
        ..Default::default()
    })?;
    cx.meta.push((
        id,
        ItemKind::Input { value: value.to_string(), placeholder: placeholder.to_string(), px },
        Semantics {
            role: Role::TextInput,
            text: value.to_string(),
            test_id: Some(test_id.to_string()),
        },
        // Clicking focuses rather than dispatching; the shell/harness
        // special-case TextInput hits. on_change rides in `on_press`
        // storage (it is "the handler name"); no markup-bound payload
        // (the control supplies the text).
        Some((on_change.to_string(), None)),
    ));
    Ok(id)
}

pub(crate) fn paint(cx: &mut PaintCx, item: &Item, value: &str, placeholder: &str, px: f32) {
    paint_field(cx, item, value, placeholder, px, 0.0);
}

/// Shared field chrome used by Input and `<Search>` (PLAN-CATALOG).
/// `leading` reserves extra left inset so Search can draw a glyph
/// without overlapping the value.
pub(crate) fn paint_field(
    cx: &mut PaintCx,
    item: &Item,
    value: &str,
    placeholder: &str,
    px: f32,
    leading: f32,
) {
    let focused = cx
        .focus
        .filter(|f| item.semantics.test_id.as_deref() == Some(f.test_id.as_str()));
    paint_input(
        cx.cmds,
        item.rect,
        value,
        placeholder,
        px,
        leading,
        focused,
        cx.measurer,
        cx.theme,
    );
}

/// Byte offset nearest `x` in a single-line field (Input / Search).
pub(crate) fn caret_byte(
    value: &str,
    rect: Rect,
    px: f32,
    leading: f32,
    x: f32,
    m: &dyn TextMeasurer,
    theme: &Theme,
    focus: Option<&InputFocus>,
) -> usize {
    let left = theme.btn_pad_x + leading;
    let inner_w = (rect.w - left - theme.btn_pad_x).max(0.0);
    let scroll_x = input_scroll_x(m, value, px, inner_w, focus);
    let rel = x - (rect.x + left) + scroll_x;
    caret_at_advance(value, px, rel, m)
}

fn caret_at_advance(value: &str, px: f32, rel: f32, m: &dyn TextMeasurer) -> usize {
    if rel <= 0.0 || value.is_empty() {
        return 0;
    }
    let mut prev_i = 0usize;
    let mut prev_w = 0.0f32;
    let mut i = 0usize;
    while i < value.len() {
        let ch = value[i..].chars().next().unwrap();
        i += ch.len_utf8();
        let w = m.measure(&value[..i], px, FontStyle::Regular).0;
        if w >= rel {
            return if (rel - prev_w) <= (w - rel) { prev_i } else { i };
        }
        prev_i = i;
        prev_w = w;
    }
    value.len()
}

/// Horizontal scroll offset so the caret and selection stay visible in a
/// clipped single-line field. Computed at paint time from focus state —
/// arrow keys and typing move the caret, which moves the viewport.
fn input_scroll_x(
    m: &dyn TextMeasurer,
    value: &str,
    px: f32,
    inner_w: f32,
    focus: Option<&InputFocus>,
) -> f32 {
    if inner_w <= 0.0 || value.is_empty() {
        return 0.0;
    }
    let text_w = m.measure(value, px, FontStyle::Regular).0;
    let max_scroll = (text_w - inner_w).max(0.0);
    if max_scroll <= 0.0 {
        return 0.0;
    }
    let Some(f) = focus else {
        return 0.0;
    };
    let len = value.len();
    let advance = |i: usize| m.measure(&value[..i.min(len)], px, FontStyle::Regular).0;
    let mut scroll_x = 0.0f32;
    let mut reveal = |x: f32| {
        if x < scroll_x {
            scroll_x = x;
        } else if x > scroll_x + inner_w {
            scroll_x = x - inner_w;
        }
    };
    reveal(advance(f.caret.min(len)));
    if !f.selection.is_empty() {
        reveal(advance(f.selection.start.min(len)));
        reveal(advance(f.selection.end.min(len)));
    }
    scroll_x.clamp(0.0, max_scroll)
}

/// Input painting (§10.1): field chrome, value or muted placeholder,
/// and — when focused — a rounded accent focus ring (inset fill over an
/// accent underlay — follows `radius_md`), the selection band, and a caret.
/// Caret/selection x-positions come from prefix advance widths, so the
/// same measurer that laid the text positions the caret.
#[allow(clippy::too_many_arguments)]
fn paint_input(
    cmds: &mut Vec<DrawCmd>,
    rect: Rect,
    value: &str,
    placeholder: &str,
    px: f32,
    leading: f32,
    focus: Option<&InputFocus>,
    m: &dyn TextMeasurer,
    theme: &Theme,
) {
    // Boxed field: `secondary` fill + 1px `StrokeRRect` (accent when
    // focused, `border` otherwise) so the frame follows `radius_md`.
    let radius = theme.radius_md;
    cmds.push(DrawCmd::RRect { rect, radius, color: theme.secondary });
    let stroke = if focus.is_some() { theme.accent } else { theme.border };
    crate::catalog::stroke_rrect_inside(cmds, rect, radius, 1.0, stroke);
    let ty = rect.y + theme.btn_pad_y;
    let (_, th) = m.measure("Ag", px, FontStyle::Regular);
    let left = theme.btn_pad_x + leading;
    let inner = Rect::new(
        rect.x + left,
        rect.y + theme.btn_pad_y,
        (rect.w - left - theme.btn_pad_x).max(0.0),
        (rect.h - 2.0 * theme.btn_pad_y).max(0.0),
    );
    cmds.push(DrawCmd::PushClip { rect: inner });

    let scroll_x = input_scroll_x(m, value, px, inner.w, focus);
    let tx = rect.x + left - scroll_x;

    if let Some(f) = focus {
        // Selection band behind the text.
        let sel = &f.selection;
        if !sel.is_empty() && sel.end <= value.len() {
            let x0 = tx + m.measure(&value[..sel.start], px, FontStyle::Regular).0;
            let x1 = tx + m.measure(&value[..sel.end], px, FontStyle::Regular).0;
            cmds.push(DrawCmd::Rect {
                rect: Rect::new(x0, ty, (x1 - x0).max(1.0), th),
                color: theme.selection,
            });
        }
    }
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
    } else {
        cmds.push(DrawCmd::Text {
            x: tx,
            y: ty,
            content: value.to_string(),
            px,
            style: FontStyle::Regular,
            color: theme.text,
        });
    }
    if let Some(f) = focus {
        let caret = f.caret.min(value.len());
        let cx = tx + m.measure(&value[..caret], px, FontStyle::Regular).0;
        cmds.push(DrawCmd::Rect {
            rect: Rect::new(cx, ty, 1.0, th),
            color: theme.text,
        });
    }
    cmds.push(DrawCmd::PopClip);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::InputFocus;

    struct UnitWidth;

    impl TextMeasurer for UnitWidth {
        fn measure(&self, text: &str, _px: f32, _style: FontStyle) -> (f32, f32) {
            (text.chars().count() as f32, 16.0)
        }
    }

    #[test]
    fn scroll_x_stays_zero_when_text_fits() {
        let m = UnitWidth;
        let focus = InputFocus {
            test_id: "f".into(),
            caret: 4,
            selection: 4..4,
        };
        assert_eq!(input_scroll_x(&m, "abcd", 14.0, 8.0, Some(&focus)), 0.0);
    }

    #[test]
    fn scroll_x_follows_caret_at_end() {
        let m = UnitWidth;
        let focus = InputFocus {
            test_id: "f".into(),
            caret: 12,
            selection: 12..12,
        };
        // 12 chars, viewport 8 — caret at end reveals the tail.
        assert_eq!(
            input_scroll_x(&m, "abcdefghijkl", 14.0, 8.0, Some(&focus)),
            4.0
        );
    }

    #[test]
    fn scroll_x_reveals_selection_edges() {
        let m = UnitWidth;
        let focus = InputFocus {
            test_id: "f".into(),
            caret: 10,
            selection: 2..10,
        };
        assert_eq!(
            input_scroll_x(&m, "abcdefghijkl", 14.0, 6.0, Some(&focus)),
            4.0
        );
    }

    #[test]
    fn scroll_x_is_zero_without_focus() {
        let m = UnitWidth;
        assert_eq!(input_scroll_x(&m, "abcdefghijkl", 14.0, 4.0, None), 0.0);
    }
}
