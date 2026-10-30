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
    ("safe_area", "SafeArea"),
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
    ("icon", "Icon"),
    ("tabs", "Tabs"),
    ("toast", "Toast"),
    ("toolbar", "Toolbar"),
    ("statusbar", "StatusBar"),
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
pub const DATA: &[(&str, &str)] = &[
    ("chart", "Chart"),
    ("sparkline", "Sparkline"),
    ("virtual_list", "VirtualList"),
    ("log_view", "LogView"),
    ("table", "Table"),
];

/// One `<VirtualList>` row. `title` is the label; Display is the press arg.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, LibraReflect)]
pub struct SampleRow {
    pub id: String,
    pub title: String,
}

impl fmt::Display for SampleRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

/// One `<Table>` row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, LibraReflect)]
pub struct Person {
    pub id: String,
    pub name: String,
    pub role: String,
}

impl fmt::Display for Person {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

/// `<Table columns>` — width is an integer so its Display parses as px.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, LibraReflect)]
pub struct TableCol {
    pub key: String,
    pub title: String,
    pub width: i64,
}

impl fmt::Display for TableCol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.key)
    }
}

pub fn sample_rows() -> Vec<SampleRow> {
    (0..24)
        .map(|i| SampleRow { id: format!("r{i}"), title: format!("Row {i:02}") })
        .collect()
}

pub fn people() -> Vec<Person> {
    ["Ada|math", "Grace|systems", "Barbara|compilers", "Katherine|orbit"]
        .into_iter()
        .enumerate()
        .map(|(i, row)| {
            let (name, role) = row.split_once('|').unwrap();
            Person { id: format!("p{i}"), name: name.into(), role: role.into() }
        })
        .collect()
}

pub fn table_cols() -> Vec<TableCol> {
    vec![
        TableCol { key: "name".into(), title: "Name".into(), width: 140 },
        TableCol { key: "role".into(), title: "Role".into(), width: 120 },
    ]
}

pub fn log_lines() -> Vec<String> {
    (0..18).map(|i| format!("log {i:02}")).chain(std::iter::once("tail marker".into())).collect()
}

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
