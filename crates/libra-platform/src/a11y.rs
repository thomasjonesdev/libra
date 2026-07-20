//! Accessibility (ARCHITECTURE.md §17): the component tree's `Semantics`
//! — the same tree component-test queries resolve against — exported to
//! assistive technology through AccessKit. One tree, two consumers:
//! that's the §17 contract, now literal.
//!
//! Screen-reader **actions** route back through the loop: an AccessKit
//! Click on a Button dispatches the same action a pointer click would,
//! via the effects sink (main-thread delivery through the event loop).

use libra_components::{LaidTree, Role};

/// The flat semantic snapshot the adapter serves: rebuilt on every
/// relayout, cloned into TreeUpdates on demand.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct A11ySnapshot {
    /// (label/value text, role, logical bounds, onPress name)
    pub items: Vec<A11yItem>,
    pub scale: f64,
    /// Focused Input's item index, if any.
    pub focus: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct A11yItem {
    pub role: Role,
    pub text: String,
    pub bounds: (f32, f32, f32, f32),
    pub action: Option<String>,
}

pub const ROOT_ID: u64 = 0;

pub fn snapshot(laid: &LaidTree, scale: f64, focused_test_id: Option<&str>) -> A11ySnapshot {
    let mut items = Vec::with_capacity(laid.items.len());
    let mut focus = None;
    for item in &laid.items {
        if item.semantics.role == Role::Container {
            continue; // transparent boxes carry no semantics yet
        }
        if item.semantics.role == Role::Source {
            continue; // §12.5: source elements are test-query-only, excluded from AccessKit
        }
        if item.semantics.role == Role::TextInput
            && item.semantics.test_id.as_deref() == focused_test_id
        {
            focus = Some(items.len());
        }
        items.push(A11yItem {
            role: item.semantics.role,
            text: item.semantics.text.clone(),
            bounds: (item.rect.x, item.rect.y, item.rect.w, item.rect.h),
            action: item.on_press.clone(),
        });
    }
    A11ySnapshot { items, scale, focus }
}

pub fn tree_update(snap: &A11ySnapshot) -> accesskit::TreeUpdate {
    use accesskit::{Action, Node, NodeId, Rect, Role as AkRole, Tree, TreeUpdate};

    let mut nodes = Vec::with_capacity(snap.items.len() + 1);
    let mut child_ids = Vec::with_capacity(snap.items.len());
    for (i, item) in snap.items.iter().enumerate() {
        let id = NodeId(i as u64 + 1);
        let mut node = Node::new(match item.role {
            Role::Container => AkRole::GenericContainer,
            Role::Text => AkRole::Label,
            Role::Button => AkRole::Button,
            Role::Chart => AkRole::Image, // closest stable mapping for a plot
            Role::TextInput => AkRole::TextInput,
            Role::Separator => AkRole::GenericContainer, // visual rule; non-interactive
            Role::Dialog => AkRole::Dialog,
            Role::Menu => AkRole::Menu,
            Role::Switch => AkRole::Switch,
            Role::Checkbox => AkRole::CheckBox,
            Role::Radio => AkRole::RadioButton,
            // Unreachable: snapshot() already filters Role::Source out
            // (§12.5 — source elements never reach the AccessKit
            // projection). Matched here only so this stays exhaustive.
            Role::Source => AkRole::GenericContainer,
        });
        match item.role {
            Role::TextInput => {
                node.set_value(item.text.clone());
                node.add_action(Action::Focus);
            }
            _ => node.set_label(item.text.clone()),
        }
        if item.action.is_some()
            && matches!(item.role, Role::Button | Role::Switch | Role::Checkbox | Role::Radio)
        {
            node.add_action(Action::Click);
        }
        let (x, y, w, h) = item.bounds;
        node.set_bounds(Rect {
            x0: x as f64,
            y0: y as f64,
            x1: (x + w) as f64,
            y1: (y + h) as f64,
        });
        nodes.push((id, node));
        child_ids.push(id);
    }

    let mut root = accesskit::Node::new(AkRole::Window);
    root.set_children(child_ids);
    // Children carry logical coords; the root transform maps to
    // physical pixels.
    root.set_transform(accesskit::Affine::scale(snap.scale));
    nodes.push((accesskit::NodeId(ROOT_ID), root));

    let focus = snap
        .focus
        .map(|i| accesskit::NodeId(i as u64 + 1))
        .unwrap_or(accesskit::NodeId(ROOT_ID));
    TreeUpdate {
        nodes,
        tree: Some(Tree::new(accesskit::NodeId(ROOT_ID))),
        tree_id: accesskit::TreeId::ROOT,
        focus,
    }
}

/// The onPress name for an AccessKit Click on `node_id`, if that node
/// is actionable.
pub fn action_for(snap: &A11ySnapshot, node_id: u64) -> Option<&str> {
    let idx = (node_id as usize).checked_sub(1)?;
    let item = snap.items.get(idx)?;
    if matches!(item.role, Role::Button | Role::Switch | Role::Checkbox | Role::Radio) {
        item.action.as_deref()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libra_components::{Item, ItemKind, Semantics, SourceDecl};

    fn laid() -> LaidTree {
        let item = |role, text: &str, on_press: Option<&str>, test_id: Option<&str>| Item {
            kind: ItemKind::Box,
            rect: libra_draw::Rect::new(10.0, 20.0, 100.0, 30.0),
            semantics: Semantics {
                role,
                text: text.into(),
                test_id: test_id.map(String::from),
            },
            on_press: on_press.map(String::from),
            on_press_arg: None,
            clip: None,
            scroll: None,
            menu: None,
        };
        LaidTree {
            items: vec![
                item(Role::Container, "", None, None),
                item(Role::Text, "Hello", None, None),
                item(Role::Button, "Save", Some("Save"), Some("save")),
                item(Role::TextInput, "draft", Some("Changed"), Some("field")),
            ],
            width: 400.0,
            height: 200.0,
        }
    }

    #[test]
    fn source_elements_are_excluded_from_the_accesskit_projection() {
        // §12.5: <Frames> et al. are test-query-only — the a11y layer
        // must never surface them to assistive tech, same as
        // Role::Container's transparent boxes.
        let mut l = laid();
        l.items.push(Item {
            kind: ItemKind::Source(SourceDecl::Frames { on_tick: "Tick".into() }),
            rect: libra_draw::Rect::new(0.0, 0.0, 0.0, 0.0),
            semantics: Semantics { role: Role::Source, text: "frames".into(), test_id: None },
            on_press: None,
            on_press_arg: None,
            clip: None,
            scroll: None,
            menu: None,
        });
        let before = snapshot(&laid(), 1.0, None).items.len();
        let after = snapshot(&l, 1.0, None).items.len();
        assert_eq!(before, after, "adding a Source item must not change the a11y snapshot");
        assert!(snapshot(&l, 1.0, None).items.iter().all(|i| i.role != Role::Source));
    }

    #[test]
    fn semantics_map_to_accesskit_roles_and_actions() {
        let snap = snapshot(&laid(), 2.0, Some("field"));
        assert_eq!(snap.items.len(), 3, "containers are skipped");
        assert_eq!(snap.focus, Some(2), "focused input found by testId");

        let update = tree_update(&snap);
        assert_eq!(update.nodes.len(), 4, "3 items + window root");
        assert_eq!(update.focus, accesskit::NodeId(3));

        // The Button node is clickable; the click routes to 'Save'.
        assert_eq!(action_for(&snap, 2), Some("Save"));
        // Text and inputs don't click-dispatch:
        assert_eq!(action_for(&snap, 1), None);
        assert_eq!(action_for(&snap, 3), None);
    }

    #[test]
    fn same_tree_serves_queries_and_a11y() {
        // The §17 claim in one assertion: what by::role finds, AT sees.
        let l = laid();
        let snap = snapshot(&l, 1.0, None);
        let buttons_in_semantics =
            l.items.iter().filter(|i| i.semantics.role == Role::Button).count();
        let buttons_in_a11y = snap.items.iter().filter(|i| i.role == Role::Button).count();
        assert_eq!(buttons_in_semantics, buttons_in_a11y);
    }
}
