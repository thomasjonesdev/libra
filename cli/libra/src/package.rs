//! `libra package` (ARCHITECTURE.md §20, PLAN-PACKAGE P3) — platform bundles
//! via cargo-packager.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use crate::app::{cargo_target_directory, resolve};
use crate::build::parse_build_args;
use crate::manifest::{load, packager_toml, version_from_cargo_manifest, PackagerSpec};

pub fn package(args: &[String]) -> ExitCode {
    let (root, package_override, extra) = parse_build_args(args);
    let (target, format, filtered_extra) = parse_package_flags(extra);

    let app = match resolve(root, package_override) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            eprintln!("hint: `libra package -p aws-login` or pass an app path");
            return ExitCode::from(2);
        }
    };

    let manifest = match load(&app.app_dir) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };

    if !has_cargo_packager() {
        eprintln!("libra package: cargo-packager not found");
        eprintln!("install: cargo install cargo-packager --locked");
        return ExitCode::from(2);
    }

    let cargo_source = std::fs::read_to_string(&app.cargo_manifest)
        .unwrap_or_default();
    let version = version_from_cargo_manifest(&cargo_source);
    let workspace_root = &app.workspace_root;
    let release_dir = match cargo_target_directory(workspace_root) {
        Ok(target) => target.join("release"),
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };

    if let Some(code) = run_release_build(&app, workspace_root) {
        return code;
    }

    let binary_path = release_dir.join(&app.crate_name);
    if !binary_path.is_file() {
        eprintln!(
            "libra package: release binary not found at {}",
            binary_path.display()
        );
        return ExitCode::from(1);
    }

    let formats = formats_for_host(&target, &format);

    let config_path = app.app_dir.join("Packager.toml");

    let workspace_manifest = app.workspace_manifest.display();
    let before = format!(
        "cargo build --release --manifest-path {workspace_manifest} -p {} --bin {}",
        app.crate_name, app.crate_name
    );

    let macos_sign = manifest.package.sign.as_ref().and_then(|s| s.macos.as_ref());

    let spec = PackagerSpec {
        name: app.crate_name.clone(),
        product_name: manifest.app.name.clone(),
        version,
        identifier: manifest.app.bundle_id.clone(),
        icon: PathBuf::from(manifest.package.icon.clone()),
        binary_name: app.crate_name.clone(),
        binaries_dir: release_dir,
        out_dir: PathBuf::from("dist"),
        formats,
        before_packaging_command: before,
        macos_signing_identity: macos_sign.map(|m| m.identity.clone()),
        macos_notary_profile: macos_sign.and_then(|m| m.notary_profile.clone()),
        macos_provider_short_name: macos_sign.and_then(|m| m.provider_short_name.clone()),
    };

    if let Err(e) = std::fs::write(&config_path, packager_toml(&spec)) {
        eprintln!("libra package: write {}: {e}", config_path.display());
        return ExitCode::from(1);
    }

    eprintln!(
        "libra package: {} v{} → {}",
        spec.product_name,
        spec.version,
        app.app_dir.join("dist").display()
    );
    eprintln!("  bundle id: {}", spec.identifier);
    if spec.macos_signing_identity.is_some() {
        eprintln!("  signing: macOS codesign + notarization (when credentials are available)");
        if let Some(profile) = &spec.macos_notary_profile {
            eprintln!("  notary profile: {profile} (→ APPLE_KEYCHAIN_PROFILE)");
        } else {
            eprintln!("  hint: add [package.sign.macos].notary_profile or set APPLE_KEYCHAIN_PROFILE for notarization");
        }
    }
    eprintln!("  config: {}", config_path.display());

    let mut cmd = Command::new("cargo");
    cmd.arg("packager")
        .arg("--release")
        .arg("--manifest-path")
        .arg(&app.cargo_manifest)
        .args(filtered_extra)
        .current_dir(&app.app_dir)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    if let Some(profile) = &spec.macos_notary_profile {
        cmd.env("APPLE_KEYCHAIN_PROFILE", profile);
    }

    match cmd.status() {
        Ok(status) if status.success() => {
            eprintln!(
                "libra package: done — see {}",
                app.app_dir.join("dist").display()
            );
            ExitCode::SUCCESS
        }
        Ok(status) => ExitCode::from(status.code().unwrap_or(1) as u8),
        Err(e) => {
            eprintln!("libra package: failed to run cargo packager: {e}");
            ExitCode::from(1)
        }
    }
}

fn run_release_build(app: &crate::app::ResolvedApp, workspace_root: &Path) -> Option<ExitCode> {
    eprintln!(
        "libra package: release build of {} (codegen views + LTO)",
        app.crate_name
    );

    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .arg("--release")
        .arg("--manifest-path")
        .arg(&app.workspace_manifest)
        .arg("-p")
        .arg(&app.crate_name)
        .arg("--bin")
        .arg(&app.crate_name)
        .current_dir(workspace_root)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    match cmd.status() {
        Ok(status) if status.success() => None,
        Ok(status) => Some(ExitCode::from(status.code().unwrap_or(1) as u8)),
        Err(e) => {
            eprintln!("libra package: failed to run cargo build: {e}");
            Some(ExitCode::from(1))
        }
    }
}

fn has_cargo_packager() -> bool {
    Command::new("cargo")
        .arg("packager")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn formats_for_host(target: &str, format: &str) -> Vec<String> {
    if format != "default" {
        return vec![format.to_string()];
    }
    match target {
        "macos" => vec!["app".into(), "dmg".into()],
        "windows" => vec!["nsis".into()],
        "linux" => vec!["deb".into(), "appimage".into()],
        _ => vec!["app".into(), "dmg".into()],
    }
}

fn parse_package_flags(args: Vec<String>) -> (String, String, Vec<String>) {
    let mut target = host_target();
    let mut format = "default".to_string();
    let mut out = Vec::new();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--target" => {
                if let Some(v) = iter.next() {
                    target = v;
                }
            }
            s if s.starts_with("--target=") => {
                target = s.trim_start_matches("--target=").to_string();
            }
            "--format" => {
                if let Some(v) = iter.next() {
                    format = v;
                }
            }
            s if s.starts_with("--format=") => {
                format = s.trim_start_matches("--format=").to_string();
            }
            other => out.push(other.to_string()),
        }
    }
    (target, format, out)
}

fn host_target() -> String {
    if cfg!(target_os = "macos") {
        "macos".into()
    } else if cfg!(target_os = "windows") {
        "windows".into()
    } else {
        "linux".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_macos_formats() {
        let f = formats_for_host("macos", "default");
        assert_eq!(f, vec!["app", "dmg"]);
    }

    #[test]
    fn explicit_format() {
        let f = formats_for_host("macos", "app");
        assert_eq!(f, vec!["app"]);
    }
}
