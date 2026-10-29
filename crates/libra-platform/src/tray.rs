//! Status-item tray (PLAN-RESIDENCY R1).
//!
//! The declaration is pure ([`libra_core::TrayDecl`]). This module turns
//! it into an OS icon. Clicks arrive on `tray-icon`'s own menu channel
//! (it links `muda` 0.15; the menubar links 0.16) and the shell dispatches
//! the item id, which is the reflected action name.
//!
//! Icons are generated from the closed [`TrayIcon`] set — never loaded
//! from a path.

use libra_core::{validate_tray, ReflectAction, TrayDecl, TrayIcon as TrayKind, TrayItem};
use tray_icon::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};

pub struct TrayHost {
    icon: Option<tray_icon::TrayIcon>,
    last: Option<TrayDecl>,
}

impl TrayHost {
    pub fn new() -> Self {
        TrayHost {
            icon: None,
            last: None,
        }
    }

    pub fn sync<M: ReflectAction>(&mut self, decl: Option<TrayDecl>) {
        if decl == self.last {
            return;
        }
        let Some(decl) = decl else {
            self.icon = None;
            self.last = None;
            return;
        };
        if let Err(e) = validate_tray::<M>(&decl) {
            panic!("[libra] invalid tray: {e}");
        }
        let menu = build_menu(&decl);
        let (rgba, w, h) = rgba(decl.icon);
        let built = (|| {
            let image = tray_icon::Icon::from_rgba(rgba, w, h).map_err(|e| e.to_string())?;
            tray_icon::TrayIconBuilder::new()
                .with_tooltip(&decl.tooltip)
                .with_icon(image)
                .with_menu(Box::new(menu))
                .build()
                .map_err(|e| e.to_string())
        })();
        match built {
            Ok(icon) => {
                self.icon = Some(icon);
                self.last = Some(decl);
            }
            Err(e) => {
                eprintln!("[libra] tray unavailable ({e})");
                self.icon = None;
                self.last = Some(decl);
            }
        }
    }
}

fn build_menu(decl: &TrayDecl) -> Menu {
    let menu = Menu::new();
    for item in &decl.items {
        match item {
            TrayItem::Label(text) => {
                let row = MenuItem::new(text, false, None);
                let _ = menu.append(&row);
            }
            TrayItem::Separator => {
                let _ = menu.append(&PredefinedMenuItem::separator());
            }
            TrayItem::Action {
                text,
                action,
                enabled,
            } => {
                let row = MenuItem::with_id(action, text, *enabled, None);
                let _ = menu.append(&row);
            }
            TrayItem::Check {
                text,
                action,
                checked,
                enabled,
            } => {
                let row = CheckMenuItem::with_id(action, text, *enabled, *checked, None);
                let _ = menu.append(&row);
            }
        }
    }
    menu
}

/// 32×32 circle. Center pixel is opaque; the corners stay transparent.
pub fn rgba(kind: TrayKind) -> (Vec<u8>, u32, u32) {
    const N: u32 = 32;
    let (r, g, b) = match kind {
        TrayKind::Default => (120, 120, 128),
        TrayKind::Ok => (48, 164, 108),
        TrayKind::Warn => (200, 140, 40),
        TrayKind::Error => (196, 64, 64),
    };
    let mut buf = vec![0u8; (N * N * 4) as usize];
    let cx = (N as f32 - 1.0) / 2.0;
    let radius = 11.0;
    for y in 0..N {
        for x in 0..N {
            let dx = x as f32 - cx;
            let dy = y as f32 - cx;
            if dx * dx + dy * dy <= radius * radius {
                let i = ((y * N + x) * 4) as usize;
                buf[i] = r;
                buf[i + 1] = g;
                buf[i + 2] = b;
                buf[i + 3] = 255;
            }
        }
    }
    (buf, N, N)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_status_paints_a_different_opaque_circle() {
        let (ok, w, h) = rgba(TrayKind::Ok);
        let (warn, _, _) = rgba(TrayKind::Warn);
        assert_eq!((w, h), (32, 32));
        assert_ne!(ok, warn);
        let center = ((16 * 32 + 16) * 4) as usize;
        assert_eq!(ok[center + 3], 255);
        assert_eq!(ok[3], 0, "the corner stays transparent");
    }
}
