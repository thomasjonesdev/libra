//! `<Toast>` — an ephemeral overlay notice (§7.1, PLAN-CATALOG wave 7).
//!
//! Detached like Dialog (paints at the viewport, not in its parent),
//! but **not** modal: no scrim, clicks outside fall through. Mount via
//! `<if>`; auto-timeout is an app Timer / `<Interval>`, never a clock
//! on the paint path (Appendix A). Optional `onDismiss` is a trailing ×.
//!
//! Gloss save/undo can mount one after a write:
//!
//! ```lx
//! <if test={show_toast}>
//!   <Toast tone="success" title="Saved" onDismiss={dismissToast} testId="toast">{status}</Toast>
//! </if>
//! ```
//!
//! Tokens only. Left stripe uses the status tone (accent when neutral).

use crate::catalog;
use crate::layout::BuildCx;
use crate::node::*;
use crate::paint::PaintCx;
use libra_draw::{DrawCmd, FontStyle, Rect};
use taffy::prelude::*;

const CARD_W: f32 = 320.0;
const STRIPE: f32 = 3.0;

pub(crate) fn build(
    cx: &mut BuildCx,
    tone: Tone,
    title: &str,
    body: &str,
    on_dismiss: &Option<String>,
    test_id: &Option<String>,
) -> Result<NodeId, taffy::TaffyError> {
    let theme = cx.theme;
    let px_md = theme.type_md;
    let px_sm = theme.type_sm;
    let pad = theme.space(4);
    let gap = theme.space(2);
    let close_s = if on_dismiss.is_some() { px_sm } else { 0.0 };
    let content_w = (CARD_W - 2.0 * pad - STRIPE - if close_s > 0.0 { gap + close_s } else { 0.0 })
        .max(1.0);

    let (_, th_md) = cx.m.measure("Ag", px_md, FontStyle::Bold);
    let (_, th_sm) = cx.m.measure("Ag", px_sm, FontStyle::Regular);
    let line_gap = theme.space(1);
    let title_n = if title.is_empty() {
        0
    } else {
        catalog::text::wrap_lines(title, content_w, px_md, FontStyle::Bold, cx.m).len()
    };
    let body_n = if body.is_empty() {
        0
    } else {
        catalog::text::wrap_lines(body, content_w, px_sm, FontStyle::Regular, cx.m).len()
    };
    let mut text_h = title_n as f32 * th_md + body_n as f32 * th_sm;
    if title_n > 0 && body_n > 0 {
        text_h += line_gap;
    }
    let text_h = text_h.max(close_s).max(th_md);

    let copy = cx.tree.new_leaf(Style {
        size: Size { width: length(content_w), height: length(text_h) },
        flex_shrink: 0.0,
        ..Default::default()
    })?;
    cx.meta.push((
        copy,
        ItemKind::ToastCopy {
            title: title.to_string(),
            body: body.to_string(),
            px_md,
            px_sm,
        },
        Semantics {
            role: Role::Text,
            text: if title.is_empty() { body.to_string() } else { title.to_string() },
            test_id: None,
        },
        None,
    ));

    let mut kids = vec![copy];
    if let Some(dismiss) = on_dismiss {
        let cid = cx.tree.new_leaf(Style {
            size: Size { width: length(close_s), height: length(close_s) },
            flex_shrink: 0.0,
            ..Default::default()
        })?;
        let close_tid = test_id.as_ref().map(|t| format!("{t}-close"));
        cx.meta.push((
            cid,
            ItemKind::ToastClose,
            Semantics {
                role: Role::Button,
                text: "close".to_string(),
                test_id: close_tid,
            },
            Some((dismiss.clone(), None)),
        ));
        kids.push(cid);
    }

    let card = cx.tree.new_with_children(
        Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            align_items: Some(AlignItems::Start),
            gap: Size { width: length(gap), height: length(0.0) },
            padding: taffy::Rect {
                left: length(pad + STRIPE),
                right: length(pad),
                top: length(pad),
                bottom: length(pad),
            },
            size: Size { width: length(CARD_W), height: auto() },
            flex_shrink: 0.0,
            ..Default::default()
        },
        &kids,
    )?;
    cx.meta.push((
        card,
        ItemKind::ToastBox { tone },
        Semantics {
            role: Role::Container,
            text: if title.is_empty() { body.to_string() } else { title.to_string() },
            test_id: test_id.clone(),
        },
        None,
    ));

    let inset = theme.space(6);
    let host = cx.tree.new_with_children(
        Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            align_items: Some(AlignItems::FlexEnd),
            justify_content: Some(JustifyContent::FlexEnd),
            padding: taffy::Rect {
                left: length(inset),
                right: length(inset),
                top: length(inset),
                bottom: length(inset),
            },
            ..Default::default()
        },
        &[card],
    )?;
    cx.meta.push((
        host,
        ItemKind::ToastHost,
        Semantics { role: Role::Container, text: String::new(), test_id: None },
        None,
    ));
    cx.overlays.push(host);
    Ok(host)
}

fn stripe_color(theme: &libra_draw::Theme, tone: Tone) -> libra_draw::Color {
    match tone {
        Tone::Neutral => theme.accent,
        Tone::Danger => theme.danger_fg,
        Tone::Warning => theme.warning_fg,
        Tone::Success => theme.success_fg,
        Tone::Info => theme.info_fg,
    }
}

pub(crate) fn paint_box(cx: &mut PaintCx, item: &Item, tone: Tone) {
    let theme = cx.theme;
    cx.cmds.push(DrawCmd::RRect {
        rect: item.rect,
        radius: theme.radius_md,
        color: theme.surface_raised,
    });
    let r = item.rect;
    let stripe = Rect::new(r.x.floor(), r.y.floor(), STRIPE, r.h.max(1.0));
    cx.cmds.push(DrawCmd::Rect { rect: stripe, color: stripe_color(theme, tone) });
}

pub(crate) fn paint_copy(cx: &mut PaintCx, item: &Item, title: &str, body: &str, px_md: f32, px_sm: f32) {
    let content_w = item.rect.w.max(0.0);
    let line_gap = cx.theme.space(1);
    let mut y = item.rect.y;
    if !title.is_empty() {
        let (_, th) = cx.measurer.measure("Ag", px_md, FontStyle::Bold);
        for line in catalog::text::wrap_lines(title, content_w, px_md, FontStyle::Bold, cx.measurer) {
            cx.cmds.push(DrawCmd::Text {
                x: item.rect.x,
                y,
                content: line,
                px: px_md,
                style: FontStyle::Bold,
                color: cx.theme.text,
            });
            y += th;
        }
        if !body.is_empty() {
            y += line_gap;
        }
    }
    if !body.is_empty() {
        let (_, th) = cx.measurer.measure("Ag", px_sm, FontStyle::Regular);
        for line in catalog::text::wrap_lines(body, content_w, px_sm, FontStyle::Regular, cx.measurer) {
            cx.cmds.push(DrawCmd::Text {
                x: item.rect.x,
                y,
                content: line,
                px: px_sm,
                style: FontStyle::Regular,
                color: cx.theme.text_muted,
            });
            y += th;
        }
    }
}

pub(crate) fn paint_close(cx: &mut PaintCx, item: &Item) {
    let fg = cx.theme.text_muted;
    let r = item.rect;
    let inset = (r.w.min(r.h) * 0.28).max(2.0);
    let x0 = r.x + inset;
    let y0 = r.y + inset;
    let x1 = r.x + r.w - inset;
    let y1 = r.y + r.h - inset;
    cx.cmds.push(DrawCmd::Polyline {
        points: vec![(x0, y0), (x1, y1)],
        width: 1.5,
        color: fg,
    });
    cx.cmds.push(DrawCmd::Polyline {
        points: vec![(x1, y0), (x0, y1)],
        width: 1.5,
        color: fg,
    });
}
