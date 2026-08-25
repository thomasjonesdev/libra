//! Markdown — display-only CommonMark (§24). Expands `source` into
//! Heading / Text / Separator children via `libra-markdown`. Mixed
//! inline spans become a wrapping row of Text runs.

use crate::layout::{box_style, BuildCx};
use crate::node::*;
use libra_draw::{Color, FontStyle};
use libra_markdown::{Block, Inline};
use taffy::prelude::*;

pub(crate) fn build(
    cx: &mut BuildCx,
    source: &str,
    skip_title: &str,
    test_id: &Option<String>,
    on_wikilink: &Option<String>,
) -> Result<NodeId, taffy::TaffyError> {
    let blocks = libra_markdown::parse_preview(source, skip_title);
    let mut kids = Vec::new();
    for block in &blocks {
        kids.push(build_block(cx, block, on_wikilink)?);
    }
    if kids.is_empty() {
        kids.push(cx.child(&ViewNode::Text {
            content: String::new(),
            size: TextSize::Md,
            bold: false,
            muted: true,
            grow: false,
            wrap: true,
            test_id: None,
        })?);
    }
    let style = BoxStyle {
        gap: cx.theme.space(3),
        ..Default::default()
    };
    let id = cx
        .tree
        .new_with_children(box_style(&style, FlexDirection::Column), &kids)?;
    cx.meta.push((
        id,
        ItemKind::Box,
        Semantics {
            role: Role::Container,
            text: libra_markdown::flatten(&blocks),
            test_id: test_id.clone(),
        },
        None,
    ));
    Ok(id)
}

fn build_block(
    cx: &mut BuildCx,
    block: &Block,
    on_wikilink: &Option<String>,
) -> Result<NodeId, taffy::TaffyError> {
    match block {
        Block::Heading { level, inlines } => {
            if is_plain(inlines) {
                cx.child(&ViewNode::Heading {
                    content: libra_markdown::flatten_inlines(inlines),
                    level: *level,
                    test_id: None,
                })
            } else {
                let px = heading_px(cx.theme, *level);
                flow(cx, inlines, px, true, false, "", on_wikilink)
            }
        }
        Block::Paragraph { inlines } => {
            if is_plain(inlines) {
                cx.child(&plain_text(
                    &libra_markdown::flatten_inlines(inlines),
                    TextSize::Md,
                    false,
                    false,
                ))
            } else {
                flow(cx, inlines, cx.theme.type_md, false, false, "", on_wikilink)
            }
        }
        Block::ListItem {
            ordered,
            index,
            checked,
            inlines,
        } => {
            let prefix = list_prefix(*ordered, *index, *checked);
            if is_plain(inlines) {
                let label = format!("{prefix}{}", libra_markdown::flatten_inlines(inlines));
                cx.child(&plain_text(&label, TextSize::Md, false, false))
            } else {
                flow(cx, inlines, cx.theme.type_md, false, false, &prefix, on_wikilink)
            }
        }
        Block::Code { text, .. } => cx.child(&plain_text(text, TextSize::Sm, false, true)),
        Block::Quote { inlines } => {
            if is_plain(inlines) {
                cx.child(&plain_text(
                    &libra_markdown::flatten_inlines(inlines),
                    TextSize::Md,
                    false,
                    true,
                ))
            } else {
                flow(cx, inlines, cx.theme.type_md, false, true, "", on_wikilink)
            }
        }
        Block::Table { headers, rows } => build_table(cx, headers, rows, on_wikilink),
        Block::Math { text } => cx.child(&plain_text(&format!("$${text}$$"), TextSize::Md, false, true)),
        Block::Rule => cx.child(&ViewNode::Separator { vertical: false }),
    }
}

fn build_table(
    cx: &mut BuildCx,
    headers: &[Vec<Inline>],
    rows: &[Vec<Vec<Inline>>],
    on_wikilink: &Option<String>,
) -> Result<NodeId, taffy::TaffyError> {
    let mut kids = Vec::new();
    if !headers.is_empty() {
        kids.push(table_row(cx, headers, true, on_wikilink)?);
    }
    for row in rows {
        kids.push(table_row(cx, row, false, on_wikilink)?);
    }
    if kids.is_empty() {
        kids.push(cx.child(&plain_text("", TextSize::Md, false, true))?);
    }
    let style = BoxStyle {
        gap: cx.theme.space(2),
        ..Default::default()
    };
    let id = cx
        .tree
        .new_with_children(box_style(&style, FlexDirection::Column), &kids)?;
    cx.meta.push((
        id,
        ItemKind::Box,
        Semantics {
            role: Role::Container,
            text: Block::Table {
                headers: headers.to_vec(),
                rows: rows.to_vec(),
            }
            .plain(),
            test_id: None,
        },
        None,
    ));
    Ok(id)
}

fn table_row(
    cx: &mut BuildCx,
    cells: &[Vec<Inline>],
    header: bool,
    on_wikilink: &Option<String>,
) -> Result<NodeId, taffy::TaffyError> {
    let mut kids = Vec::new();
    for (i, cell) in cells.iter().enumerate() {
        if i > 0 {
            kids.push(crate::catalog::text::build_run(
                cx,
                "|",
                cx.theme.type_md,
                FontStyle::Regular,
                cx.theme.text_muted,
                false,
                None,
            )?);
        }
        if is_plain(cell) {
            kids.push(cx.child(&plain_text(
                &libra_markdown::flatten_inlines(cell),
                TextSize::Md,
                header,
                false,
            ))?);
        } else {
            kids.push(flow(
                cx,
                cell,
                cx.theme.type_md,
                header,
                false,
                "",
                on_wikilink,
            )?);
        }
    }
    if kids.is_empty() {
        kids.push(cx.child(&plain_text("", TextSize::Md, header, false))?);
    }
    let gap = cx.theme.space(2);
    let style = Style {
        display: Display::Flex,
        flex_direction: FlexDirection::Row,
        flex_wrap: FlexWrap::Wrap,
        gap: Size {
            width: length(gap),
            height: length(gap),
        },
        size: Size {
            width: percent(1.0),
            height: auto(),
        },
        min_size: Size {
            width: length(0.0),
            height: auto(),
        },
        ..Default::default()
    };
    let id = cx.tree.new_with_children(style, &kids)?;
    cx.meta.push((
        id,
        ItemKind::Box,
        Semantics {
            role: Role::Container,
            text: cells
                .iter()
                .map(|c| libra_markdown::flatten_inlines(c))
                .collect::<Vec<_>>()
                .join(" | "),
            test_id: None,
        },
        None,
    ));
    Ok(id)
}

fn list_prefix(ordered: bool, index: u32, checked: Option<bool>) -> String {
    match checked {
        Some(true) => "[x] ".into(),
        Some(false) => "[ ] ".into(),
        None if ordered => format!("{index}. "),
        None => "- ".into(),
    }
}

fn is_plain(inlines: &[Inline]) -> bool {
    inlines.iter().all(|i| matches!(i, Inline::Text(_)))
}

fn plain_text(content: &str, size: TextSize, bold: bool, muted: bool) -> ViewNode {
    ViewNode::Text {
        content: content.to_string(),
        size,
        bold,
        muted,
        grow: false,
        wrap: true,
        test_id: None,
    }
}

fn heading_px(theme: &libra_draw::Theme, level: u8) -> f32 {
    match level {
        0 | 1 => theme.type_xl,
        2 => theme.type_lg,
        3 => theme.type_md,
        _ => theme.type_sm,
    }
}

fn flow(
    cx: &mut BuildCx,
    inlines: &[Inline],
    px: f32,
    heading: bool,
    muted: bool,
    prefix: &str,
    on_wikilink: &Option<String>,
) -> Result<NodeId, taffy::TaffyError> {
    let mut kids = Vec::new();
    if !prefix.is_empty() {
        kids.push(crate::catalog::text::build_run(
            cx,
            prefix.trim_end(),
            px,
            FontStyle::Regular,
            cx.theme.text_muted,
            false,
            None,
        )?);
    }
    for inline in inlines {
        let (style, color, strike) = run_style(cx, inline, heading, muted);
        let press = match (inline, on_wikilink) {
            (Inline::Wikilink(w), Some(action)) => Some((action.clone(), Some(w.target.clone()))),
            _ => None,
        };
        for word in run_words(inline) {
            kids.push(crate::catalog::text::build_run(
                cx, &word, px, style, color, strike, press.clone(),
            )?);
        }
    }
    if kids.is_empty() {
        kids.push(crate::catalog::text::build_run(
            cx,
            "",
            px,
            FontStyle::Regular,
            cx.theme.text,
            false,
            None,
        )?);
    }
    let gap = cx.theme.space(2);
    let style = Style {
        display: Display::Flex,
        flex_direction: FlexDirection::Row,
        flex_wrap: FlexWrap::Wrap,
        gap: Size {
            width: length(gap),
            height: length(gap),
        },
        size: Size {
            width: percent(1.0),
            height: auto(),
        },
        min_size: Size {
            width: length(0.0),
            height: auto(),
        },
        ..Default::default()
    };
    let id = cx.tree.new_with_children(style, &kids)?;
    cx.meta.push((
        id,
        ItemKind::Box,
        Semantics {
            role: Role::Container,
            text: format!("{prefix}{}", libra_markdown::flatten_inlines(inlines)),
            test_id: None,
        },
        None,
    ));
    Ok(id)
}

fn run_words(inline: &Inline) -> Vec<String> {
    match inline {
        Inline::Wikilink(w) => vec![w.display().to_string()],
        Inline::Link { text, .. } | Inline::Code(text) => vec![text.clone()],
        Inline::Math(tex) => vec![format!("${tex}$")],
        other => other
            .plain()
            .split_whitespace()
            .map(str::to_string)
            .collect(),
    }
}

fn run_style(cx: &BuildCx, inline: &Inline, heading: bool, muted: bool) -> (FontStyle, Color, bool) {
    let theme = cx.theme;
    match inline {
        Inline::Strong(_) => (FontStyle::Bold, theme.text, false),
        Inline::Emphasis(_) => (FontStyle::Italic, theme.text, false),
        Inline::Strike(_) => (FontStyle::Regular, theme.text, true),
        Inline::Code(_) | Inline::Math(_) => (FontStyle::Regular, theme.text_muted, false),
        Inline::Link { .. } | Inline::Wikilink(_) => (FontStyle::Regular, theme.accent, false),
        Inline::Text(_) => (
            if heading {
                FontStyle::Bold
            } else {
                FontStyle::Regular
            },
            if muted { theme.text_muted } else { theme.text },
            false,
        ),
    }
}
