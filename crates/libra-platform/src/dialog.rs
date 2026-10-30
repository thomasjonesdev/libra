//! `Effect::Dialog` (PLAN-NATIVE-INTEGRATIONS N2, §12.1).
//!
//! An OS alert or confirm via `rfd`. Tests call [`host_gate`] and
//! [`libra_core::interpret_dialog`]. They do not call [`show`].

use libra_core::{DialogButton, DialogKind, DialogResult, DialogSpec};

/// `Some` when this host must refuse instead of pretending a sheet was shown.
/// Linux message dialogs go through zenity (rfd's non-GTK backend).
pub fn host_gate(needs_zenity: bool, zenity_on_path: bool) -> Option<&'static str> {
    if needs_zenity && !zenity_on_path {
        Some("alert dialogs need zenity")
    } else {
        None
    }
}

pub fn show(spec: &DialogSpec) -> DialogResult {
    if cfg!(target_os = "linux") {
        if let Some(reason) = host_gate(true, zenity_on_path()) {
            return DialogResult::Unsupported(reason.into());
        }
    }
    let buttons = match spec.kind {
        DialogKind::Alert => rfd::MessageButtons::Ok,
        DialogKind::Confirm => rfd::MessageButtons::OkCancel,
    };
    let result = rfd::MessageDialog::new()
        .set_title(&spec.title)
        .set_description(&spec.message)
        .set_level(rfd::MessageLevel::Info)
        .set_buttons(buttons)
        .show();
    libra_core::interpret_dialog(spec.kind, button(result))
}

fn button(result: rfd::MessageDialogResult) -> DialogButton {
    match result {
        rfd::MessageDialogResult::Ok => DialogButton::Ok,
        rfd::MessageDialogResult::Yes => DialogButton::Yes,
        rfd::MessageDialogResult::No => DialogButton::No,
        rfd::MessageDialogResult::Cancel => DialogButton::Cancel,
        rfd::MessageDialogResult::Custom(_) => DialogButton::Other,
    }
}

fn zenity_on_path() -> bool {
    std::process::Command::new("zenity")
        .arg("--version")
        .output()
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::host_gate;

    #[test]
    fn linux_without_zenity_refuses() {
        assert_eq!(host_gate(true, false), Some("alert dialogs need zenity"));
        assert_eq!(host_gate(true, true), None);
        assert_eq!(host_gate(false, false), None);
    }
}
