//! Desktop notifications (PLAN-NOTIFY, §12.1).
//!
//! OS chrome — nothing here enters the display list. A host with no
//! notification mechanism returns [`NotifyResult::Unsupported`]. Tests
//! never call [`show`].

use std::path::Path;

use libra_core::{Notification, NotifyResult, Urgency};

/// Show `note`. `on_activate` runs on a background thread when the user
/// clicks the banner (NT2). A repeated `tag` is the replace key (NT3):
/// Linux and the macOS UserNotifications backend honor the id.
pub fn show(
    note: &Notification,
    app_name: &str,
    app_user_model_id: Option<&str>,
    on_activate: Option<Box<dyn FnOnce() + Send + 'static>>,
) -> NotifyResult {
    let needs_bundle = cfg!(target_os = "macos");
    let needs_aumid = cfg!(target_os = "windows");
    let bundled = std::env::current_exe()
        .ok()
        .is_some_and(|exe| exe_is_app_bundle(&exe));
    if let Some(reason) = host_gate(needs_bundle, bundled, needs_aumid, app_user_model_id) {
        return NotifyResult::Unsupported(reason.into());
    }

    let mut banner = notify_rust::Notification::new();
    banner
        .summary(&note.title)
        .body(&note.body)
        .appname(app_name);
    apply_urgency(&mut banner, note.urgency);
    if let Some(tag) = &note.tag {
        apply_tag(&mut banner, tag);
    }
    if on_activate.is_some() {
        banner.action("default", "Show");
    }
    #[cfg(target_os = "windows")]
    if let Some(id) = app_user_model_id {
        banner.app_id(id);
    }
    #[cfg(not(target_os = "windows"))]
    let _ = app_user_model_id;

    match banner.show() {
        Ok(handle) => {
            if let Some(click) = on_activate {
                std::thread::spawn(move || {
                    handle.wait_for_action(move |action| {
                        if action != "__closed" {
                            click();
                        }
                    });
                });
            }
            NotifyResult::Shown
        }
        Err(e) => classify(&e.to_string()),
    }
}

/// `Some` when this host must refuse instead of pretending a banner was shown.
pub fn host_gate(
    needs_bundle: bool,
    bundled: bool,
    needs_aumid: bool,
    app_user_model_id: Option<&str>,
) -> Option<&'static str> {
    if needs_bundle && !bundled {
        return Some("notifications need a bundled .app");
    }
    if needs_aumid && app_user_model_id.unwrap_or("").is_empty() {
        return Some("notifications need a registered AppUserModelID");
    }
    None
}

pub fn exe_is_app_bundle(exe: &Path) -> bool {
    let Some(macos) = exe.parent() else {
        return false;
    };
    if macos.file_name().and_then(|s| s.to_str()) != Some("MacOS") {
        return false;
    }
    let Some(contents) = macos.parent() else {
        return false;
    };
    if contents.file_name().and_then(|s| s.to_str()) != Some("Contents") {
        return false;
    }
    contents
        .parent()
        .and_then(|app| app.extension())
        .is_some_and(|ext| ext == "app")
}

fn apply_urgency(banner: &mut notify_rust::Notification, urgency: Urgency) {
    // macOS uses notify-rust's UserNotifications backend (the
    // `preview-macos-un` dependency feature), which has urgency.
    // The cfg is the target, not a feature of this crate.
    #[cfg(not(target_os = "macos"))]
    banner.urgency(match urgency {
        Urgency::Low => notify_rust::Urgency::Low,
        Urgency::Normal => notify_rust::Urgency::Normal,
        Urgency::Critical => notify_rust::Urgency::Critical,
    });
    #[cfg(target_os = "macos")]
    banner.urgency(match urgency {
        Urgency::Low => notify_rust::Urgency::Low,
        Urgency::Normal => notify_rust::Urgency::Normal,
        Urgency::Critical => notify_rust::Urgency::Critical,
    });
}

fn apply_tag(banner: &mut notify_rust::Notification, tag: &str) {
    // macOS UserNotifications replaces by string id. Other hosts replace
    // by the stable numeric id (Linux). Windows stores the same number;
    // notify-rust 4's toast builder does not read it.
    #[cfg(target_os = "macos")]
    banner.id(tag);
    #[cfg(not(target_os = "macos"))]
    banner.id(tag_id(tag));
}

/// Stable non-zero id so a later notify with the same tag replaces the
/// banner on backends that honor `Notification::id`.
#[cfg_attr(target_os = "macos", allow(dead_code))]
pub fn tag_id(tag: &str) -> u32 {
    let mut hash = 2_166_136_261u32;
    for byte in tag.bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(16_777_619);
    }
    if hash == 0 {
        1
    } else {
        hash
    }
}

pub fn classify(err: &str) -> NotifyResult {
    let lower = err.to_ascii_lowercase();
    if lower.contains("not supported")
        || lower.contains("no notification")
        || lower.contains("not available")
        || lower.contains("org.freedesktop.notifications")
        || lower.contains("no dbus")
        || lower.contains("permission")
    {
        if lower.contains("permission") || lower.contains("denied") {
            NotifyResult::PermissionDenied
        } else {
            NotifyResult::Unsupported(err.to_string())
        }
    } else {
        NotifyResult::Failed(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn a_missing_daemon_is_unsupported_not_a_failure() {
        assert!(matches!(
            classify("org.freedesktop.Notifications was not provided"),
            NotifyResult::Unsupported(_)
        ));
    }

    #[test]
    fn permission_denied_is_its_own_outcome() {
        assert!(matches!(
            classify("notification permission denied"),
            NotifyResult::PermissionDenied
        ));
    }

    #[test]
    fn a_timeout_is_a_failure() {
        assert!(matches!(
            classify("timed out talking to the notification server"),
            NotifyResult::Failed(_)
        ));
    }

    #[test]
    fn tag_ids_are_stable_and_nonzero() {
        assert_eq!(tag_id("aws-session"), tag_id("aws-session"));
        assert_ne!(tag_id("aws-session"), tag_id("other"));
        assert_ne!(tag_id("aws-session"), 0);
    }

    #[test]
    fn dev_builds_and_unpackaged_windows_refuse() {
        assert_eq!(
            host_gate(true, false, false, None),
            Some("notifications need a bundled .app")
        );
        assert_eq!(host_gate(true, true, false, None), None);
        assert_eq!(
            host_gate(false, false, true, None),
            Some("notifications need a registered AppUserModelID")
        );
        assert_eq!(host_gate(false, false, true, Some("Libra.App")), None);
    }

    #[test]
    fn a_mac_bundle_layout_is_recognized() {
        let exe = PathBuf::from("/Applications/AWS Session.app/Contents/MacOS/aws-login");
        assert!(exe_is_app_bundle(&exe));
        assert!(!exe_is_app_bundle(Path::new("/usr/local/bin/aws-login")));
    }
}
