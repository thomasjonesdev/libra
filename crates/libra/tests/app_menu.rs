//! Covers `libra::app!`'s optional `menu:` field (§19.3).

use libra::{actions, app, state};
use libra_core::{validate_menu, App, Menu, MenuDecl, MenuItem};

#[derive(Default)]
#[state]
struct Editor {
    recent: String,
}

#[actions]
impl Editor {
    fn new_note(&mut self) {}
    fn open_vault(&mut self, path: String) {
        self.recent = path;
    }
}

fn menu(state: &Editor) -> MenuDecl {
    let mut recents = Vec::new();
    if !state.recent.is_empty() {
        recents.push(MenuItem::action("r1", "Last", "openVault").with_arg(&state.recent));
    }
    MenuDecl::new(vec![Menu::new(
        "file",
        "File",
        vec![
            MenuItem::action("new", "New", "newNote"),
            MenuItem::submenu("recents", "Open Recent", recents),
        ],
    )])
}

app! {
    state: Editor,
    view: "fixtures/counter.lx",
    menu: menu,
}

#[test]
fn menu_is_empty_until_a_recent_exists() {
    let mut e = Editor::default();
    assert!(validate_menu::<Action>(&e.menu()).is_ok());
    assert_eq!(libra_core::collect_menu_actions(&e.menu()).len(), 1);
    e.recent = "/tmp/v".into();
    let hits = libra_core::collect_menu_actions(&e.menu());
    assert!(hits.contains(&("openVault".into(), Some("/tmp/v".into()))));
}
