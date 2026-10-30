//! Native menubar + context popup via `muda` (§19.3 / N7).
//!
//! OS chrome — nothing here enters the display list. The decl is
//! validated and built from `App::menu` / `App::context_menu`.

use std::collections::HashMap;

use libra_core::{parse_chord, validate_menu, Chord, ChordKey, MenuDecl, MenuItem, ReflectAction};
use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{
    CheckMenuItem, ContextMenu, Menu as MudaMenu, MenuId, MenuItem as MudaItem, PredefinedMenuItem,
    Submenu,
};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

/// User-event wrapper so menu activations wake the winit loop.
pub(crate) enum ShellEvent<M> {
    App(M),
    Menu(MenuId),
    /// §18.2: SIGTERM/SIGINT asked the process to snapshot and exit so
    /// `libra dev` can restart onto the same model.
    PersistAndExit,
    /// A mounted stream has events waiting (PLAN-STREAM-DELIVERY).
    Streams,
    /// A signed feed offered a newer build (PLAN-RELEASE).
    UpdateOffer(crate::update::UpdateOffer),
    /// The verified replacement `.app` is ready to swap in.
    ApplyUpdate(std::path::PathBuf),
    /// A notification click (PLAN-NOTIFY NT2).
    NotifyActivate(M),
    /// The second instance asked this process to raise its window.
    RaiseWindow,
    /// A tray menu click. The string is the reflected action name.
    /// `tray-icon` speaks a different `muda` than the menubar, so this
    /// is not a [`muda::MenuId`].
    TrayClick(String),
    /// One JSON line from the automation socket (§16). The shell answers
    /// on `reply` so the listener thread never touches the app.
    Automate {
        line: String,
        reply: std::sync::mpsc::Sender<String>,
    },
}

pub(crate) struct NativeMenus {
    bar: Option<MudaMenu>,
    ids: HashMap<MenuId, (String, Option<String>)>,
    last: MenuDecl,
}

impl NativeMenus {
    pub(crate) fn new() -> Self {
        NativeMenus {
            bar: None,
            ids: HashMap::new(),
            last: MenuDecl::default(),
        }
    }

    pub(crate) fn lookup(&self, id: &MenuId) -> Option<(String, Option<String>)> {
        self.ids.get(id).cloned()
    }
}

pub(crate) fn sync_menubar<M: ReflectAction>(
    native: &mut NativeMenus,
    decl: MenuDecl,
    window: Option<&Window>,
) {
    if decl == native.last {
        return;
    }
    if let Err(e) = validate_menu::<M>(&decl) {
        panic!("[libra] invalid menu: {e}");
    }
    if decl.is_empty() {
        native.bar = None;
        native.ids.clear();
        native.last = decl;
        return;
    }
    let (menu, ids) = build_bar(&decl);
    attach_bar(&menu, window);
    native.bar = Some(menu);
    native.ids = ids;
    native.last = decl;
}

pub(crate) fn show_context<M: ReflectAction>(
    native: &mut NativeMenus,
    decl: &MenuDecl,
    window: &Window,
    physical: (f64, f64),
) {
    if let Err(e) = validate_menu::<M>(decl) {
        panic!("[libra] invalid context menu: {e}");
    }
    let items = match context_items(decl) {
        Ok(items) => items,
        Err(reason) => {
            eprintln!("[libra] context menu refused: {reason}");
            return;
        }
    };
    let mut ids = HashMap::new();
    let popup = MudaMenu::new();
    fill(&popup, items, &mut ids);
    native.ids.extend(ids);
    popup_at(&popup, window, physical);
}

/// A context popup is one menu. A menubar (several top-level menus) or
/// an empty decl is refused out loud — a silent return would leave the
/// right-click looking like a dead control (§19.4).
pub(crate) fn context_items(decl: &MenuDecl) -> Result<&[MenuItem], &'static str> {
    match decl.menus.as_slice() {
        [one] => Ok(one.items.as_slice()),
        [] => Err("context menu is empty"),
        _ => Err("a context menu is one menu"),
    }
}

fn build_bar(decl: &MenuDecl) -> (MudaMenu, HashMap<MenuId, (String, Option<String>)>) {
    let menu = MudaMenu::new();
    let mut ids = HashMap::new();
    #[cfg(target_os = "macos")]
    {
        let app = Submenu::new("App", true);
        let _ = app.append(&PredefinedMenuItem::hide(None));
        let _ = app.append(&PredefinedMenuItem::hide_others(None));
        let _ = app.append(&PredefinedMenuItem::separator());
        let _ = app.append(&PredefinedMenuItem::quit(None));
        let _ = menu.append(&app);
    }
    for m in &decl.menus {
        let sub = Submenu::new(&m.title, true);
        fill_sub(&sub, &m.items, &mut ids);
        let _ = menu.append(&sub);
    }
    (menu, ids)
}

fn fill(menu: &MudaMenu, items: &[MenuItem], ids: &mut HashMap<MenuId, (String, Option<String>)>) {
    for item in items {
        match item {
            MenuItem::Separator => {
                let _ = menu.append(&PredefinedMenuItem::separator());
            }
            MenuItem::Action { .. } => append_action(menu, item, ids),
            MenuItem::Submenu { label, items, .. } => {
                let sub = Submenu::new(label, true);
                fill_sub(&sub, items, ids);
                let _ = menu.append(&sub);
            }
        }
    }
}

fn fill_sub(
    menu: &Submenu,
    items: &[MenuItem],
    ids: &mut HashMap<MenuId, (String, Option<String>)>,
) {
    for item in items {
        match item {
            MenuItem::Separator => {
                let _ = menu.append(&PredefinedMenuItem::separator());
            }
            MenuItem::Action { .. } => append_action_sub(menu, item, ids),
            MenuItem::Submenu { label, items, .. } => {
                let sub = Submenu::new(label, true);
                fill_sub(&sub, items, ids);
                let _ = menu.append(&sub);
            }
        }
    }
}

fn append_action(
    menu: &MudaMenu,
    item: &MenuItem,
    ids: &mut HashMap<MenuId, (String, Option<String>)>,
) {
    let (native, action, arg) = action_item(item);
    match native {
        NativeItem::Plain(i) => {
            ids.insert(i.id().clone(), (action, arg));
            let _ = menu.append(&i);
        }
        NativeItem::Check(i) => {
            ids.insert(i.id().clone(), (action, arg));
            let _ = menu.append(&i);
        }
    }
}

fn append_action_sub(
    menu: &Submenu,
    item: &MenuItem,
    ids: &mut HashMap<MenuId, (String, Option<String>)>,
) {
    let (native, action, arg) = action_item(item);
    match native {
        NativeItem::Plain(i) => {
            ids.insert(i.id().clone(), (action, arg));
            let _ = menu.append(&i);
        }
        NativeItem::Check(i) => {
            ids.insert(i.id().clone(), (action, arg));
            let _ = menu.append(&i);
        }
    }
}

enum NativeItem {
    Plain(MudaItem),
    Check(CheckMenuItem),
}

fn action_item(item: &MenuItem) -> (NativeItem, String, Option<String>) {
    let MenuItem::Action {
        id,
        label,
        action,
        arg,
        enabled,
        checked,
        accelerator,
    } = item
    else {
        unreachable!("action_item on non-action");
    };
    let accel = accelerator.as_deref().and_then(chord_accel);
    let native = if *checked {
        NativeItem::Check(CheckMenuItem::with_id(id, label, *enabled, true, accel))
    } else {
        NativeItem::Plain(MudaItem::with_id(id, label, *enabled, accel))
    };
    (native, action.clone(), arg.clone())
}

fn attach_bar(menu: &MudaMenu, window: Option<&Window>) {
    #[cfg(target_os = "macos")]
    {
        let _ = window;
        menu.init_for_nsapp();
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(hwnd) = window.and_then(win_hwnd) {
            let _ = unsafe { menu.init_for_hwnd(hwnd) };
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (menu, window);
        note_linux_menu_unsupported();
    }
}

fn popup_at(menu: &MudaMenu, window: &Window, physical: (f64, f64)) {
    let pos = muda::dpi::PhysicalPosition::new(physical.0, physical.1);
    #[cfg(target_os = "macos")]
    {
        if let Some(view) = ns_view(window) {
            unsafe {
                menu.show_context_menu_for_nsview(view, Some(pos.into()));
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(hwnd) = win_hwnd(window) {
            unsafe {
                menu.show_context_menu_for_hwnd(hwnd, Some(pos.into()));
            }
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (menu, window, pos);
        note_linux_menu_unsupported();
    }
}

/// Linux winit windows are X11/Wayland, not `gtk::Window`. muda's GTK
/// attach needs that window. Compiling GTK and discarding the menu
/// would hide the gap (§19.4), so this is a typed refusal.
#[cfg_attr(any(target_os = "macos", target_os = "windows"), allow(dead_code))]
pub fn linux_menu_support() -> Result<(), &'static str> {
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Err("winit has no GTK window for muda")
    }
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        Ok(())
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn note_linux_menu_unsupported() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static ONCE: AtomicBool = AtomicBool::new(false);
    if ONCE.swap(true, Ordering::Relaxed) {
        return;
    }
    if let Err(reason) = linux_menu_support() {
        eprintln!("[libra] native menu unsupported on this host: {reason} (§19.4)");
    }
}

#[cfg(target_os = "macos")]
fn ns_view(window: &Window) -> Option<*mut std::ffi::c_void> {
    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::AppKit(h) => Some(h.ns_view.as_ptr()),
        _ => None,
    }
}

#[cfg(target_os = "windows")]
fn win_hwnd(window: &Window) -> Option<isize> {
    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get() as isize),
        _ => None,
    }
}

fn chord_accel(chord: &str) -> Option<Accelerator> {
    let c = parse_chord(chord).ok()?;
    Some(Accelerator::new(Some(chord_mods(&c)), chord_code(&c)?))
}

fn chord_mods(c: &Chord) -> Modifiers {
    let mut mods = Modifiers::empty();
    if c.cmd {
        #[cfg(target_os = "macos")]
        {
            mods |= Modifiers::SUPER;
        }
        #[cfg(not(target_os = "macos"))]
        {
            mods |= Modifiers::CONTROL;
        }
    }
    if c.ctrl {
        mods |= Modifiers::CONTROL;
    }
    if c.alt {
        mods |= Modifiers::ALT;
    }
    if c.shift {
        mods |= Modifiers::SHIFT;
    }
    mods
}

fn chord_code(c: &Chord) -> Option<Code> {
    Some(match c.key {
        ChordKey::Char(ch) => char_code(ch)?,
        ChordKey::Escape => Code::Escape,
        ChordKey::Backspace => Code::Backspace,
        ChordKey::Delete => Code::Delete,
        ChordKey::Enter => Code::Enter,
        ChordKey::Space => Code::Space,
        ChordKey::Tab => Code::Tab,
        ChordKey::Up => Code::ArrowUp,
        ChordKey::Down => Code::ArrowDown,
        ChordKey::Left => Code::ArrowLeft,
        ChordKey::Right => Code::ArrowRight,
    })
}

#[cfg(test)]
mod tests {
    use super::{chord_accel, context_items, linux_menu_support};
    use libra_core::{Menu, MenuDecl};

    #[test]
    fn linux_menus_are_a_typed_refusal() {
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        assert!(linux_menu_support().unwrap_err().contains("GTK"));
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        assert!(linux_menu_support().is_ok());
    }

    #[test]
    fn a_context_menu_is_exactly_one_menu() {
        let one = MenuDecl::new(vec![Menu::new("edit", "Edit", vec![])]);
        assert!(context_items(&one).is_ok());
        let none = MenuDecl::default();
        assert_eq!(context_items(&none).unwrap_err(), "context menu is empty");
        let two = MenuDecl::new(vec![
            Menu::new("a", "A", vec![]),
            Menu::new("b", "B", vec![]),
        ]);
        assert_eq!(
            context_items(&two).unwrap_err(),
            "a context menu is one menu"
        );
    }

    #[test]
    fn every_parsed_chord_becomes_an_accelerator() {
        for key in [
            "a", "z", "0", "9", "escape", "backspace", "delete", "enter", "space", "tab", "up",
            "down", "left", "right",
        ] {
            let spec = format!("cmd+shift+{key}");
            let accel = chord_accel(&spec).unwrap_or_else(|| panic!("{spec} dropped"));
            let rendered = format!("{accel:?}");
            #[cfg(target_os = "macos")]
            assert!(rendered.contains("SUPER"), "{spec} -> {rendered}");
            #[cfg(not(target_os = "macos"))]
            assert!(rendered.contains("CONTROL"), "{spec} -> {rendered}");
            assert!(rendered.contains("SHIFT"), "{spec} -> {rendered}");
        }
    }
}

fn char_code(c: char) -> Option<Code> {
    Some(match c {
        'a' => Code::KeyA,
        'b' => Code::KeyB,
        'c' => Code::KeyC,
        'd' => Code::KeyD,
        'e' => Code::KeyE,
        'f' => Code::KeyF,
        'g' => Code::KeyG,
        'h' => Code::KeyH,
        'i' => Code::KeyI,
        'j' => Code::KeyJ,
        'k' => Code::KeyK,
        'l' => Code::KeyL,
        'm' => Code::KeyM,
        'n' => Code::KeyN,
        'o' => Code::KeyO,
        'p' => Code::KeyP,
        'q' => Code::KeyQ,
        'r' => Code::KeyR,
        's' => Code::KeyS,
        't' => Code::KeyT,
        'u' => Code::KeyU,
        'v' => Code::KeyV,
        'w' => Code::KeyW,
        'x' => Code::KeyX,
        'y' => Code::KeyY,
        'z' => Code::KeyZ,
        '0' => Code::Digit0,
        '1' => Code::Digit1,
        '2' => Code::Digit2,
        '3' => Code::Digit3,
        '4' => Code::Digit4,
        '5' => Code::Digit5,
        '6' => Code::Digit6,
        '7' => Code::Digit7,
        '8' => Code::Digit8,
        '9' => Code::Digit9,
        _ => return None,
    })
}
