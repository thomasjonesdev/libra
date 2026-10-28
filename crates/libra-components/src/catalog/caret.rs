//! Pointer → caret byte for focused text fields (§10.1).
//!
//! The editing core stays pure (`EditOp::SetCaret`). The shell and
//! harness map a click or drag through this helper, which reuses the
//! same wrap / scroll math the fields paint with.

use crate::catalog::{input, search, textarea};
use crate::node::*;
use libra_draw::Theme;

/// Nearest char-boundary byte in the text field under `(x, y)`.
/// `None` if `item` is not an Input / Search / Textarea.
pub fn caret_byte_at(
    item: &Item,
    x: f32,
    y: f32,
    measurer: &dyn TextMeasurer,
    theme: &Theme,
    focus: Option<&InputFocus>,
) -> Option<usize> {
    match &item.kind {
        ItemKind::Input { value, px, variant, bold, .. } => Some(input::caret_byte(
            value,
            item.rect,
            *px,
            *bold,
            *variant,
            0.0,
            x,
            measurer,
            theme,
            focus,
        )),
        ItemKind::Search { value, px, .. } => Some(input::caret_byte(
            value,
            item.rect,
            *px,
            false,
            InputVariant::Field,
            search::leading(theme),
            x,
            measurer,
            theme,
            focus,
        )),
        ItemKind::Textarea { value, px, markdown, .. } => {
            let focus_tuple = focus.map(|f| (f.caret, f.selection.start, f.selection.end));
            Some(textarea::caret_byte(
                value, item.rect, *px, x, y, measurer, theme, *markdown, focus_tuple,
            ))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libra_draw::{FontStyle, Rect};

    struct Fixed;

    impl TextMeasurer for Fixed {
        fn measure(&self, text: &str, _px: f32, _style: FontStyle) -> (f32, f32) {
            (text.chars().count() as f32 * 10.0, 14.0)
        }
    }

    fn theme() -> Theme {
        Theme::dark()
    }

    fn item(kind: ItemKind, rect: Rect) -> Item {
        Item {
            kind,
            rect,
            semantics: Semantics {
                role: Role::TextInput,
                text: String::new(),
                test_id: Some("t".into()),
            },
            on_press: Some("changed".into()),
            on_press_arg: None,
            clip: None,
            scroll: None,
            menu: None,
        }
    }

    #[test]
    fn textarea_click_past_the_last_line_is_end() {
        let theme = theme();
        let i = item(
            ItemKind::Textarea {
                value: "hello\nworld".into(),
                placeholder: String::new(),
                px: 14.0,
                markdown: false,
                on_wikilink: None,
            },
            Rect::new(0.0, 0.0, 200.0, 80.0),
        );
        let y = theme.btn_pad_y + 200.0;
        let byte = caret_byte_at(&i, theme.btn_pad_x + 4.0, y, &Fixed, &theme, None).unwrap();
        assert_eq!(byte, 11);
    }

    #[test]
    fn textarea_click_on_second_hard_line() {
        let theme = theme();
        let i = item(
            ItemKind::Textarea {
                value: "hello\nworld".into(),
                placeholder: String::new(),
                px: 14.0,
                markdown: false,
                on_wikilink: None,
            },
            Rect::new(0.0, 0.0, 200.0, 80.0),
        );
        let line_h = 14.0 + TEXTAREA_LEADING;
        let y = theme.btn_pad_y + line_h + 1.0;
        let x = theme.btn_pad_x + 5.0; // nearer 'w' than 'o'
        let byte = caret_byte_at(&i, x, y, &Fixed, &theme, None).unwrap();
        assert_eq!(byte, 6);
    }

    #[test]
    fn input_click_left_of_text_is_start() {
        let theme = theme();
        let i = item(
            ItemKind::Input {
                value: "abcd".into(),
                placeholder: String::new(),
                px: 14.0,
                on_submit: None,
                variant: InputVariant::Field,
                bold: false,
            },
            Rect::new(0.0, 0.0, 200.0, 32.0),
        );
        let byte = caret_byte_at(&i, 2.0, 16.0, &Fixed, &theme, None).unwrap();
        assert_eq!(byte, 0);
    }
}
