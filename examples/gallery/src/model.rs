//! Pure domain layer (§5.9 module layout): the curated component registry the
//! sidebar lists and the projected row type. Nothing here knows about actions,
//! effects, or the view.
//!
//! Architecture: ARCHITECTURE.md §5.9 (module layout), §6.3 (projections);
//! plan PLAN-GALLERY.

use libra_core::LibraReflect;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A sidebar entry — projected each action with `selected` set for the current
/// component (§6.3). Bound through `<for>` as
/// `<ListItem title={c.name} selected={c.selected} onPress={select(c.id)} />`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, LibraReflect)]
pub struct ComponentRow {
    pub id: String,
    pub name: String,
    pub selected: bool,
}

impl fmt::Display for ComponentRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.id) // key={c.id}
    }
}

// The curated catalog, grouped for the sidebar. `(id, display name)`. Adding a
// catalog component means adding a row here + its canvas block in the view.
pub const LAYOUT: &[(&str, &str)] = &[
    ("column", "Column"),
    ("row", "Row"),
    ("spacer", "Spacer"),
    ("separator", "Separator"),
    ("split", "Split"),
    ("card", "Card"),
    ("panel", "Panel"),
];
pub const DISPLAY: &[(&str, &str)] = &[
    ("text", "Text"),
    ("heading", "Heading"),
    ("badge", "Badge"),
    ("progress", "Progress"),
    ("alert", "Alert"),
    ("skeleton", "Skeleton"),
    ("avatar", "Avatar"),
    ("tooltip", "Tooltip"),
    ("breadcrumb", "Breadcrumb"),
    ("pagination", "Pagination"),
    ("stepper", "Stepper"),
    ("timeline", "Timeline"),
    ("tabs", "Tabs"),
];
pub const CONTROLS: &[(&str, &str)] = &[
    ("button", "Button"),
    ("input", "Input"),
    ("search", "Search"),
    ("chip", "Chip"),
    ("textarea", "Textarea"),
    ("list_item", "ListItem"),
    ("switch", "Switch"),
    ("checkbox", "Checkbox"),
    ("radio", "Radio"),
    ("slider", "Slider"),
    ("dialog", "Dialog"),
    ("drawer", "Drawer"),
];
pub const DATA: &[(&str, &str)] = &[("chart", "Chart"), ("sparkline", "Sparkline")];

/// The component selected on launch.
pub const DEFAULT: &str = "button";

/// Project a group into sidebar rows, flagging the current selection.
pub fn rows(group: &[(&str, &str)], selected: &str) -> Vec<ComponentRow> {
    group
        .iter()
        .map(|(id, name)| ComponentRow {
            id: (*id).into(),
            name: (*name).into(),
            selected: *id == selected,
        })
        .collect()
}

/// Display name for a component id (title of the canvas).
pub fn name_of(id: &str) -> String {
    [LAYOUT, DISPLAY, CONTROLS, DATA]
        .iter()
        .flat_map(|g| g.iter())
        .find(|(i, _)| *i == id)
        .map(|(_, name)| (*name).to_string())
        .unwrap_or_default()
}
