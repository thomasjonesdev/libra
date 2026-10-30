//! Styled markdown paint for `<Textarea markdown>` (§10.1). Inactive hard
//! lines render as preview runs; the active line stays source-accurate.

use crate::node::{Item, ItemKind, Role, Semantics, TextMeasurer, TEXTAREA_LEADING};
use libra_draw::{Color, DrawCmd, FontStyle, Rect, Theme};
use libra_markdown::{
    hard_line_ranges, line_is_active, line_view, BlockKind, DisplayRun, RunStyle,
};

use super::textarea::{fit_prefix, wrap_hard_line};

/// Hard lines longer than this skip styled preview during layout metrics.
const LONG_HARD_LINE: usize = 4096;
/// Cap soft-wrap rows per hard line so one pathological line cannot freeze UI.
const MAX_VISUAL_ROWS_PER_HARD_LINE: usize = 256;

#[derive(Clone)]
struct StyledPiece {
    text: String,
    px: f32,
    style: FontStyle,
    color: Color,
    strike: bool,
    link_target: Option<String>,
}

#[derive(Clone)]
struct VisualRow {
    source_start: usize,
    source_end: usize,
    line_px: f32,
    pieces: Vec<StyledPiece>,
}

pub(crate) fn wikilink_hits(
    parent: &Item,
    value: &str,
    px: f32,
    m: &dyn TextMeasurer,
    theme: &Theme,
    action: &str,
) -> Vec<Item> {
    let max_w = (parent.rect.w - 2.0 * theme.btn_pad_x).max(0.0);
    if max_w <= 0.0 {
        return Vec::new();
    }
    let tx = parent.rect.x + theme.btn_pad_x;
    let ty = parent.rect.y + theme.btn_pad_y;
    let rows = visual_rows(value, max_w, px, m, theme, None);
    let mut y = ty;
    let mut out = Vec::new();
    for row in rows {
        let lh = row.line_px + TEXTAREA_LEADING;
        let mut x = tx;
        for piece in &row.pieces {
            if piece.text.is_empty() {
                continue;
            }
            let w = m.measure(&piece.text, piece.px, piece.style).0;
            if piece.link_target.is_some() {
                out.push(Item {
                    kind: ItemKind::LinkTarget,
                    rect: Rect::new(x, y, w.max(1.0), row.line_px.max(1.0)),
                    semantics: Semantics {
                        role: Role::Button,
                        text: piece.text.clone(),
                        test_id: None,
                    },
                    on_press: Some(action.to_string()),
                    on_press_arg: piece.link_target.clone(),
                    clip: parent.clip,
                    scroll: parent.scroll.clone(),
                    menu: parent.menu.clone(),
                });
            }
            x += w;
        }
        y += lh;
    }
    out
}

pub(crate) fn wrapped_height(
    value: &str,
    max_w: f32,
    px: f32,
    m: &dyn TextMeasurer,
    theme: &Theme,
    focus: Option<(usize, usize, usize)>,
) -> f32 {
    let rows = visual_rows(value, max_w, px, m, theme, focus);
    let line_h_base = m.measure("Ag", px, FontStyle::Regular).1 + TEXTAREA_LEADING;
    let h: f32 = rows
        .iter()
        .map(|r| r.line_px + TEXTAREA_LEADING)
        .sum::<f32>()
        .max(3.0 * line_h_base);
    h + 2.0 * theme.btn_pad_y
}

pub(crate) fn caret_byte(
    value: &str,
    tx: f32,
    ty: f32,
    px: f32,
    max_w: f32,
    x: f32,
    y: f32,
    m: &dyn TextMeasurer,
    theme: &Theme,
    focus: Option<(usize, usize, usize)>,
) -> usize {
    if value.is_empty() {
        return 0;
    }
    let rows = visual_rows(value, max_w, px, m, theme, focus);
    if y < ty {
        return rows.first().map(|r| r.source_start).unwrap_or(0);
    }
    let mut row_y = ty;
    for row in &rows {
        let lh = row.line_px + TEXTAREA_LEADING;
        if y < row_y + lh {
            let rel = x - tx;
            if rel <= 0.0 {
                return row.source_start;
            }
            let flat_len: usize = row.pieces.iter().map(|p| p.text.len()).sum();
            let mut xacc = 0.0f32;
            let mut flat_off = 0usize;
            for piece in &row.pieces {
                let pw = m.measure(&piece.text, piece.px, piece.style).0;
                if xacc + pw >= rel {
                    flat_off += piece.text.len();
                    break;
                }
                xacc += pw;
                flat_off += piece.text.len();
            }
            let line_len = row.source_end.saturating_sub(row.source_start);
            if flat_len == 0 || line_len == 0 {
                return row.source_start;
            }
            let src_off = (flat_off * line_len / flat_len).min(line_len);
            return row.source_start + src_off;
        }
        row_y += lh;
    }
    value.len()
}

pub(crate) fn paint_rows(
    cmds: &mut Vec<DrawCmd>,
    tx: f32,
    ty: f32,
    max_w: f32,
    value: &str,
    px: f32,
    m: &dyn TextMeasurer,
    theme: &Theme,
    focus: Option<(usize, usize, usize)>,
) {
    let rows = visual_rows(value, max_w, px, m, theme, focus);
    let mut y = ty;
    for row in rows {
        let lh = row.line_px + TEXTAREA_LEADING;
        if let Some((caret, sel_start, sel_end)) = focus {
            let (s, e) = if sel_start <= sel_end {
                (sel_start, sel_end)
            } else {
                (sel_end, sel_start)
            };
            if s < e && s < row.source_end && e > row.source_start {
                paint_selection(
                    cmds, tx, y, value, px, m, theme, &row, s, e,
                );
            }
            if caret >= row.source_start && caret <= row.source_end {
                paint_caret(cmds, tx, y, value, px, m, theme, &row, caret);
            }
        }
        paint_pieces(cmds, tx, y, &row.pieces, m);
        y += lh;
    }
}

fn visual_rows(
    value: &str,
    max_w: f32,
    px: f32,
    m: &dyn TextMeasurer,
    theme: &Theme,
    focus: Option<(usize, usize, usize)>,
) -> Vec<VisualRow> {
    let (caret, sel_start, sel_end) = focus.unwrap_or((usize::MAX, 0, 0));
    let mut out = Vec::new();
    for (ls, le) in hard_line_ranges(value) {
        let line = &value[ls..le];
        let active = line_is_active(caret, sel_start, sel_end, ls, le);
        if active || max_w <= 0.0 || le.saturating_sub(ls) > LONG_HARD_LINE {
            wrap_source_line(value, ls, le, max_w, px, m, theme, &mut out);
        } else {
            wrap_preview_line(line, ls, le, max_w, px, m, theme, &mut out);
        }
    }
    if out.is_empty() {
        out.push(VisualRow {
            source_start: 0,
            source_end: 0,
            line_px: m.measure("Ag", px, FontStyle::Regular).1,
            pieces: vec![],
        });
    }
    out
}

fn wrap_source_line(
    value: &str,
    ls: usize,
    le: usize,
    max_w: f32,
    px: f32,
    m: &dyn TextMeasurer,
    theme: &Theme,
    out: &mut Vec<VisualRow>,
) {
    let mut ranges = Vec::new();
    wrap_hard_line(value, ls, le, max_w, px, m, &mut ranges);
    if ranges.len() > MAX_VISUAL_ROWS_PER_HARD_LINE {
        ranges.truncate(MAX_VISUAL_ROWS_PER_HARD_LINE);
    }
    let lh = m.measure("Ag", px, FontStyle::Regular).1;
    for (vs, ve) in ranges {
        out.push(VisualRow {
            source_start: vs,
            source_end: ve,
            line_px: lh,
            pieces: vec![StyledPiece {
                text: value[vs..ve].to_string(),
                px,
                style: FontStyle::Regular,
                color: theme.text,
                strike: false,
                link_target: None,
            }],
        });
    }
}

fn wrap_preview_line(
    line: &str,
    ls: usize,
    le: usize,
    max_w: f32,
    px: f32,
    m: &dyn TextMeasurer,
    theme: &Theme,
    out: &mut Vec<VisualRow>,
) {
    let view = line_view(line);
    let (line_px, bold) = block_style(theme, view.block, px);
    let mut pieces: Vec<StyledPiece> = view
        .runs
        .iter()
        .map(|r| styled_run(r, line_px, bold, theme))
        .collect();
    if pieces.is_empty() {
        pieces.push(StyledPiece {
            text: String::new(),
            px: line_px,
            style: if bold {
                FontStyle::Bold
            } else {
                FontStyle::Regular
            },
            color: theme.text,
            strike: false,
            link_target: None,
        });
    }
    wrap_pieces(pieces, ls, le, max_w, line_px, m, out);
}

fn wrap_pieces(
    pieces: Vec<StyledPiece>,
    ls: usize,
    le: usize,
    max_w: f32,
    line_px: f32,
    m: &dyn TextMeasurer,
    out: &mut Vec<VisualRow>,
) {
    let flat: String = pieces.iter().map(|p| p.text.as_str()).collect();
    if flat.is_empty() {
        out.push(VisualRow {
            source_start: ls,
            source_end: le,
            line_px,
            pieces: vec![],
        });
        return;
    }
    let mut vis_start = 0usize;
    let mut rows_for_line = 0usize;
    while vis_start < flat.len() && rows_for_line < MAX_VISUAL_ROWS_PER_HARD_LINE {
        let (cut, next) = fit_prefix(&flat, vis_start, flat.len(), max_w, line_px, m);
        let mut row_pieces = Vec::new();
        let mut pos = 0usize;
        for piece in &pieces {
            let p_end = pos + piece.text.len();
            if p_end <= vis_start {
                pos = p_end;
                continue;
            }
            if pos >= cut {
                break;
            }
            let start = vis_start.saturating_sub(pos);
            let end = (cut - pos).min(piece.text.len());
            if start < end {
                row_pieces.push(StyledPiece {
                    text: piece.text[start..end].to_string(),
                    px: piece.px,
                    style: piece.style,
                    color: piece.color,
                    strike: piece.strike,
                    link_target: piece.link_target.clone(),
                });
            }
            pos = p_end;
        }
        out.push(VisualRow {
            source_start: ls,
            source_end: le,
            line_px,
            pieces: row_pieces,
        });
        rows_for_line += 1;
        if next <= vis_start {
            break;
        }
        vis_start = next;
    }
}

fn block_style(theme: &Theme, block: BlockKind, px: f32) -> (f32, bool) {
    let lh = match block {
        BlockKind::Heading(1) => theme.type_xl,
        BlockKind::Heading(2) => theme.type_lg,
        BlockKind::Heading(3) => theme.type_md,
        BlockKind::Heading(4) => theme.type_sm,
        _ => px,
    };
    let bold = matches!(block, BlockKind::Heading(_));
    (lh, bold)
}

fn styled_run(run: &DisplayRun, line_px: f32, heading_bold: bool, theme: &Theme) -> StyledPiece {
    let px = match run.style {
        RunStyle::Code => theme.type_sm,
        _ => line_px,
    };
    let style = match run.style {
        RunStyle::Strong | RunStyle::Wikilink | RunStyle::Link if heading_bold => FontStyle::Bold,
        RunStyle::Strong => FontStyle::Bold,
        RunStyle::Emphasis => FontStyle::Italic,
        RunStyle::Code => FontStyle::Regular,
        RunStyle::Wikilink | RunStyle::Link => FontStyle::Regular,
        RunStyle::Strike => FontStyle::Regular,
        RunStyle::Muted | RunStyle::Text if heading_bold => FontStyle::Bold,
        _ => FontStyle::Regular,
    };
    let color = match run.style {
        RunStyle::Muted => theme.text_muted,
        RunStyle::Code => theme.text_muted,
        RunStyle::Link | RunStyle::Wikilink => theme.accent,
        _ => theme.text,
    };
    StyledPiece {
        text: run.text.clone(),
        px,
        style,
        color,
        strike: run.style == RunStyle::Strike,
        link_target: run.link_target.clone(),
    }
}

fn paint_pieces(cmds: &mut Vec<DrawCmd>, tx: f32, y: f32, pieces: &[StyledPiece], m: &dyn TextMeasurer) {
    let mut x = tx;
    for piece in pieces {
        if piece.text.is_empty() {
            continue;
        }
        cmds.push(DrawCmd::Text {
            x,
            y,
            content: piece.text.clone(),
            px: piece.px,
            style: piece.style,
            color: piece.color,
        });
        x += m.measure(&piece.text, piece.px, piece.style).0;
    }
}

fn paint_selection(
    cmds: &mut Vec<DrawCmd>,
    tx: f32,
    y: f32,
    value: &str,
    px: f32,
    m: &dyn TextMeasurer,
    theme: &Theme,
    row: &VisualRow,
    sel_start: usize,
    sel_end: usize,
) {
    let s = sel_start.max(row.source_start).min(row.source_end);
    let e = sel_end.max(row.source_start).min(row.source_end);
    if s >= e {
        return;
    }
    let x0 = tx + m.measure(&value[row.source_start..s], px, FontStyle::Regular).0;
    let x1 = tx + m.measure(&value[row.source_start..e], px, FontStyle::Regular).0;
    let th = m.measure("Ag", px, FontStyle::Regular).1;
    cmds.push(DrawCmd::Rect {
        rect: libra_draw::Rect::new(x0, y, (x1 - x0).max(1.0), th),
        color: theme.selection,
    });
}

fn paint_caret(
    cmds: &mut Vec<DrawCmd>,
    tx: f32,
    y: f32,
    value: &str,
    px: f32,
    m: &dyn TextMeasurer,
    theme: &Theme,
    row: &VisualRow,
    caret: usize,
) {
    let c = caret.max(row.source_start).min(row.source_end);
    let cx = tx + m.measure(&value[row.source_start..c], px, FontStyle::Regular).0;
    let th = m.measure("Ag", px, FontStyle::Regular).1;
    cmds.push(DrawCmd::Rect {
        rect: libra_draw::Rect::new(cx, y, 1.0, th),
        color: theme.text,
    });
}

/// Visual-row Up/Down for a markdown textarea. X is measured the same
/// way [`paint_caret`] places the bar: source bytes, regular face.
pub(crate) fn vertical_caret(
    value: &str,
    max_w: f32,
    px: f32,
    caret: usize,
    dir: i32,
    sticky_x: Option<f32>,
    m: &dyn TextMeasurer,
    theme: &Theme,
) -> (usize, Option<f32>) {
    if value.is_empty() || dir == 0 {
        return (caret.min(value.len()), sticky_x);
    }
    let rows = visual_rows(value, max_w, px, m, theme, None);
    if rows.is_empty() {
        return (0, sticky_x);
    }
    let caret = caret.min(value.len());
    let mut row_i = rows.len() - 1;
    for (i, r) in rows.iter().enumerate() {
        let soft = caret == r.source_end
            && rows.get(i + 1).is_some_and(|n| n.source_start == r.source_end);
        if soft {
            continue;
        }
        if caret >= r.source_start && caret <= r.source_end {
            row_i = i;
            break;
        }
    }
    let row = &rows[row_i];
    let end = caret.clamp(row.source_start, row.source_end);
    let x = sticky_x.unwrap_or_else(|| {
        m.measure(&value[row.source_start..end], px, FontStyle::Regular).0
    });
    let next = row_i as i32 + dir;
    if next < 0 {
        return (rows[0].source_start, Some(x));
    }
    if next as usize >= rows.len() {
        return (value.len(), Some(x));
    }
    let target = &rows[next as usize];
    (byte_at_x(value, target.source_start, target.source_end, x, px, m), Some(x))
}

fn byte_at_x(
    value: &str,
    ls: usize,
    le: usize,
    rel: f32,
    px: f32,
    m: &dyn TextMeasurer,
) -> usize {
    if rel <= 0.0 {
        return ls;
    }
    let mut prev_i = ls;
    let mut prev_w = 0.0f32;
    let mut i = ls;
    while i < le {
        let ch = value[i..].chars().next().unwrap();
        i += ch.len_utf8();
        let w = m.measure(&value[ls..i], px, FontStyle::Regular).0;
        if w >= rel {
            return if (rel - prev_w) <= (w - rel) { prev_i } else { i };
        }
        prev_i = i;
        prev_w = w;
    }
    le
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::Keyboard;
    use libra_draw::FontStyle;

    struct Fixed;

    impl TextMeasurer for Fixed {
        fn measure(&self, text: &str, _px: f32, _style: FontStyle) -> (f32, f32) {
            (text.chars().count() as f32 * 8.0, 16.0)
        }
    }

    #[test]
    fn preview_heading_is_taller_than_body() {
        let theme = Theme::default();
        let plain = wrapped_height("hello", 400.0, 14.0, &Fixed, &theme, None);
        let heading = wrapped_height("# Title", 400.0, 14.0, &Fixed, &theme, None);
        assert!(heading >= plain);
    }

    #[test]
    fn long_hard_line_layout_completes_quickly() {
        let theme = Theme::default();
        let long = "x".repeat(200_000);
        let body = format!("{long}\n");
        let start = std::time::Instant::now();
        let h = wrapped_height(&body, 400.0, 14.0, &Fixed, &theme, None);
        assert!(h > 0.0);
        assert!(
            start.elapsed() < std::time::Duration::from_secs(2),
            "layout took {:?}",
            start.elapsed()
        );
    }

    #[test]
    fn many_wikilinks_produce_bounded_hit_items() {
        let theme = Theme::default();
        let body: String = (0..2000)
            .map(|i| format!("see [[N{i}|link {i}]]\n"))
            .collect();
        let parent = Item {
            kind: ItemKind::Textarea {
                value: body.clone(),
                placeholder: String::new(),
                px: 14.0,
                markdown: true,
                on_wikilink: Some("followWikilink".into()),
                keyboard: Keyboard::Text,
            },
            rect: Rect::new(0.0, 0.0, 800.0, 600.0),
            semantics: Semantics {
                role: Role::TextInput,
                text: body,
                test_id: Some("reader-body".into()),
            },
            on_press: None,
            on_press_arg: None,
            clip: None,
            scroll: None,
            menu: None,
        };
        let start = std::time::Instant::now();
        let hits = wikilink_hits(&parent, &parent.semantics.text, 14.0, &Fixed, &theme, "followWikilink");
        assert!(!hits.is_empty());
        assert!(hits.len() <= 2000);
        assert!(hits.iter().all(|i| matches!(i.kind, ItemKind::LinkTarget)));
        assert!(
            start.elapsed() < std::time::Duration::from_secs(2),
            "wikilink_hits took {:?} for {} hits",
            start.elapsed(),
            hits.len()
        );
    }
}
