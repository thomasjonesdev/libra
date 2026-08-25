//! CommonMark → [`Block`] via pulldown-cmark, then wikilink split on text runs.

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::ast::{Block, Document, Inline};
use crate::wikilink::{self, Piece};

/// Split a leading `---` YAML fence. Unknown keys stay in the yaml text.
pub fn split_frontmatter(src: &str) -> (Option<&str>, &str) {
    let rest = match src.strip_prefix("---") {
        Some(r) => r,
        None => return (None, src),
    };
    let rest = match rest.strip_prefix('\n').or_else(|| rest.strip_prefix("\r\n")) {
        Some(r) => r,
        None => return (None, src),
    };
    let close = match rest.find("\n---").or_else(|| rest.find("\r\n---")) {
        Some(c) => c,
        None => return (None, src),
    };
    let after_close = if rest[close..].starts_with("\r\n---") {
        close + "\r\n---".len()
    } else {
        close + "\n---".len()
    };
    let yaml = &rest[..close];
    let after = &rest[after_close..];
    let body = after
        .strip_prefix('\n')
        .or_else(|| after.strip_prefix("\r\n"))
        .unwrap_or(after);
    (Some(yaml), body)
}

pub fn parse(src: &str) -> Document {
    let (yaml, body) = split_frontmatter(src);
    Document {
        frontmatter: yaml.map(str::to_string),
        blocks: parse_blocks(body),
    }
}

/// Preview projection: skip frontmatter and a leading H1 that repeats `skip_title`.
pub fn parse_preview(src: &str, skip_title: &str) -> Vec<Block> {
    let mut blocks = parse(src).blocks;
    if !skip_title.is_empty() {
        if let Some(Block::Heading { level: 1, inlines }) = blocks.first() {
            if crate::ast::flatten_inlines(inlines) == skip_title {
                blocks.remove(0);
            }
        }
    }
    blocks
}

#[derive(Default)]
struct Marks {
    strong: u32,
    emphasis: u32,
    strike: u32,
    code: bool,
    link: Option<String>,
}

fn parse_blocks(src: &str) -> Vec<Block> {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_MATH);
    let parser = Parser::new_ext(src, opts);
    let mut out = Vec::new();
    let mut spans = Vec::new();
    let mut marks = Marks::default();
    let mut heading_level = 1u8;
    let mut in_code = false;
    let mut code = String::new();
    let mut code_lang = None;
    let mut in_quote = false;
    let mut list_ordered = false;
    let mut list_index = 0u32;
    let mut in_item = false;
    let mut checked = None;
    let mut in_table = false;
    let mut table_headers: Vec<Vec<Inline>> = Vec::new();
    let mut table_rows: Vec<Vec<Vec<Inline>>> = Vec::new();
    let mut table_row: Vec<Vec<Inline>> = Vec::new();

    for ev in parser {
        match ev {
            Event::Start(Tag::Heading { level, .. }) => {
                heading_level = heading_level_u8(level);
                spans.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                let inlines = take_spans(&mut spans);
                if !inlines.is_empty() {
                    out.push(Block::Heading {
                        level: heading_level,
                        inlines,
                    });
                }
            }
            Event::Start(Tag::Paragraph) => {
                if !in_item && !in_quote && !in_table {
                    spans.clear();
                }
            }
            Event::End(TagEnd::Paragraph) => {
                if in_item || in_quote || in_table {
                    continue;
                }
                let inlines = take_spans(&mut spans);
                if !inlines.is_empty() {
                    out.push(Block::Paragraph { inlines });
                }
            }
            Event::Start(Tag::Table(_)) => {
                in_table = true;
                table_headers.clear();
                table_rows.clear();
                table_row.clear();
            }
            Event::End(TagEnd::Table) => {
                out.push(Block::Table {
                    headers: std::mem::take(&mut table_headers),
                    rows: std::mem::take(&mut table_rows),
                });
                in_table = false;
            }
            Event::Start(Tag::TableHead) => table_row.clear(),
            Event::End(TagEnd::TableHead) => {
                table_headers = std::mem::take(&mut table_row);
            }
            Event::Start(Tag::TableRow) => table_row.clear(),
            Event::End(TagEnd::TableRow) => {
                table_rows.push(std::mem::take(&mut table_row));
            }
            Event::Start(Tag::TableCell) => spans.clear(),
            Event::End(TagEnd::TableCell) => table_row.push(take_spans(&mut spans)),
            Event::InlineMath(t) => spans.push(Inline::Math(t.to_string())),
            Event::DisplayMath(t) => out.push(Block::Math { text: t.to_string() }),
            Event::Start(Tag::List(start)) => {
                list_ordered = start.is_some();
                list_index = start.map(|n| n.saturating_sub(1) as u32).unwrap_or(0);
            }
            Event::End(TagEnd::List(_)) => {}
            Event::Start(Tag::Item) => {
                in_item = true;
                checked = None;
                if list_ordered {
                    list_index = list_index.saturating_add(1);
                }
                spans.clear();
            }
            Event::End(TagEnd::Item) => {
                out.push(Block::ListItem {
                    ordered: list_ordered,
                    index: list_index,
                    checked,
                    inlines: take_spans(&mut spans),
                });
                in_item = false;
                checked = None;
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                in_code = true;
                code.clear();
                code_lang = match kind {
                    CodeBlockKind::Fenced(lang) if !lang.is_empty() => Some(lang.into_string()),
                    _ => None,
                };
            }
            Event::End(TagEnd::CodeBlock) => {
                out.push(Block::Code {
                    lang: code_lang.take(),
                    text: std::mem::take(&mut code).trim_end_matches('\n').to_string(),
                });
                in_code = false;
            }
            Event::Start(Tag::BlockQuote(_)) => {
                in_quote = true;
                spans.clear();
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                let inlines = take_spans(&mut spans);
                if !inlines.is_empty() {
                    out.push(Block::Quote { inlines });
                }
                in_quote = false;
            }
            Event::Rule => out.push(Block::Rule),
            Event::Text(t) => {
                if in_code {
                    code.push_str(&t);
                } else {
                    push_text(&mut spans, &marks, &t);
                }
            }
            Event::Code(t) => {
                if in_code {
                    code.push_str(&t);
                } else {
                    let prev_code = marks.code;
                    marks.code = true;
                    push_text(&mut spans, &marks, &t);
                    marks.code = prev_code;
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if in_code {
                    code.push('\n');
                } else {
                    push_text(&mut spans, &marks, " ");
                }
            }
            Event::Start(Tag::Emphasis) => marks.emphasis += 1,
            Event::End(TagEnd::Emphasis) => marks.emphasis = marks.emphasis.saturating_sub(1),
            Event::Start(Tag::Strong) => marks.strong += 1,
            Event::End(TagEnd::Strong) => marks.strong = marks.strong.saturating_sub(1),
            Event::Start(Tag::Strikethrough) => marks.strike += 1,
            Event::End(TagEnd::Strikethrough) => marks.strike = marks.strike.saturating_sub(1),
            Event::Start(Tag::Link { dest_url, .. }) => marks.link = Some(dest_url.into_string()),
            Event::End(TagEnd::Link) => marks.link = None,
            Event::TaskListMarker(done) => checked = Some(done),
            Event::Start(_) | Event::End(_) | Event::Html(_) | Event::InlineHtml(_)
            | Event::FootnoteReference(_) => {}
        }
    }
    out
}

fn take_spans(spans: &mut Vec<Inline>) -> Vec<Inline> {
    extract_wikilinks(coalesce(std::mem::take(spans)))
}

fn extract_wikilinks(spans: Vec<Inline>) -> Vec<Inline> {
    let mut out = Vec::new();
    for span in spans {
        match span {
            Inline::Text(t) => push_split(&mut out, None, t),
            Inline::Strong(t) => push_split(&mut out, Some(Mark::Strong), t),
            Inline::Emphasis(t) => push_split(&mut out, Some(Mark::Emphasis), t),
            Inline::Strike(t) => push_split(&mut out, Some(Mark::Strike), t),
            other => out.push(other),
        }
    }
    coalesce(out)
}

enum Mark {
    Strong,
    Emphasis,
    Strike,
}

fn push_split(out: &mut Vec<Inline>, mark: Option<Mark>, raw: String) {
    for piece in wikilink::split(&raw) {
        match piece {
            Piece::Link(w) => out.push(Inline::Wikilink(w)),
            Piece::Text(t) => out.push(match mark {
                Some(Mark::Strong) => Inline::Strong(t),
                Some(Mark::Emphasis) => Inline::Emphasis(t),
                Some(Mark::Strike) => Inline::Strike(t),
                None => Inline::Text(t),
            }),
        }
    }
}

fn coalesce(spans: Vec<Inline>) -> Vec<Inline> {
    let mut out = Vec::new();
    for span in spans {
        match (out.last_mut(), &span) {
            (Some(Inline::Text(a)), Inline::Text(b)) => a.push_str(b),
            (Some(Inline::Strong(a)), Inline::Strong(b)) => a.push_str(b),
            (Some(Inline::Emphasis(a)), Inline::Emphasis(b)) => a.push_str(b),
            _ => out.push(span),
        }
    }
    out.retain(|s| !s.plain().is_empty() || matches!(s, Inline::Text(_)));
    out.retain(|s| !s.plain().is_empty());
    out
}

fn push_text(spans: &mut Vec<Inline>, marks: &Marks, raw: &str) {
    if marks.code {
        spans.push(Inline::Code(raw.to_string()));
        return;
    }
    spans.push(styled(marks, raw.to_string()));
}

fn styled(marks: &Marks, t: String) -> Inline {
    if let Some(dest) = &marks.link {
        return Inline::Link { text: t, dest: dest.clone() };
    }
    if marks.strong > 0 {
        Inline::Strong(t)
    } else if marks.strike > 0 {
        Inline::Strike(t)
    } else if marks.emphasis > 0 {
        Inline::Emphasis(t)
    } else {
        Inline::Text(t)
    }
}

fn heading_level_u8(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 | HeadingLevel::H6 => 4,
    }
}
