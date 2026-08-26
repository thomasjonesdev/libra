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
    if decl.is_empty() {
        return;
    }
    if let Err(e) = validate_menu::<M>(decl) {
        panic!("[libra] invalid context menu: {e}");
    }
    let items = match decl.menus.as_slice() {
        [one] => &one.items,
        _ => return,
    };
    let mut ids = HashMap::new();
    let popup = MudaMenu::new();
    fill(&popup, items, &mut ids);
    native.ids.extend(ids);
    popup_at(&popup, window, physical);
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
