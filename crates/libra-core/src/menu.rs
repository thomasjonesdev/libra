//! Native menubar / context-menu declarations (§19.3).
//!
//! Pure data — no `muda` types — so the shell and the harness share one
//! table. Clicks dispatch reflected actions the same way shortcuts do.

use crate::{parse_chord, ReflectAction};

/// A native menubar: top-level menus, each a list of items.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MenuDecl {
    pub menus: Vec<Menu>,
}

impl MenuDecl {
    pub fn new(menus: Vec<Menu>) -> Self {
        MenuDecl { menus }
    }

    pub fn is_empty(&self) -> bool {
        self.menus.is_empty()
    }
}

/// One top-level menu (`File`, `View`, …) or a context-menu root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Menu {
    pub id: String,
    pub title: String,
    pub items: Vec<MenuItem>,
}

impl Menu {
    pub fn new(id: impl Into<String>, title: impl Into<String>, items: Vec<MenuItem>) -> Self {
        Menu {
            id: id.into(),
            title: title.into(),
            items,
        }
    }
}

/// One entry in a [`Menu`]. Actions carry a reflected name and an
/// optional static string payload — same rule as §19.2 shortcuts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuItem {
    Action {
        id: String,
        label: String,
        action: String,
        arg: Option<String>,
        enabled: bool,
        checked: bool,
        accelerator: Option<String>,
    },
    Separator,
    Submenu {
        id: String,
        label: String,
        items: Vec<MenuItem>,
    },
}

impl MenuItem {
    pub fn action(
        id: impl Into<String>,
        label: impl Into<String>,
        action: impl Into<String>,
    ) -> Self {
        MenuItem::Action {
            id: id.into(),
            label: label.into(),
            action: action.into(),
            arg: None,
            enabled: true,
            checked: false,
            accelerator: None,
        }
    }

    pub fn with_arg(mut self, arg: impl Into<String>) -> Self {
        if let MenuItem::Action { arg: slot, .. } = &mut self {
            *slot = Some(arg.into());
        }
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        if let MenuItem::Action { enabled: slot, .. } = &mut self {
            *slot = enabled;
        }
        self
    }

    pub fn checked(mut self, checked: bool) -> Self {
        if let MenuItem::Action { checked: slot, .. } = &mut self {
            *slot = checked;
        }
        self
    }

    pub fn accelerator(mut self, chord: impl Into<String>) -> Self {
        if let MenuItem::Action { accelerator, .. } = &mut self {
            *accelerator = Some(chord.into());
        }
        self
    }

    pub fn submenu(id: impl Into<String>, label: impl Into<String>, items: Vec<MenuItem>) -> Self {
        MenuItem::Submenu {
            id: id.into(),
            label: label.into(),
            items,
        }
    }

    pub fn separator() -> Self {
        MenuItem::Separator
    }
}

/// Validate every action name (and optional accelerator chord) against
/// reflected actions. Errors name the offending item.
pub fn validate_menu<M: ReflectAction>(decl: &MenuDecl) -> Result<(), String> {
    for menu in &decl.menus {
        validate_items::<M>(&menu.items, &menu.title)?;
    }
    Ok(())
}

fn validate_items<M: ReflectAction>(items: &[MenuItem], path: &str) -> Result<(), String> {
    for item in items {
        match item {
            MenuItem::Separator => {}
            MenuItem::Submenu { id, label, items } => {
                let next = format!("{path} > {label} ({id})");
                validate_items::<M>(items, &next)?;
            }
            MenuItem::Action {
                id,
                label,
                action,
                arg,
                accelerator,
                ..
            } => {
                let where_ = format!("{path} > {label} ({id})");
                if let Some(chord) = accelerator {
                    parse_chord(chord).map_err(|e| format!("menu {where_}: {e}"))?;
                }
                let resolves = match arg {
                    Some(a) => M::action_with_str(action, a.clone()).is_some(),
                    None => M::action_by_name(action).is_some(),
                };
                if !resolves {
                    let kind = if arg.is_some() {
                        "String-payload"
                    } else {
                        "unit"
                    };
                    return Err(format!(
                        "menu {where_}: '{action}' is not a {kind} action — check the actions block (§19.3)"
                    ));
                }
            }
        }
    }
    Ok(())
}

/// Flatten action items as `(action, arg)` for harness lookup.
pub fn collect_menu_actions(decl: &MenuDecl) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    for menu in &decl.menus {
        collect_items(&menu.items, &mut out);
    }
    out
}

fn collect_items(items: &[MenuItem], out: &mut Vec<(String, Option<String>)>) {
    for item in items {
        match item {
            MenuItem::Action {
                action,
                arg,
                enabled,
                ..
            } if *enabled => {
                out.push((action.clone(), arg.clone()));
            }
            MenuItem::Submenu { items, .. } => collect_items(items, out),
            _ => {}
        }
    }
}

/// First matching enabled action in the decl.
pub fn find_menu_action<'a>(
    decl: &'a MenuDecl,
    action: &str,
    arg: Option<&str>,
) -> Option<&'a MenuItem> {
    for menu in &decl.menus {
        if let Some(item) = find_item(&menu.items, action, arg) {
            return Some(item);
        }
    }
    None
}

fn find_item<'a>(items: &'a [MenuItem], action: &str, arg: Option<&str>) -> Option<&'a MenuItem> {
    for item in items {
        match item {
            MenuItem::Action {
                action: a,
                arg: item_arg,
                enabled: true,
                ..
            } if a == action && item_arg.as_deref() == arg => {
                return Some(item);
            }
            MenuItem::Submenu { items, .. } => {
                if let Some(hit) = find_item(items, action, arg) {
                    return Some(hit);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ReflectAction;

    #[derive(Clone, Debug)]
    #[allow(dead_code)]
    enum Msg {
        New,
        Open(String),
    }

    impl ReflectAction for Msg {
        fn action_by_name(name: &str) -> Option<Msg> {
            match name {
                "newNote" => Some(Msg::New),
                _ => None,
            }
        }
        fn action_with_str(name: &str, value: String) -> Option<Self> {
            match name {
                "openVault" => Some(Msg::Open(value)),
                _ => None,
            }
        }
        fn action_names() -> &'static [&'static str] {
            &["newNote", "openVault"]
        }
    }

    fn sample() -> MenuDecl {
        MenuDecl::new(vec![Menu::new(
            "file",
            "File",
            vec![
                MenuItem::action("new", "New Note", "newNote").accelerator("cmd+n"),
                MenuItem::separator(),
                MenuItem::submenu(
                    "recents",
                    "Open Recent",
                    vec![MenuItem::action("r1", "Vault", "openVault").with_arg("/tmp/a")],
                ),
            ],
        )])
    }

    #[test]
    fn accepts_unit_and_string_actions() {
        assert!(validate_menu::<Msg>(&sample()).is_ok());
        let hits = collect_menu_actions(&sample());
        assert!(hits.contains(&("newNote".into(), None)));
        assert!(hits.contains(&("openVault".into(), Some("/tmp/a".into()))));
    }

    #[test]
    fn rejects_unknown_action() {
        let decl = MenuDecl::new(vec![Menu::new(
            "file",
            "File",
            vec![MenuItem::action("x", "Nope", "quench")],
        )]);
        let err = validate_menu::<Msg>(&decl).unwrap_err();
        assert!(err.contains("'quench' is not a unit action"), "{err}");
    }

    #[test]
    fn rejects_bad_accelerator() {
        let decl = MenuDecl::new(vec![Menu::new(
            "file",
            "File",
            vec![MenuItem::action("n", "New", "newNote").accelerator("cmd+???")],
        )]);
        assert!(validate_menu::<Msg>(&decl)
            .unwrap_err()
            .contains("unknown key"));
    }
}
