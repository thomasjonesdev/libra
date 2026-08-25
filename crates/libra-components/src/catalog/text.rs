//! Text — a measured text leaf (§7.1). Single-line by default; optional
//! `wrap` soft-wraps to the laid-out width (PLAN-GALLERY controls pane).

use crate::layout::BuildCx;
use crate::node::*;
use crate::paint::PaintCx;
use libra_draw::{Color, DrawCmd, FontStyle};
use taffy::prelude::*;

/// Word-wrap `text` to at most `max_w` advance width (space-separated words).
/// A single word wider than `max_w` on its own (a URL, an ARN — no internal
/// spaces to break on) is hard-broken at the character level so it clips
/// within `max_w` instead of overflowing it.
pub(crate) fn wrap_lines(
    text: &str,
    max_w: f32,
    px: f32,
    style: FontStyle,
    m: &dyn TextMeasurer,
) -> Vec<String> {
    if max_w <= 0.0 || text.is_empty() {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let trial = if current.is_empty() {
            word.to_string()
        } else {
            format!("{current} {word}")
        };
        let (w, _) = m.measure(&trial, px, style);
        if w <= max_w {
            current = trial;
            continue;
        }
        if !current.is_empty() {
            lines.push(std::mem::take(&mut current));
        }
        let (word_w, _) = m.measure(word, px, style);
        current = if word_w <= max_w {
            word.to_string()
        } else {
            break_long_word(word, max_w, px, style, m, &mut lines)
        };
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Hard-break a single overlong token at the character level: push every
/// `max_w`-filling prefix straight into `lines`, and return the leftover
/// tail as the new accumulating line (so trailing words can still join it).
fn break_long_word(
    word: &str,
    max_w: f32,
    px: f32,
    style: FontStyle,
    m: &dyn TextMeasurer,
    lines: &mut Vec<String>,
) -> String {
    let mut chunk = String::new();
    for ch in word.chars() {
        let mut trial = chunk.clone();
        trial.push(ch);
        let (w, _) = m.measure(&trial, px, style);
        if w > max_w && !chunk.is_empty() {
            lines.push(std::mem::take(&mut chunk));
        }
        chunk.push(ch);
    }
    chunk
}

pub(crate) fn wrapped_height(
    text: &str,
    max_w: f32,
    px: f32,
    style: FontStyle,
    m: &dyn TextMeasurer,
) -> f32 {
    let lines = wrap_lines(text, max_w, px, style, m);
    let (_, lh) = m.measure("Ag", px, style);
    lines.len() as f32 * lh
}

pub(crate) fn build(
    cx: &mut BuildCx,
    content: &str,
    size: TextSize,
    bold: bool,
    muted: bool,
    grow: bool,
    wrap: bool,
    test_id: &Option<String>,
) -> Result<NodeId, taffy::TaffyError> {
    let theme = cx.theme;
    let px = size.px(theme);
    let font = if bold { FontStyle::Bold } else { FontStyle::Regular };
    let (_, lh) = cx.m.measure("Ag", px, font);
    let (width, height) = if wrap {
        (percent(1.0), length(lh))
    } else {
        let (w, h) = cx.m.measure(content, px, font);
        (length(w), length(h))
    };
    let mut leaf = Style {
        size: Size { width, height },
        flex_grow: if grow { 1.0 } else { 0.0 },
        flex_shrink: if wrap { 1.0 } else { 0.0 },
        ..Default::default()
    };
    if wrap {
        leaf.min_size = Size { width: length(0.0), height: auto() };
    }
    let id = cx.tree.new_leaf(leaf)?;
    let color = if muted { theme.text_muted } else { theme.text };
    cx.meta.push((
        id,
        ItemKind::Text {
            content: content.to_string(),
            px,
            style: font,
            color,
            wrap,
            strike: false,
        },
        Semantics { role: Role::Text, text: content.to_string(), test_id: test_id.clone() },
        None,
    ));
    Ok(id)
}

/// A single-line colored run. Used by `<Markdown>` to paint inline spans
/// (bold, italic, strike, code, links) as real Text items without a new
/// grammar element. `press` turns a wikilink into a Button with a target
/// payload.
pub(crate) fn build_run(
    cx: &mut BuildCx,
    content: &str,
    px: f32,
    style: FontStyle,
    color: Color,
    strike: bool,
    press: Option<(String, Option<String>)>,
) -> Result<NodeId, taffy::TaffyError> {
    let (w, h) = cx.m.measure(content, px, style);
    let id = cx.tree.new_leaf(Style {
        size: Size { width: length(w), height: length(h) },
        flex_shrink: 0.0,
        ..Default::default()
    })?;
    let role = if press.is_some() { Role::Button } else { Role::Text };
    cx.meta.push((
        id,
        ItemKind::Text {
            content: content.to_string(),
            px,
            style,
            color,
            wrap: false,
            strike,
        },
        Semantics { role, text: content.to_string(), test_id: None },
        press,
    ));
    Ok(id)
}

pub(crate) fn paint(
    cx: &mut PaintCx,
    item: &Item,
    content: &str,
    px: f32,
    style: FontStyle,
    color: Color,
    wrap: bool,
    strike: bool,
) {
    if !wrap || item.rect.w <= 0.0 {
        let (tw, _) = cx.measurer.measure(content, px, style);
        let clip = item.rect.w > 0.0 && tw > item.rect.w;
        if clip {
            cx.cmds.push(DrawCmd::PushClip { rect: item.rect });
        }
        paint_run(cx, item.rect.x, item.rect.y, content, px, style, color, item.rect.w, strike);
        if clip {
            cx.cmds.push(DrawCmd::PopClip);
        }
        return;
    }
    let lines = wrap_lines(content, item.rect.w, px, style, cx.measurer);
    let (_, lh) = cx.measurer.measure("Ag", px, style);
    for (i, line) in lines.iter().enumerate() {
        let (lw, _) = cx.measurer.measure(line, px, style);
        paint_run(
            cx,
            item.rect.x,
            item.rect.y + i as f32 * lh,
            line,
            px,
            style,
            color,
            lw,
            strike,
        );
    }
}

fn paint_run(
    cx: &mut PaintCx,
    x: f32,
    y: f32,
    content: &str,
    px: f32,
    style: FontStyle,
    color: Color,
    width: f32,
    strike: bool,
) {
    cx.cmds.push(DrawCmd::Text {
        x,
        y,
        content: content.to_string(),
        px,
        style,
        color,
    });
    if strike && width > 0.0 {
        let (_, h) = cx.measurer.measure("Ag", px, style);
        cx.cmds.push(DrawCmd::Rect {
            rect: libra_draw::Rect::new(x, y + (h * 0.5).round(), width.max(1.0), 1.0),
            color,
        });
    }
}
