//! Source-line markdown for styled Textarea paint (§10.1). The buffer stays
//! plain UTF-8; this module projects a hard line into display runs when the
//! caret is elsewhere (Obsidian-style live preview per line).

use crate::wikilink::{split as split_wikilinks, Piece as WikilinkPiece};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStyle {
    Text,
    Strong,
    Emphasis,
    Strike,
    Code,
    Link,
    Wikilink,
    Muted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayRun {
    pub text: String,
    pub style: RunStyle,
    /// Wikilink target (or URL for markdown links) for press handlers.
    pub link_target: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Para,
    Heading(u8),
    Bullet,
    Ordered(u32),
    Quote,
    Task(bool),
}

const TAB_WIDTH: usize = 4;

/// Visual columns consumed by leading whitespace (tabs expand to tab stops).
fn leading_whitespace_cols(line: &str) -> usize {
    let mut col = 0usize;
    for ch in line.chars() {
        match ch {
            ' ' => col += 1,
            '\t' => col += TAB_WIDTH - (col % TAB_WIDTH),
            _ => break,
        }
    }
    col
}

fn apply_block_indent(pad: usize, mut view: LineView) -> LineView {
    if pad > 0 {
        view.runs.insert(
            0,
            DisplayRun {
                text: " ".repeat(pad),
                style: RunStyle::Text,
                link_target: None,
            },
        );
    }
    view
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineView {
    pub block: BlockKind,
    pub runs: Vec<DisplayRun>,
}

/// Byte ranges of hard lines in `text` (no trailing `\n`).
pub fn hard_line_ranges(text: &str) -> Vec<(usize, usize)> {
    if text.is_empty() {
        return vec![(0, 0)];
    }
    let mut out = Vec::new();
    let mut start = 0usize;
    for (i, b) in text.bytes().enumerate() {
        if b == b'\n' {
            out.push((start, i));
            start = i + 1;
        }
    }
    out.push((start, text.len()));
    out
}

pub fn line_is_active(caret: usize, sel_start: usize, sel_end: usize, ls: usize, le: usize) -> bool {
    let (s, e) = if sel_start <= sel_end {
        (sel_start, sel_end)
    } else {
        (sel_end, sel_start)
    };
    if s != e {
        return s < le && e > ls;
    }
    caret >= ls && caret <= le
}

/// Project one hard line into styled display runs (syntax markers stripped).
pub fn line_view(line: &str) -> LineView {
    let pad = leading_whitespace_cols(line);
    apply_block_indent(pad, line_view_body(line))
}

fn line_view_body(line: &str) -> LineView {
    let trimmed = line.trim_start();
    let indent = line.len().saturating_sub(trimmed.len());

    if let Some(_rest) = trimmed.strip_prefix("# ") {
        return LineView {
            block: BlockKind::Heading(1),
            runs: inline_runs(&line[indent + 2..]),
        };
    }
    if let Some(_rest) = trimmed.strip_prefix("## ") {
        return LineView {
            block: BlockKind::Heading(2),
            runs: inline_runs(&line[indent + 3..]),
        };
    }
    if let Some(_rest) = trimmed.strip_prefix("### ") {
        return LineView {
            block: BlockKind::Heading(3),
            runs: inline_runs(&line[indent + 4..]),
        };
    }
    if let Some(_rest) = trimmed.strip_prefix("#### ") {
        return LineView {
            block: BlockKind::Heading(4),
            runs: inline_runs(&line[indent + 5..]),
        };
    }
    if let Some(_rest) = trimmed.strip_prefix("> ") {
        let body = &line[indent + 2..];
        let mut runs = inline_runs(body);
        for r in &mut runs {
            if r.style == RunStyle::Text {
                r.style = RunStyle::Muted;
            }
        }
        return LineView {
            block: BlockKind::Quote,
            runs,
        };
    }
    if let Some(_rest) = trimmed.strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("+ "))
    {
        let body = &line[indent + 2..];
        let mut runs = vec![DisplayRun {
            text: "• ".into(),
            style: RunStyle::Muted,
            link_target: None,
        }];
        runs.extend(inline_runs(body));
        return LineView {
            block: BlockKind::Bullet,
            runs,
        };
    }
    if let Some((n, rest)) = parse_ordered_prefix(trimmed) {
        let marker = format!("{n}. ");
        let body_start = indent + (trimmed.len() - rest.len());
        let mut runs = vec![DisplayRun {
            text: marker,
            style: RunStyle::Muted,
            link_target: None,
        }];
        runs.extend(inline_runs(&line[body_start..]));
        return LineView {
            block: BlockKind::Ordered(n),
            runs,
        };
    }
    if let Some(_rest) = trimmed.strip_prefix("[ ] ") {
        let body = &line[indent + 4..];
        let mut runs = vec![DisplayRun {
            text: "☐ ".into(),
            style: RunStyle::Muted,
            link_target: None,
        }];
        runs.extend(inline_runs(body));
        return LineView {
            block: BlockKind::Task(false),
            runs,
        };
    }
    if let Some(_rest) = trimmed.strip_prefix("[x] ") {
        let body = &line[indent + 4..];
        let mut runs = vec![DisplayRun {
            text: "☑ ".into(),
            style: RunStyle::Muted,
            link_target: None,
        }];
        runs.extend(inline_runs(body));
        return LineView {
            block: BlockKind::Task(true),
            runs,
        };
    }

    LineView {
        block: BlockKind::Para,
        runs: inline_runs(&line[indent..]),
    }
}

fn parse_ordered_prefix(trimmed: &str) -> Option<(u32, &str)> {
    let dot = trimmed.find(". ")?;
    let digits = &trimmed[..dot];
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let n: u32 = digits.parse().ok()?;
    let rest = trimmed[dot + 2..].trim_start();
    Some((n, rest))
}

pub fn flatten_runs(runs: &[DisplayRun]) -> String {
    runs.iter().map(|r| r.text.as_str()).collect()
}

pub fn inline_runs(src: &str) -> Vec<DisplayRun> {
    if src.is_empty() {
        return vec![DisplayRun {
            text: String::new(),
            style: RunStyle::Text,
            link_target: None,
        }];
    }
    let mut out = Vec::new();
    for piece in split_wikilinks(src) {
        match piece {
            WikilinkPiece::Text(t) => out.extend(parse_emphasis(&t)),
            WikilinkPiece::Link(w) => out.push(DisplayRun {
                text: w.display().to_string(),
                style: RunStyle::Wikilink,
                link_target: Some(w.target.clone()),
            }),
        }
    }
    if out.is_empty() {
        out.push(DisplayRun {
            text: src.to_string(),
            style: RunStyle::Text,
            link_target: None,
        });
    }
    out
}

fn parse_emphasis(mut src: &str) -> Vec<DisplayRun> {
    let mut out = Vec::new();
    while !src.is_empty() {
        if let Some((run, rest)) = take_link(src) {
            out.push(run);
            src = rest;
            continue;
        }
        if let Some((run, rest)) = take_delimited(src, "**", RunStyle::Strong) {
            out.push(run);
            src = rest;
            continue;
        }
        if let Some((run, rest)) = take_delimited(src, "~~", RunStyle::Strike) {
            out.push(run);
            src = rest;
            continue;
        }
        if let Some((run, rest)) = take_delimited(src, "*", RunStyle::Emphasis) {
            out.push(run);
            src = rest;
            continue;
        }
        if let Some((run, rest)) = take_delimited(src, "_", RunStyle::Emphasis) {
            out.push(run);
            src = rest;
            continue;
        }
        if let Some((run, rest)) = take_delimited(src, "`", RunStyle::Code) {
            out.push(run);
            src = rest;
            continue;
        }
        let next = src
            .find(['*', '_', '~', '`', '['])
            .unwrap_or(src.len());
        out.push(DisplayRun {
            text: src[..next].to_string(),
            style: RunStyle::Text,
            link_target: None,
        });
        src = &src[next..];
    }
    out
}

fn take_link(src: &str) -> Option<(DisplayRun, &str)> {
    if !src.starts_with('[') {
        return None;
    }
    let close = src.find("](")?;
    let text = &src[1..close];
    let after = &src[close + 2..];
    let url_end = after.find(')')?;
    Some((
        DisplayRun {
            text: text.to_string(),
            style: RunStyle::Link,
            link_target: Some(after[..url_end].to_string()),
        },
        &after[url_end + 1..],
    ))
}

fn take_delimited<'a>(src: &'a str, mark: &str, style: RunStyle) -> Option<(DisplayRun, &'a str)> {
    if !src.starts_with(mark) {
        return None;
    }
    let inner = &src[mark.len()..];
    let end = inner.find(mark)?;
    Some((
        DisplayRun {
            text: inner[..end].to_string(),
            style,
            link_target: None,
        },
        &inner[end + mark.len()..],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_strips_markers() {
        let v = line_view("## More");
        assert_eq!(v.block, BlockKind::Heading(2));
        assert_eq!(flatten_runs(&v.runs), "More");
    }

    #[test]
    fn inline_styles_and_wikilink() {
        let v = line_view("see **bold** and [[Atlas|the atlas]]");
        assert_eq!(
            flatten_runs(&v.runs),
            "see bold and the atlas"
        );
        assert!(v.runs.iter().any(|r| r.style == RunStyle::Strong));
        assert!(v.runs.iter().any(|r| r.style == RunStyle::Wikilink));
    }

    #[test]
    fn markdown_link_renders_label() {
        let v = line_view("[click me](https://x.test)");
        assert_eq!(flatten_runs(&v.runs), "click me");
        assert_eq!(v.runs[0].style, RunStyle::Link);
    }

    #[test]
    fn tab_indented_bullet_keeps_nesting() {
        let v = line_view("\t- nested item");
        assert_eq!(v.block, BlockKind::Bullet);
        assert!(flatten_runs(&v.runs).starts_with("    • nested"));
    }

    #[test]
    fn double_tab_indents_eight_columns() {
        let v = line_view("\t\tplain nested line");
        assert_eq!(v.block, BlockKind::Para);
        assert!(flatten_runs(&v.runs).starts_with("        plain"));
    }
}
