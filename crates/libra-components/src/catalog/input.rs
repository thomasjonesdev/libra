//! Input — single-line controlled text entry (§10.1). The model owns the
//! value; `test_id` is the focus identity. Clicking focuses (the shell/harness
//! special-case TextInput hits) rather than dispatching; `on_change` rides in
//! handler storage.

use crate::layout::BuildCx;
use crate::node::*;
use crate::paint::PaintCx;
use libra_draw::{DrawCmd, FontStyle, Rect, Theme};
use taffy::prelude::*;

fn font_style(bold: bool) -> FontStyle {
    if bold {
        FontStyle::Bold
    } else {
        FontStyle::Regular
    }
}

fn field_pad(variant: InputVariant, theme: &Theme) -> (f32, f32) {
    match variant {
        InputVariant::Field => (theme.btn_pad_x, theme.btn_pad_y),
        InputVariant::Plain => (0.0, 0.0),
    }
}

pub(crate) fn build(
    cx: &mut BuildCx,
    value: &str,
    placeholder: &str,
    on_change: &str,
    on_submit: Option<&str>,
    variant: InputVariant,
    size: TextSize,
    bold: bool,
    keyboard: Keyboard,
    test_id: &str,
) -> Result<NodeId, taffy::TaffyError> {
    let theme = cx.theme;
    let px = size.px(theme);
    let style = font_style(bold);
    let (_, th) = cx.m.measure("Ag", px, style);
    let (_, pad_y) = field_pad(variant, theme);
    let h = th + 2.0 * pad_y;
    let id = cx.tree.new_leaf(Style {
        size: Size { width: auto(), height: length(h) },
        flex_grow: 1.0,
        flex_shrink: 1.0,
        ..Default::default()
    })?;
    cx.meta.push((
        id,
        ItemKind::Input {
            value: value.to_string(),
            placeholder: placeholder.to_string(),
            px,
            on_submit: on_submit.map(str::to_string),
            variant,
            bold,
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

pub(crate) fn paint(
    cx: &mut PaintCx,
    item: &Item,
    value: &str,
    placeholder: &str,
    px: f32,
    variant: InputVariant,
    bold: bool,
) {
    let focused = cx
        .focus
        .filter(|f| item.semantics.test_id.as_deref() == Some(f.test_id.as_str()));
    match variant {
        InputVariant::Field => paint_field_input(cx, item, value, placeholder, px, bold, 0.0, focused),
        InputVariant::Plain => paint_plain(cx, item, value, placeholder, px, bold, focused),
    }
}

/// Shared field chrome used by `<Search>` (PLAN-CATALOG).
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
        false,
        leading,
        InputVariant::Field,
        focused,
        cx.measurer,
        cx.theme,
    );
}

fn paint_field_input(
    cx: &mut PaintCx,
    item: &Item,
    value: &str,
    placeholder: &str,
    px: f32,
    bold: bool,
    leading: f32,
    focus: Option<&InputFocus>,
) {
    paint_input(
        cx.cmds,
        item.rect,
        value,
        placeholder,
        px,
        bold,
        leading,
        InputVariant::Field,
        focus,
        cx.measurer,
        cx.theme,
    );
}

fn paint_plain(
    cx: &mut PaintCx,
    item: &Item,
    value: &str,
    placeholder: &str,
    px: f32,
    bold: bool,
    focus: Option<&InputFocus>,
) {
    paint_input(
        cx.cmds,
        item.rect,
        value,
        placeholder,
        px,
        bold,
        0.0,
        InputVariant::Plain,
        focus,
        cx.measurer,
        cx.theme,
    );
}

/// Byte offset nearest `x` in a single-line field (Input / Search).
pub(crate) fn caret_byte(
    value: &str,
    rect: Rect,
    px: f32,
    bold: bool,
    variant: InputVariant,
    leading: f32,
    x: f32,
    m: &dyn TextMeasurer,
    theme: &Theme,
    focus: Option<&InputFocus>,
) -> usize {
    let (pad_x, _) = field_pad(variant, theme);
    let left = pad_x + leading;
    let inner_w = (rect.w - left - pad_x).max(0.0);
    let style = font_style(bold);
    let scroll_x = input_scroll_x(m, value, px, style, inner_w, focus);
    let rel = x - (rect.x + left) + scroll_x;
    caret_at_advance(value, px, style, rel, m)
}

fn caret_at_advance(value: &str, px: f32, style: FontStyle, rel: f32, m: &dyn TextMeasurer) -> usize {
    if rel <= 0.0 || value.is_empty() {
        return 0;
    }
    let mut prev_i = 0usize;
    let mut prev_w = 0.0f32;
    let mut i = 0usize;
    while i < value.len() {
        let ch = value[i..].chars().next().unwrap();
        i += ch.len_utf8();
        let w = m.measure(&value[..i], px, style).0;
        if w >= rel {
            return if (rel - prev_w) <= (w - rel) { prev_i } else { i };
        }
        prev_i = i;
        prev_w = w;
    }
    value.len()
}

fn input_scroll_x(
    m: &dyn TextMeasurer,
    value: &str,
    px: f32,
    style: FontStyle,
    inner_w: f32,
    focus: Option<&InputFocus>,
) -> f32 {
    if inner_w <= 0.0 || value.is_empty() {
        return 0.0;
    }
    let text_w = m.measure(value, px, style).0;
    let max_scroll = (text_w - inner_w).max(0.0);
    if max_scroll <= 0.0 {
        return 0.0;
    }
    let Some(f) = focus else {
        return 0.0;
    };
    let len = value.len();
    let advance = |i: usize| m.measure(&value[..i.min(len)], px, style).0;
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

#[allow(clippy::too_many_arguments)]
fn paint_input(
    cmds: &mut Vec<DrawCmd>,
    rect: Rect,
    value: &str,
    placeholder: &str,
    px: f32,
    bold: bool,
    leading: f32,
    variant: InputVariant,
    focus: Option<&InputFocus>,
    m: &dyn TextMeasurer,
    theme: &Theme,
) {
    let style = font_style(bold);
    let (pad_x, pad_y) = field_pad(variant, theme);
    if variant == InputVariant::Field {
        let radius = theme.radius_md;
        cmds.push(DrawCmd::RRect { rect, radius, color: theme.secondary });
        let stroke = if focus.is_some() { theme.accent } else { theme.border };
        crate::catalog::stroke_rrect_inside(cmds, rect, radius, 1.0, stroke);
    }
    let (_, th) = m.measure("Ag", px, style);
    let ty = rect.y + pad_y;
    let left = pad_x + leading;
    let inner = Rect::new(
        rect.x + left,
        rect.y + pad_y,
        (rect.w - left - pad_x).max(0.0),
        (rect.h - 2.0 * pad_y).max(0.0),
    );
    cmds.push(DrawCmd::PushClip { rect: inner });

    let shown;
    let value = if let Some(p) = focus.and_then(|f| f.preedit.as_ref()) {
        shown = p.splice(value);
        shown.as_str()
    } else {
        value
    };
    let scroll_x = input_scroll_x(m, value, px, style, inner.w, focus);
    let tx = rect.x + left - scroll_x;

    if let Some(f) = focus {
        let sel = &f.selection;
        if f.preedit.is_none() && !sel.is_empty() && sel.end <= value.len() {
            let x0 = tx + m.measure(&value[..sel.start], px, style).0;
            let x1 = tx + m.measure(&value[..sel.end], px, style).0;
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
                style,
                color: theme.text_muted,
            });
        }
    } else {
        cmds.push(DrawCmd::Text {
            x: tx,
            y: ty,
            content: value.to_string(),
            px,
            style,
            color: theme.text,
        });
    }
    if let Some(f) = focus {
        if let Some(p) = &f.preedit {
            let end = (p.at + p.text.len()).min(value.len());
            let x0 = tx + m.measure(&value[..p.at.min(value.len())], px, style).0;
            let x1 = tx + m.measure(&value[..end], px, style).0;
            paint_underline(cmds, x0, x1, ty + th, theme.accent);
        }
        if f.show_caret {
            let caret = f.caret.min(value.len());
            let cx_pos = tx + m.measure(&value[..caret], px, style).0;
            cmds.push(DrawCmd::Rect {
                rect: Rect::new(cx_pos, ty, 1.0, th),
                color: theme.text,
            });
        }
        if f.handles && !f.selection.is_empty() {
            paint_grip(cmds, tx, ty, th, value, f.selection.start, px, style, m, theme);
            paint_grip(cmds, tx, ty, th, value, f.selection.end, px, style, m, theme);
        }
    }
    cmds.push(DrawCmd::PopClip);
}

fn paint_underline(cmds: &mut Vec<DrawCmd>, x0: f32, x1: f32, y: f32, color: libra_draw::Color) {
    let y = y.floor();
    let x0 = x0.floor();
    cmds.push(DrawCmd::Rect {
        rect: Rect::new(x0, y, (x1 - x0).max(1.0), 1.0),
        color,
    });
}

fn paint_grip(
    cmds: &mut Vec<DrawCmd>,
    tx: f32,
    ty: f32,
    th: f32,
    value: &str,
    byte: usize,
    px: f32,
    style: FontStyle,
    m: &dyn TextMeasurer,
    theme: &Theme,
) {
    let byte = byte.min(value.len());
    let x = tx + m.measure(&value[..byte], px, style).0;
    let size = 6.0;
    cmds.push(DrawCmd::RRect {
        rect: Rect::new(x - size / 2.0, ty + (th - size) / 2.0, size, size),
        radius: size / 2.0,
        color: theme.accent,
    });
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
            preedit: None,
            handles: false,
            show_caret: true,
        };
        assert_eq!(
            input_scroll_x(&m, "abcd", 14.0, FontStyle::Regular, 8.0, Some(&focus)),
            0.0
        );
    }

    #[test]
    fn scroll_x_follows_caret_at_end() {
        let m = UnitWidth;
        let focus = InputFocus {
            test_id: "f".into(),
            caret: 12,
            selection: 12..12,
            preedit: None,
            handles: false,
            show_caret: true,
        };
        assert_eq!(
            input_scroll_x(&m, "abcdefghijkl", 14.0, FontStyle::Regular, 8.0, Some(&focus)),
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
            preedit: None,
            handles: false,
            show_caret: true,
        };
        assert_eq!(
            input_scroll_x(&m, "abcdefghijkl", 14.0, FontStyle::Regular, 6.0, Some(&focus)),
            4.0
        );
    }

    #[test]
    fn scroll_x_is_zero_without_focus() {
        let m = UnitWidth;
        assert_eq!(input_scroll_x(&m, "abcdefghijkl", 14.0, FontStyle::Regular, 4.0, None), 0.0);
    }
}
