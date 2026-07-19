//! Badge — a small status pill (§7.1, PLAN-CATALOG wave 2). Non-interactive:
//! child text is the label, `tone` selects a semantic `theme.<tone>_bg`/`_fg`
//! token pair (PLAN-TOKENS T2). Type `type_xs`, pill radius, `space`-scaled
//! padding — all density-aware, all tokens (no raw colors — C3 guard).

use crate::layout::BuildCx;
use crate::node::*;
use crate::paint::PaintCx;
use libra_draw::{Color, DrawCmd, FontStyle, Theme};
use taffy::prelude::*;

pub(crate) fn build(
    cx: &mut BuildCx,
    label: &str,
    tone: Tone,
    test_id: &Option<String>,
) -> Result<NodeId, taffy::TaffyError> {
    let theme = cx.theme;
    let px = theme.type_xs;
    let (tw, th) = cx.m.measure(label, px, FontStyle::Regular);
    let (w, h) = (tw + 2.0 * theme.space(2), th + 2.0 * theme.space(1));
    let id = cx.tree.new_leaf(Style {
        size: Size { width: length(w), height: length(h) },
        flex_shrink: 0.0,
        ..Default::default()
    })?;
    cx.meta.push((
        id,
        ItemKind::Badge { label: label.to_string(), tone, px },
        Semantics { role: Role::Text, text: label.to_string(), test_id: test_id.clone() },
        None,
    ));
    Ok(id)
}

pub(crate) fn paint(cx: &mut PaintCx, item: &Item, label: &str, tone: Tone, px: f32) {
    let theme = cx.theme;
    let (bg, fg) = tone_colors(theme, tone);
    // Pill: full radius clamped to half the short side (independent of the
    // rasterizer's own clamping).
    let radius = theme.radius_full.min(item.rect.h / 2.0);
    cx.cmds.push(DrawCmd::RRect { rect: item.rect, radius, color: bg });
    let (tw, th) = cx.measurer.measure(label, px, FontStyle::Regular);
    cx.cmds.push(DrawCmd::Text {
        x: item.rect.x + (item.rect.w - tw) / 2.0,
        y: item.rect.y + (item.rect.h - th) / 2.0,
        content: label.to_string(),
        px,
        style: FontStyle::Regular,
        color: fg,
    });
}

/// Map a tone to its `(background, foreground)` token pair.
fn tone_colors(theme: &Theme, tone: Tone) -> (Color, Color) {
    match tone {
        Tone::Neutral => (theme.surface_raised, theme.text_muted),
        Tone::Danger => (theme.danger_bg, theme.danger_fg),
        Tone::Warning => (theme.warning_bg, theme.warning_fg),
        Tone::Success => (theme.success_bg, theme.success_fg),
        Tone::Info => (theme.info_bg, theme.info_fg),
    }
}
