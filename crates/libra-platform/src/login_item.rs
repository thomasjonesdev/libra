//! Launch at login (PLAN-RESIDENCY R5).
//!
//! Linux writes an XDG autostart desktop file. Windows writes a Startup
//! folder command. macOS writes a LaunchAgent only when the executable
//! is inside a bundled `.app`; `cargo run` returns [`LoginItemResult::Unsupported`].
//! The switch reads the file back, so it cannot drift from disk.

use std::path::{Path, PathBuf};

use libra_core::LoginItemResult;

use crate::notify::exe_is_app_bundle;

pub fn query(app_id: Option<&str>) -> LoginItemResult {
    let Some(app_id) = app_id.filter(|id| !id.is_empty()) else {
        return LoginItemResult::Unsupported("launch at login needs an app id".into());
    };
    host_query(app_id)
}

pub fn set(app_id: Option<&str>, title: &str, enabled: bool) -> LoginItemResult {
    let Some(app_id) = app_id.filter(|id| !id.is_empty()) else {
        return LoginItemResult::Unsupported("launch at login needs an app id".into());
    };
    let Ok(exe) = std::env::current_exe() else {
        return LoginItemResult::Failed("could not resolve the current executable".into());
    };
    host_set(app_id, title, &exe, enabled)
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn desktop_entry(title: &str, exe: &Path) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName={title}\nExec={}\nX-GNOME-Autostart-enabled=true\n",
        exe.display()
    )
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub fn startup_cmd(exe: &Path) -> String {
    format!("@echo off\r\nstart \"\" \"{}\"\r\n", exe.display())
}

pub fn launch_agent_plist(label: &str, exe: &Path) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>{label}</string>
<key>ProgramArguments</key><array><string>{}</string></array>
<key>RunAtLoad</key><true/>
</dict></plist>
"#,
        exe.display()
    )
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn write_desktop(dir: &Path, app_id: &str, title: &str, exe: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{app_id}.desktop"));
    std::fs::write(&path, desktop_entry(title, exe)).map_err(|e| e.to_string())
}

pub fn remove_named(dir: &Path, file_name: &str) -> Result<(), String> {
    let path = dir.join(file_name);
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

pub fn file_exists(dir: &Path, file_name: &str) -> bool {
    dir.join(file_name).is_file()
}

fn host_query(app_id: &str) -> LoginItemResult {
    let Some(dir) = host_dir() else {
        return LoginItemResult::Unsupported("no home directory for launch at login".into());
    };
    if !host_supported() {
        return LoginItemResult::Unsupported(
            "launch at login needs a bundled .app on macOS".into(),
        );
    }
    if file_exists(&dir, &host_file_name(app_id)) {
        LoginItemResult::Enabled
    } else {
        LoginItemResult::Disabled
    }
}

fn host_set(app_id: &str, title: &str, exe: &Path, enabled: bool) -> LoginItemResult {
    if !host_supported() {
        return LoginItemResult::Unsupported(
            "launch at login needs a bundled .app on macOS".into(),
        );
    }
    let Some(dir) = host_dir() else {
        return LoginItemResult::Unsupported("no home directory for launch at login".into());
    };
    let name = host_file_name(app_id);
    let result = if enabled {
        write_host_file(&dir, app_id, title, exe)
    } else {
        remove_named(&dir, &name)
    };
    match result {
        Ok(()) => host_query(app_id),
        Err(e) => LoginItemResult::Failed(e),
    }
}

fn host_supported() -> bool {
    #[cfg(target_os = "macos")]
    {
        std::env::current_exe()
            .ok()
            .is_some_and(|exe| exe_is_app_bundle(&exe))
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

fn host_file_name(app_id: &str) -> String {
    #[cfg(target_os = "linux")]
    {
        format!("{app_id}.desktop")
    }
    #[cfg(target_os = "windows")]
    {
        format!("{app_id}.cmd")
    }
    #[cfg(target_os = "macos")]
    {
        format!("{app_id}.plist")
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        let _ = app_id;
        "launch-item".into()
    }
}

fn host_dir() -> Option<PathBuf> {
    let base = directories::BaseDirs::new()?;
    #[cfg(target_os = "linux")]
    {
        Some(base.config_dir().join("autostart"))
    }
    #[cfg(target_os = "windows")]
    {
        Some(
            base.config_dir()
                .join("Microsoft/Windows/Start Menu/Programs/Startup"),
        )
    }
    #[cfg(target_os = "macos")]
    {
        Some(base.home_dir().join("Library/LaunchAgents"))
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        let _ = base;
        None
    }
}

fn write_host_file(dir: &Path, app_id: &str, title: &str, exe: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    #[cfg(target_os = "linux")]
    {
        write_desktop(dir, app_id, title, exe)
    }
    #[cfg(target_os = "windows")]
    {
        let _ = title;
        std::fs::write(dir.join(host_file_name(app_id)), startup_cmd(exe))
            .map_err(|e| e.to_string())
    }
    #[cfg(target_os = "macos")]
    {
        let _ = title;
        std::fs::write(
            dir.join(host_file_name(app_id)),
            launch_agent_plist(app_id, exe),
        )
        .map_err(|e| e.to_string())
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        let _ = (dir, app_id, title, exe);
        Err("launch at login is unsupported on this host".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_entry_names_the_executable() {
        let text = desktop_entry("AWS Session", Path::new("/usr/bin/aws-login"));
        assert!(text.contains("Name=AWS Session"));
        assert!(text.contains("Exec=/usr/bin/aws-login"));
        assert!(text.contains("Type=Application"));
    }

    #[test]
    fn writing_and_removing_a_desktop_file_is_the_enabled_bit() {
        let dir = std::env::temp_dir().join(format!(
            "libra-login-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        write_desktop(
            &dir,
            "aws-login",
            "AWS Session",
            Path::new("/usr/bin/aws-login"),
        )
        .unwrap();
        assert!(file_exists(&dir, "aws-login.desktop"));
        let body = std::fs::read_to_string(dir.join("aws-login.desktop")).unwrap();
        assert!(body.contains("Exec=/usr/bin/aws-login"));
        remove_named(&dir, "aws-login.desktop").unwrap();
        assert!(!file_exists(&dir, "aws-login.desktop"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn launch_agent_plist_points_at_the_bundle_executable() {
        let text = launch_agent_plist(
            "dev.libra.aws-login",
            Path::new("/Applications/AWS Session.app/Contents/MacOS/aws-login"),
        );
        assert!(text.contains("<string>dev.libra.aws-login</string>"));
        assert!(text.contains("Contents/MacOS/aws-login"));
        assert!(text.contains("<key>RunAtLoad</key>"));
    }
}
