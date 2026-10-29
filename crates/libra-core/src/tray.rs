//! Tray / menubar declaration (PLAN-RESIDENCY R1, §19).
//!
//! Pure data — no `tray-icon` types — so the shell and the harness share
//! one table. Clicks dispatch reflected actions the same way menus do.

use crate::ReflectAction;

/// Closed icon set. The shell paints these; a path would put filesystem
/// bytes on the chrome path and make the decl untestable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrayIcon {
    #[default]
    Default,
    Ok,
    Warn,
    Error,
}

/// One status-item menu. `None` from [`crate::App::tray`] means no icon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayDecl {
    pub tooltip: String,
    pub icon: TrayIcon,
    pub items: Vec<TrayItem>,
}

impl TrayDecl {
    pub fn new(tooltip: impl Into<String>, icon: TrayIcon, items: Vec<TrayItem>) -> Self {
        TrayDecl {
            tooltip: tooltip.into(),
            icon,
            items,
        }
    }
}

/// One tray-menu entry. Actions carry a reflected name — same rule as
/// §19.2 shortcuts. `Label` is not clickable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayItem {
    Label(String),
    Action {
        text: String,
        action: String,
        enabled: bool,
    },
    Check {
        text: String,
        action: String,
        checked: bool,
        enabled: bool,
    },
    Separator,
}

impl TrayItem {
    pub fn action(text: impl Into<String>, action: impl Into<String>) -> Self {
        TrayItem::Action {
            text: text.into(),
            action: action.into(),
            enabled: true,
        }
    }

    pub fn check(text: impl Into<String>, action: impl Into<String>, checked: bool) -> Self {
        TrayItem::Check {
            text: text.into(),
            action: action.into(),
            checked,
            enabled: true,
        }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        match &mut self {
            TrayItem::Action { enabled: slot, .. } | TrayItem::Check { enabled: slot, .. } => {
                *slot = enabled;
            }
            TrayItem::Label(_) | TrayItem::Separator => {}
        }
        self
    }

    pub fn separator() -> Self {
        TrayItem::Separator
    }

    pub fn label(text: impl Into<String>) -> Self {
        TrayItem::Label(text.into())
    }
}

/// Validate every action name against reflected unit actions.
pub fn validate_tray<M: ReflectAction>(decl: &TrayDecl) -> Result<(), String> {
    for item in &decl.items {
        let (text, action) = match item {
            TrayItem::Action { text, action, .. } | TrayItem::Check { text, action, .. } => {
                (text.as_str(), action.as_str())
            }
            TrayItem::Label(_) | TrayItem::Separator => continue,
        };
        if M::action_by_name(action).is_none() {
            return Err(format!(
                "tray '{text}': '{action}' is not a unit action — check the actions block (§19)"
            ));
        }
    }
    Ok(())
}

/// `Ok(())` when `action` is present and enabled. The error distinguishes
/// a missing item from a disabled one so a test failure names the cause.
pub fn tray_clickable(decl: &TrayDecl, action: &str) -> Result<(), String> {
    let mut disabled = false;
    for item in &decl.items {
        let (name, enabled) = match item {
            TrayItem::Action {
                action: name,
                enabled,
                ..
            }
            | TrayItem::Check {
                action: name,
                enabled,
                ..
            } => (name.as_str(), *enabled),
            TrayItem::Label(_) | TrayItem::Separator => continue,
        };
        if name == action {
            if enabled {
                return Ok(());
            }
            disabled = true;
        }
    }
    if disabled {
        Err(format!("click_tray: '{action}' is disabled"))
    } else {
        Err(format!("click_tray: '{action}' is not in the tray menu"))
    }
}

/// Markup `<Stream>` handlers first, then ambient [`crate::Sub::Stream`]
/// ids that markup did not declare (PLAN-STREAM-DELIVERY D5). A repeated
/// id keeps the markup handler.
pub fn union_stream_handlers(
    markup: &[(String, String)],
    ambient: &[(String, String)],
) -> Vec<(String, String)> {
    let mut out = markup.to_vec();
    for (id, handler) in ambient {
        if out.iter().any(|(existing, _)| existing == id) {
            continue;
        }
        out.push((id.clone(), handler.clone()));
    }
    out
}
