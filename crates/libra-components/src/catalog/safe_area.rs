//! `<SafeArea>` — pad by the shell's insets (PLAN-NATIVE-INTEGRATIONS N5).
//!
//! The insets are a layout input ([`crate::node::Insets`]), zero unless
//! the shell or the harness sets them. This node paints nothing.

use crate::layout::BuildCx;
use crate::node::*;
use taffy::prelude::*;

pub(crate) fn build(
    cx: &mut BuildCx,
    edges: SafeEdges,
    children: &[ViewNode],
    test_id: &Option<String>,
) -> Result<NodeId, taffy::TaffyError> {
    let insets = cx.insets;
    let pad = |on: bool, value: f32| if on { value } else { 0.0 };
    let mut kids = Vec::with_capacity(children.len());
    for child in children {
        kids.push(cx.child(child)?);
    }
    let style = Style {
        display: Display::Flex,
        flex_direction: FlexDirection::Column,
        padding: taffy::Rect {
            left: length(pad(edges.left, insets.left)),
            right: length(pad(edges.right, insets.right)),
            top: length(pad(edges.top, insets.top)),
            bottom: length(pad(edges.bottom, insets.bottom)),
        },
        size: Size { width: percent(1.0), height: auto() },
        ..Default::default()
    };
    let id = cx.tree.new_with_children(style, &kids)?;
    cx.meta.push((
        id,
        ItemKind::Box,
        Semantics {
            role: Role::Container,
            text: String::new(),
            test_id: test_id.clone(),
        },
        None,
    ));
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::layout_insets;
    use libra_draw::{FontStyle, Theme};

    struct Fixed;
    impl TextMeasurer for Fixed {
        fn measure(&self, text: &str, px: f32, _: FontStyle) -> (f32, f32) {
            (text.chars().count() as f32 * px, px)
        }
    }

    fn label() -> ViewNode {
        ViewNode::Text {
            content: "Hi".into(),
            size: TextSize::Md,
            bold: false,
            muted: false,
            grow: false,
            wrap: false,
            test_id: Some("label".into()),
        }
    }

    #[test]
    fn padding_follows_the_named_edges() {
        let theme = Theme::dark();
        let insets = Insets { top: 28.0, right: 6.0, bottom: 10.0, left: 4.0 };
        let view = ViewNode::SafeArea {
            edges: SafeEdges::ALL,
            children: vec![label()],
            test_id: Some("safe".into()),
        };
        let laid = layout_insets(&view, (200.0, 100.0), &Fixed, &theme, &insets).unwrap();
        let text = laid.items.iter().find(|i| i.semantics.test_id.as_deref() == Some("label")).unwrap();
        assert!((text.rect.y - 28.0).abs() < 0.01, "y {}", text.rect.y);
        assert!((text.rect.x - 4.0).abs() < 0.01, "x {}", text.rect.x);

        let bottom_only = ViewNode::SafeArea {
            edges: SafeEdges::parse("bottom").unwrap(),
            children: vec![label()],
            test_id: None,
        };
        let laid = layout_insets(&bottom_only, (200.0, 100.0), &Fixed, &theme, &insets).unwrap();
        let text = laid.items.iter().find(|i| i.semantics.test_id.as_deref() == Some("label")).unwrap();
        assert!(text.rect.y.abs() < 0.01, "bottom-only must not pad the top, y {}", text.rect.y);
        assert!(text.rect.x.abs() < 0.01);
    }

    #[test]
    fn zero_insets_leave_the_child_at_the_origin() {
        let theme = Theme::dark();
        let view = ViewNode::SafeArea {
            edges: SafeEdges::ALL,
            children: vec![label()],
            test_id: None,
        };
        let laid = layout_insets(&view, (200.0, 100.0), &Fixed, &theme, &Insets::ZERO).unwrap();
        let text = laid.items.iter().find(|i| i.semantics.text == "Hi").unwrap();
        assert!(text.rect.x.abs() < 0.01 && text.rect.y.abs() < 0.01);
    }

    #[test]
    fn edges_spec_rejects_unknown_tokens() {
        assert!(SafeEdges::parse("top,bottom").unwrap().top);
        assert!(!SafeEdges::parse("top,bottom").unwrap().left);
        assert!(SafeEdges::parse("notch").is_none());
        assert!(SafeEdges::parse("").is_none());
    }
}
