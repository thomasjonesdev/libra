//! `<Tabs>` / `<Tab>` — editor chrome (§7.1, PLAN-CATALOG wave 7).
//!
//! The strip paints a bottom hairline; each tab is model-owned
//! `selected` with `onPress` to activate and optional `onClose` (a
//! trailing ×, `{testId}-close`). Gloss note tabs are the first
//! consumer:
//!
//! ```lx
//! <Tabs testId="editor-tabs">
//!   <for each={tab_rows} as="t" key={t.id}>
//!     <Tab selected={t.selected} onPress={selectNote(t.id)} onClose={closeTab(t.id)}>{t.title}</Tab>
//!   </for>
//! </Tabs>
//! ```
//!
//! `+` / back / forward stay outside — they are app chrome, not tabs.
//! Tokens only. Floor the 1px hairline (tiny-skia AA hairline gotcha).

use crate::layout::{is_detached_node, BuildCx};
use crate::node::*;
use crate::paint::PaintCx;
use libra_draw::{DrawCmd, FontStyle, Rect};
use taffy::prelude::*;

pub(crate) fn build_tabs(
    cx: &mut BuildCx,
    children: &[ViewNode],
    test_id: &Option<String>,
) -> Result<NodeId, taffy::TaffyError> {
    let mut kids = Vec::new();
    for c in children {
        if is_detached_node(c) {
            let _ = cx.child(c)?;
            continue;
        }
        kids.push(cx.child(c)?);
    }
    let id = cx.tree.new_with_children(
        Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            align_items: Some(AlignItems::FlexEnd),
            gap: Size { width: length(cx.theme.space(1)), height: length(0.0) },
            size: Size { width: auto(), height: auto() },
            ..Default::default()
        },
        &kids,
    )?;
    cx.meta.push((
        id,
        ItemKind::TabsBox,
        Semantics {
            role: Role::Container,
            text: String::new(),
            test_id: test_id.clone(),
        },
        None,
    ));
    Ok(id)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_tab(
    cx: &mut BuildCx,
    label: &str,
    selected: bool,
    on_press: &Option<String>,
    on_press_arg: &Option<String>,
    on_close: &Option<String>,
    on_close_arg: &Option<String>,
    test_id: &Option<String>,
) -> Result<NodeId, taffy::TaffyError> {
    let theme = cx.theme;
    let px = theme.type_sm;
    let (tw, th) = cx.m.measure(label, px, FontStyle::Regular);
    let close_s = th;

    let label_id = cx.tree.new_leaf(Style {
        size: Size { width: length(tw.max(1.0)), height: length(th) },
        flex_shrink: 0.0,
        ..Default::default()
    })?;
    cx.meta.push((
        label_id,
        ItemKind::TabLabel { label: label.to_string(), px, selected },
        Semantics {
            role: Role::Text,
            text: label.to_string(),
            test_id: None,
        },
        None,
    ));

    let mut kids = vec![label_id];
    if let Some(close) = on_close {
        let cid = cx.tree.new_leaf(Style {
            size: Size { width: length(close_s), height: length(close_s) },
            flex_shrink: 0.0,
            ..Default::default()
        })?;
        let close_tid = test_id.as_ref().map(|t| format!("{t}-close"));
        cx.meta.push((
            cid,
            ItemKind::TabClose { selected },
            Semantics {
                role: Role::Button,
                text: "close".to_string(),
                test_id: close_tid,
            },
            Some((close.clone(), on_close_arg.clone())),
        ));
        kids.push(cid);
    }

    let parent = cx.tree.new_with_children(
        Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            align_items: Some(AlignItems::Center),
            gap: Size { width: length(theme.space(2)), height: length(0.0) },
            padding: taffy::Rect {
                left: length(theme.space(3)),
                right: length(theme.space(3)),
                top: length(theme.space(2)),
                bottom: length(theme.space(2)),
            },
            flex_shrink: 0.0,
            ..Default::default()
        },
        &kids,
    )?;
    let press = on_press.as_ref().map(|p| (p.clone(), on_press_arg.clone()));
    cx.meta.push((
        parent,
        ItemKind::TabBox { selected },
        Semantics {
            role: if press.is_some() { Role::Button } else { Role::Container },
            text: label.to_string(),
            test_id: test_id.clone(),
        },
        press,
    ));
    Ok(parent)
}

pub(crate) fn paint_tabs(cx: &mut PaintCx, item: &Item) {
    let y = (item.rect.y + item.rect.h - 1.0).floor();
    cx.cmds.push(DrawCmd::Rect {
        rect: Rect::new(item.rect.x.floor(), y, item.rect.w.max(1.0), 1.0),
        color: cx.theme.hairline,
    });
}

pub(crate) fn paint_box(cx: &mut PaintCx, item: &Item, selected: bool) {
    let theme = cx.theme;
    let hovered = cx.hover.is_some_and(|(hx, hy)| item.rect.contains(hx, hy));
    if !selected && hovered {
        cx.cmds.push(DrawCmd::RRect {
            rect: item.rect,
            radius: theme.radius_sm,
            color: theme.secondary,
        });
    }
    if selected {
        let h = 2.0;
        let y = (item.rect.y + item.rect.h - h).floor();
        cx.cmds.push(DrawCmd::Rect {
            rect: Rect::new(item.rect.x.floor(), y, item.rect.w.max(1.0), h),
            color: theme.accent,
        });
    }
}

pub(crate) fn paint_label(cx: &mut PaintCx, item: &Item, label: &str, px: f32, selected: bool) {
    let fg = if selected { cx.theme.text } else { cx.theme.text_muted };
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

pub(crate) fn paint_close(cx: &mut PaintCx, item: &Item, selected: bool) {
    let fg = if selected { cx.theme.text } else { cx.theme.text_muted };
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
