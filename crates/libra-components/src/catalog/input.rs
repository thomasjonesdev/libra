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
    let focused = cx
        .focus
        .filter(|f| item.semantics.test_id.as_deref() == Some(f.test_id.as_str()));
    paint_input(cx.cmds, item.rect, value, placeholder, px, focused, cx.measurer, cx.theme);
}

/// Input painting (§10.1): field chrome, value or muted placeholder,
/// and — when focused — a focus ring, the selection band, and a caret.
/// Caret/selection x-positions come from prefix advance widths, so the
/// same measurer that laid the text positions the caret.
#[allow(clippy::too_many_arguments)]
fn paint_input(
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
        // Focus ring: a 1px accent frame (four thin rects — no stroke
        // rrect in the Phase 3 display list).
        let c = theme.accent;
        cmds.push(DrawCmd::Rect { rect: Rect::new(rect.x, rect.y, rect.w, 1.0), color: c });
        cmds.push(DrawCmd::Rect { rect: Rect::new(rect.x, rect.y + rect.h - 1.0, rect.w, 1.0), color: c });
        cmds.push(DrawCmd::Rect { rect: Rect::new(rect.x, rect.y, 1.0, rect.h), color: c });
        cmds.push(DrawCmd::Rect { rect: Rect::new(rect.x + rect.w - 1.0, rect.y, 1.0, rect.h), color: c });
    }
    let (tx, ty) = (rect.x + theme.btn_pad_x, rect.y + theme.btn_pad_y);
    let (_, th) = m.measure("Ag", px, FontStyle::Regular);

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
}
