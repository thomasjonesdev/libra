//! VirtualList, LogView, and Table (§7.1). The view node carries the full
//! collection; this module lays out only the window around the scroll
//! offset, with leading and trailing spacers so the existing scroll
//! offset (`LaidTree::apply_scroll`) lands the window on screen.
//!
//! Row height is fixed (one text line plus button padding) so the window
//! math and the laid content height agree. Overscan is two rows.

use crate::layout::BuildCx;
use crate::node::*;
use libra_draw::{FontStyle, Theme};
use taffy::prelude::*;

const OVERSCAN: usize = 2;

pub(crate) fn row_height(m: &dyn TextMeasurer, theme: &Theme) -> f32 {
    let (_, th) = m.measure("Ag", theme.type_md, FontStyle::Regular);
    th + 2.0 * theme.btn_pad_y
}

/// `[start, end)` row indexes to materialize for `n` rows of `row_h`
/// inside a viewport of `height`, scrolled by `offset` (already clamped).
pub(crate) fn row_window(n: usize, height: f32, row_h: f32, offset: f32) -> (usize, usize) {
    if n == 0 || row_h <= 0.0 || height <= 0.0 {
        return (0, 0);
    }
    let first = (offset.max(0.0) / row_h).floor() as usize;
    let visible = ((height / row_h).ceil() as usize).max(1);
    let start = first.saturating_sub(OVERSCAN).min(n);
    let end = (first + visible + OVERSCAN).min(n);
    (start, end.max(start))
}

fn clamped_offset(scroll: &ScrollState, test_id: &str, content_h: f32, height: f32, follow: bool) -> f32 {
    let overflow = (content_h - height).max(0.0);
    if follow {
        overflow
    } else {
        scroll.get(test_id).copied().unwrap_or(0.0).clamp(0.0, overflow)
    }
}

pub(crate) fn build_list(
    cx: &mut BuildCx,
    rows: &[String],
    args: &[String],
    height: f32,
    on_press: &Option<String>,
    test_id: &str,
) -> Result<NodeId, taffy::TaffyError> {
    let row_h = row_height(cx.m, cx.theme);
    let content_h = rows.len() as f32 * row_h;
    let offset = clamped_offset(cx.scroll, test_id, content_h, height, false);
    let (start, end) = row_window(rows.len(), height, row_h, offset);
    let mut kids = Vec::new();
    push_spacer(cx, &mut kids, start as f32 * row_h)?;
    for i in start..end {
        let label = rows.get(i).map(String::as_str).unwrap_or("");
        let arg = args.get(i).cloned();
        kids.push(press_row(cx, label, row_h, on_press, arg)?);
    }
    push_spacer(cx, &mut kids, (rows.len() - end) as f32 * row_h)?;
    viewport(cx, height, test_id, false, kids)
}

pub(crate) fn build_log(
    cx: &mut BuildCx,
    lines: &[String],
    height: f32,
    follow: bool,
    test_id: &str,
) -> Result<NodeId, taffy::TaffyError> {
    let row_h = row_height(cx.m, cx.theme);
    let theme = cx.theme;
    let color = theme.text_muted;
    let px = theme.type_md;
    let content_h = lines.len() as f32 * row_h;
    let offset = clamped_offset(cx.scroll, test_id, content_h, height, follow);
    let (start, end) = row_window(lines.len(), height, row_h, offset);
    let mut kids = Vec::new();
    push_spacer(cx, &mut kids, start as f32 * row_h)?;
    for i in start..end {
        let label = lines.get(i).cloned().unwrap_or_default();
        kids.push(text_leaf(cx, &label, row_h, None, px, FontStyle::Regular, color, Role::Text, None)?);
    }
    push_spacer(cx, &mut kids, (lines.len() - end) as f32 * row_h)?;
    viewport(cx, height, test_id, follow, kids)
}

pub(crate) fn build_table(
    cx: &mut BuildCx,
    columns: &[TableColumn],
    rows: &[Vec<String>],
    args: &[String],
    height: f32,
    on_row: &Option<String>,
    on_header: &Option<String>,
    test_id: &str,
) -> Result<NodeId, taffy::TaffyError> {
    let row_h = row_height(cx.m, cx.theme);
    let header = header_row(cx, columns, row_h, on_header)?;
    let rule = crate::catalog::separator::build(cx, false)?;
    let content_h = rows.len() as f32 * row_h;
    let offset = clamped_offset(cx.scroll, test_id, content_h, height, false);
    let (start, end) = row_window(rows.len(), height, row_h, offset);
    let mut kids = Vec::new();
    push_spacer(cx, &mut kids, start as f32 * row_h)?;
    for i in start..end {
        let cells = rows.get(i).cloned().unwrap_or_default();
        let arg = args.get(i).cloned();
        kids.push(body_row(cx, columns, &cells, row_h, on_row, arg)?);
    }
    push_spacer(cx, &mut kids, (rows.len() - end) as f32 * row_h)?;
    let body = viewport(cx, height, test_id, false, kids)?;
    cx.tree.new_with_children(
        Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            size: Size { width: percent(1.0), height: auto() },
            ..Default::default()
        },
        &[header, rule, body],
    )
}

fn header_row(
    cx: &mut BuildCx,
    columns: &[TableColumn],
    row_h: f32,
    on_header: &Option<String>,
) -> Result<NodeId, taffy::TaffyError> {
    let px = cx.theme.type_md;
    let color = cx.theme.text;
    let mut kids = Vec::new();
    for col in columns {
        let press = on_header.as_ref().map(|name| (name.clone(), Some(col.key.clone())));
        let role = if press.is_some() { Role::Button } else { Role::Text };
        kids.push(text_leaf(
            cx,
            &col.title,
            row_h,
            Some(col.width),
            px,
            FontStyle::Bold,
            color,
            role,
            press,
        )?);
    }
    cx.tree.new_with_children(
        Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            size: Size { width: percent(1.0), height: length(row_h) },
            flex_shrink: 0.0,
            ..Default::default()
        },
        &kids,
    )
}

fn body_row(
    cx: &mut BuildCx,
    columns: &[TableColumn],
    cells: &[String],
    row_h: f32,
    on_row: &Option<String>,
    arg: Option<String>,
) -> Result<NodeId, taffy::TaffyError> {
    let px = cx.theme.type_md;
    let color = cx.theme.text;
    let mut kids = Vec::new();
    for (i, col) in columns.iter().enumerate() {
        let text = cells.get(i).map(String::as_str).unwrap_or("");
        kids.push(text_leaf(
            cx,
            text,
            row_h,
            Some(col.width),
            px,
            FontStyle::Regular,
            color,
            Role::Text,
            None,
        )?);
    }
    let label = cells.first().cloned().unwrap_or_default();
    let id = cx.tree.new_with_children(
        Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            size: Size { width: percent(1.0), height: length(row_h) },
            flex_shrink: 0.0,
            ..Default::default()
        },
        &kids,
    )?;
    let press = on_row.as_ref().map(|name| (name.clone(), arg));
    cx.meta.push((
        id,
        ItemKind::Box,
        Semantics {
            role: if press.is_some() { Role::Button } else { Role::Text },
            text: label,
            test_id: None,
        },
        press,
    ));
    Ok(id)
}

fn press_row(
    cx: &mut BuildCx,
    label: &str,
    row_h: f32,
    on_press: &Option<String>,
    arg: Option<String>,
) -> Result<NodeId, taffy::TaffyError> {
    let theme = cx.theme;
    let px = theme.type_md;
    if let Some(name) = on_press {
        let id = cx.tree.new_leaf(Style {
            size: Size { width: percent(1.0), height: length(row_h) },
            flex_shrink: 0.0,
            ..Default::default()
        })?;
        cx.meta.push((
            id,
            ItemKind::ListItem {
                title: label.to_string(),
                subtitle: String::new(),
                meta: String::new(),
                selected: false,
                px_md: px,
                px_sm: theme.type_sm,
                inset: 0.0,
                on_menu: None,
            },
            Semantics { role: Role::Button, text: label.to_string(), test_id: None },
            Some((name.clone(), arg)),
        ));
        Ok(id)
    } else {
        text_leaf(
            cx,
            label,
            row_h,
            None,
            px,
            FontStyle::Regular,
            theme.text,
            Role::Text,
            None,
        )
    }
}

fn text_leaf(
    cx: &mut BuildCx,
    label: &str,
    row_h: f32,
    width: Option<f32>,
    px: f32,
    style: FontStyle,
    color: libra_draw::Color,
    role: Role,
    press: Option<(String, Option<String>)>,
) -> Result<NodeId, taffy::TaffyError> {
    let id = cx.tree.new_leaf(Style {
        size: Size {
            width: width.map(length).unwrap_or_else(|| percent(1.0)),
            height: length(row_h),
        },
        flex_shrink: 0.0,
        ..Default::default()
    })?;
    cx.meta.push((
        id,
        ItemKind::Text {
            content: label.to_string(),
            px,
            style,
            color,
            wrap: false,
            strike: false,
        },
        Semantics { role, text: label.to_string(), test_id: None },
        press,
    ));
    Ok(id)
}

fn push_spacer(cx: &mut BuildCx, kids: &mut Vec<NodeId>, h: f32) -> Result<(), taffy::TaffyError> {
    if h <= 0.0 {
        return Ok(());
    }
    let id = cx.tree.new_leaf(Style {
        size: Size { width: percent(1.0), height: length(h) },
        flex_shrink: 0.0,
        ..Default::default()
    })?;
    cx.meta.push((
        id,
        ItemKind::Box,
        Semantics { role: Role::Container, text: String::new(), test_id: None },
        None,
    ));
    kids.push(id);
    Ok(())
}

fn viewport(
    cx: &mut BuildCx,
    height: f32,
    test_id: &str,
    follow: bool,
    kids: Vec<NodeId>,
) -> Result<NodeId, taffy::TaffyError> {
    let content = cx.tree.new_with_children(
        Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            flex_shrink: 0.0,
            size: Size { width: percent(1.0), height: auto() },
            ..Default::default()
        },
        &kids,
    )?;
    let id = cx.tree.new_with_children(
        Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            size: Size { width: percent(1.0), height: length(height) },
            min_size: Size { width: auto(), height: length(0.0) },
            flex_shrink: 1.0,
            overflow: taffy::geometry::Point {
                x: taffy::style::Overflow::Visible,
                y: taffy::style::Overflow::Hidden,
            },
            ..Default::default()
        },
        &[content],
    )?;
    cx.meta.push((
        id,
        ItemKind::VirtualViewport { follow },
        Semantics { role: Role::Container, text: String::new(), test_id: Some(test_id.to_string()) },
        None,
    ));
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::Insets;
    use libra_draw::Theme;

    struct Fixed;
    impl TextMeasurer for Fixed {
        fn measure(&self, text: &str, px: f32, _: FontStyle) -> (f32, f32) {
            (text.chars().count() as f32 * px, px)
        }
    }

    #[test]
    fn window_is_a_slice_around_the_offset() {
        let (start, end) = row_window(100, 80.0, 20.0, 400.0);
        assert!(start > 0 && end < 100, "window {start}..{end} should not be the whole list");
        assert!(end - start < 20);
        assert!(start <= 20 && end > 20, "offset 400 / row 20 starts at row 20, got {start}..{end}");
    }

    #[test]
    fn a_hundred_rows_lay_out_as_a_window() {
        let theme = Theme::dark();
        let rows: Vec<String> = (0..100).map(|i| format!("r{i}")).collect();
        let view = ViewNode::VirtualList {
            rows: rows.clone(),
            args: rows,
            height: 80.0,
            on_press: Some("pick".into()),
            test_id: "list".into(),
        };
        let mut scroll = ScrollState::new();
        scroll.insert("list".into(), 400.0);
        let laid = crate::layout::layout_scrolled(&view, (200.0, 400.0), &Fixed, &theme, &scroll, &Insets::ZERO)
            .expect("layout");
        let mut laid = laid;
        laid.apply_scroll(&scroll);
        let titles: Vec<_> = laid
            .items
            .iter()
            .filter_map(|i| match &i.kind {
                ItemKind::ListItem { title, .. } => Some(title.clone()),
                _ => None,
            })
            .collect();
        let row_h = row_height(&Fixed, &theme);
        let offset = 400.0_f32.min((100.0 * row_h - 80.0).max(0.0));
        let (start, end) = row_window(100, 80.0, row_h, offset);
        let expect: Vec<_> = (start..end).map(|i| format!("r{i}")).collect();
        assert_eq!(titles, expect);
        assert!(titles.len() < 20, "laid {} rows", titles.len());
        assert!(!titles.iter().any(|t| t == "r0" || t == "r99"));
    }

    #[test]
    fn follow_pins_the_last_line_in_the_viewport() {
        let theme = Theme::dark();
        let lines: Vec<String> = (0..40).map(|i| format!("L{i}")).collect();
        let view = ViewNode::LogView {
            lines,
            height: 60.0,
            follow: true,
            test_id: "log".into(),
        };
        let scroll = ScrollState::new();
        let mut laid =
            crate::layout::layout_scrolled(&view, (200.0, 200.0), &Fixed, &theme, &scroll, &Insets::ZERO).expect("layout");
        laid.apply_scroll(&scroll);
        let vp = laid
            .items
            .iter()
            .find(|i| i.semantics.test_id.as_deref() == Some("log"))
            .expect("viewport")
            .rect;
        let last = laid
            .items
            .iter()
            .find(|i| i.semantics.text == "L39")
            .expect("tail line");
        assert!(
            last.rect.y >= vp.y - 1.0 && last.rect.y + last.rect.h <= vp.y + vp.h + 1.0,
            "tail {:?} outside viewport {:?}",
            last.rect,
            vp
        );
    }
}
