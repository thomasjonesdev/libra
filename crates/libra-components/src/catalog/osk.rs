//! Framework on-screen keyboard (§19.1). Layouts are `text`, `numeric`,
//! `hex`, and `none`. The shell summons one on touch focus and never on
//! a pointer click. Keys are attached after layout so apps don't author
//! them; a tap applies an [`libra_text::EditOp`] instead of an action.

use crate::node::*;
use crate::paint::PaintCx;
use libra_draw::{DrawCmd, FontStyle, Rect, Theme};
use libra_text::EditOp;

pub fn key_rows(keyboard: Keyboard) -> Vec<Vec<&'static str>> {
    match keyboard {
        Keyboard::None => Vec::new(),
        Keyboard::Text => vec![
            vec!["q", "w", "e", "r", "t", "y", "u", "i", "o", "p"],
            vec!["a", "s", "d", "f", "g", "h", "j", "k", "l"],
            vec!["z", "x", "c", "v", "b", "n", "m", "⌫"],
            vec!["space"],
        ],
        Keyboard::Numeric => vec![
            vec!["1", "2", "3"],
            vec!["4", "5", "6"],
            vec!["7", "8", "9"],
            vec!["⌫", "0", "."],
        ],
        Keyboard::Hex => vec![
            vec!["0", "1", "2", "3"],
            vec!["4", "5", "6", "7"],
            vec!["8", "9", "a", "b"],
            vec!["c", "d", "e", "f"],
            vec!["⌫"],
        ],
    }
}

/// The edit a key applies. `⌫` deletes, `space` inserts a space, any
/// other one-character label inserts that character.
pub fn osk_edit(label: &str) -> Option<EditOp> {
    match label {
        "⌫" => Some(EditOp::Backspace),
        "space" => Some(EditOp::Insert(' ')),
        other => {
            let mut chars = other.chars();
            let ch = chars.next()?;
            if chars.next().is_some() {
                None
            } else {
                Some(EditOp::Insert(ch))
            }
        }
    }
}

/// Append key items along the bottom of `tree`. Called after scroll is
/// applied so the keys stay pinned to the viewport.
pub fn attach_osk(tree: &mut LaidTree, keyboard: Keyboard, theme: &Theme) {
    let rows = key_rows(keyboard);
    if rows.is_empty() || tree.width <= 0.0 || tree.height <= 0.0 {
        return;
    }
    let key_h = theme.type_md + theme.space(3);
    let gap = theme.space(1);
    let margin = theme.space(2);
    let n = rows.len() as f32;
    let band = n * key_h + (n - 1.0) * gap + margin * 2.0;
    let top = (tree.height - band).max(0.0);
    for (r, row) in rows.iter().enumerate() {
        let cols = row.len().max(1) as f32;
        let inner = (tree.width - margin * 2.0 - gap * (cols - 1.0)).max(cols);
        let key_w = inner / cols;
        let y = top + margin + r as f32 * (key_h + gap);
        for (c, label) in row.iter().enumerate() {
            let x = margin + c as f32 * (key_w + gap);
            tree.items.push(Item {
                kind: ItemKind::OskKey { label: (*label).to_string() },
                rect: Rect::new(x, y, key_w, key_h),
                semantics: Semantics {
                    role: Role::Button,
                    text: (*label).to_string(),
                    test_id: None,
                },
                on_press: Some("osk".into()),
                on_press_arg: None,
                clip: None,
                scroll: None,
                menu: None,
            });
        }
    }
}

pub(crate) fn paint(cx: &mut PaintCx, item: &Item, label: &str) {
    let theme = cx.theme;
    cx.cmds.push(DrawCmd::RRect {
        rect: item.rect,
        radius: theme.radius_sm,
        color: theme.surface_raised,
    });
    let px = theme.type_md;
    let (tw, th) = cx.measurer.measure(label, px, FontStyle::Regular);
    let x = item.rect.x + (item.rect.w - tw) / 2.0;
    let y = item.rect.y + (item.rect.h - th) / 2.0;
    cx.cmds.push(DrawCmd::Text {
        x,
        y,
        content: label.to_string(),
        px,
        style: FontStyle::Regular,
        color: theme.text,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_and_edits() {
        let text = key_rows(Keyboard::Text);
        assert!(text.iter().flatten().any(|k| *k == "q"));
        assert!(!key_rows(Keyboard::Numeric).iter().flatten().any(|k| *k == "q"));
        assert!(key_rows(Keyboard::Hex).iter().flatten().any(|k| *k == "a"));
        assert!(key_rows(Keyboard::None).is_empty());
        assert!(matches!(osk_edit("⌫"), Some(EditOp::Backspace)));
        assert!(matches!(osk_edit("space"), Some(EditOp::Insert(' '))));
        assert!(matches!(osk_edit("a"), Some(EditOp::Insert('a'))));
    }

    #[test]
    fn numeric_keys_sit_on_the_bottom_edge() {
        let theme = Theme::dark();
        let mut tree = LaidTree { items: Vec::new(), width: 200.0, height: 400.0 };
        attach_osk(&mut tree, Keyboard::Numeric, &theme);
        assert!(tree.items.iter().any(|i| i.semantics.text == "5"));
        assert!(!tree.items.iter().any(|i| i.semantics.text == "q"));
        let bottom = tree.items.iter().map(|i| i.rect.y + i.rect.h).fold(0.0, f32::max);
        assert!(bottom <= 400.0 + 0.5 && bottom > 300.0, "band bottom {bottom}");
    }
}
