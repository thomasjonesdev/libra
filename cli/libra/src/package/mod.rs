//! `libra package` (ARCHITECTURE.md §20, PLAN-PACKAGE P3–P6) — platform bundles
//! via cargo-packager.

mod target;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use crate::app::{cargo_target_directory, resolve};
use crate::build::parse_build_args;
use crate::manifest::{
    bundle_icons, identity_toml, load, packager_toml, version_from_cargo_manifest, LibraManifest,
    PackagerSpec,
};

pub use target::parse_package_flags;

pub fn package(args: &[String]) -> ExitCode {
    let (root, package_override, extra) = parse_build_args(args);
    let (target, format, filtered_extra) = parse_package_flags(extra);

    if let Err(e) = target::validate_package_target(&target) {
        eprintln!("{e}");
        return ExitCode::from(2);
    }

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

    let cargo_source = std::fs::read_to_string(&app.cargo_manifest).unwrap_or_default();
    let version = version_from_cargo_manifest(&cargo_source);
    let workspace_root = &app.workspace_root;
    let release_dir = match cargo_target_directory(workspace_root) {
        Ok(target_dir) => target_dir.join("release"),
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };

    if let Some(code) = run_release_build(&app, workspace_root) {
        return code;
    }

    let binary_name = target::release_binary_name(&app.crate_name, &target);
    let binary_path = release_dir.join(&binary_name);
    if !binary_path.is_file() {
        eprintln!(
            "libra package: release binary not found at {}",
            binary_path.display()
        );
        return ExitCode::from(1);
    }

    let formats = target::formats_for_target(&target, &format);
    let config_path = app.app_dir.join("Packager.toml");

    let workspace_manifest = app.workspace_manifest.display();
    let before = format!(
        "cargo build --release --manifest-path {workspace_manifest} -p {} --bin {}",
        app.crate_name, app.crate_name
    );

    let macos_sign = manifest
        .package
        .sign
        .as_ref()
        .and_then(|s| s.macos.as_ref());
    let windows_sign = manifest
        .package
        .sign
        .as_ref()
        .and_then(|s| s.windows.as_ref());

    let identity_resource = match write_identity_resource(&app.app_dir, &manifest, &version) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("libra package: {e}");
            return ExitCode::from(1);
        }
    };

    let spec = PackagerSpec {
        name: app.crate_name.clone(),
        product_name: manifest.app.name.clone(),
        version,
        identifier: manifest.app.bundle_id.clone(),
        app_user_model_id: manifest.app.app_user_model_id.clone(),
        icon: PathBuf::from(manifest.package.icon.clone()),
        icons: bundle_icons(&app.app_dir, &manifest.package.icon),
        binary_name,
        binaries_dir: release_dir,
        out_dir: PathBuf::from("dist"),
        formats,
        before_packaging_command: before,
        identity_resource,
        macos_signing_identity: macos_sign.map(|m| m.identity.clone()),
        macos_notary_profile: macos_sign.and_then(|m| m.notary_profile.clone()),
        macos_provider_short_name: macos_sign.and_then(|m| m.provider_short_name.clone()),
        windows_certificate_thumbprint: windows_sign.and_then(|w| w.thumbprint.clone()),
        update_endpoint: manifest.package.update.as_ref().map(|u| u.endpoint.clone()),
        update_pubkey: manifest.package.update.as_ref().map(|u| u.pubkey.clone()),
    };

    if let Err(e) = std::fs::write(&config_path, packager_toml(&spec, &target)) {
        eprintln!("libra package: write {}: {e}", config_path.display());
        return ExitCode::from(1);
    }

    log_package_plan(&spec, &target, &app.app_dir);

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
            if target == "macos" {
                if let Some(code) = finish_unsigned_macos_bundle(&app.app_dir, &spec) {
                    return code;
                }
            }
            eprintln!(
                "libra package: done — see {}",
                app.app_dir.join("dist").display()
            );
            eprintln!("libra package: local build is the backup — dist/ is ready without GitHub");
            ExitCode::SUCCESS
        }
        Ok(status) => ExitCode::from(status.code().unwrap_or(1) as u8),
        Err(e) => {
            eprintln!("libra package: failed to run cargo packager: {e}");
            ExitCode::from(1)
        }
    }
}

/// cargo-packager leaves a linker-only adhoc signature on the Mach-O with
/// `CSResourcesFileMapped` in Info.plist but no sealed bundle resources — macOS
/// on other machines often reports that as “damaged”. Deep ad-hoc sign the
/// `.app` when no Developer ID identity is configured; rebuild any DMG so the
/// disk image contains the fixed bundle.
#[cfg(target_os = "macos")]
fn finish_unsigned_macos_bundle(app_dir: &Path, spec: &PackagerSpec) -> Option<ExitCode> {
    if spec.macos_signing_identity.is_some() {
        return None;
    }
    let app_path = app_dir
        .join("dist")
        .join(format!("{}.app", spec.product_name));
    if !app_path.is_dir() {
        return None;
    }
    eprintln!(
        "libra package: ad-hoc signing {} (unsigned builds)",
        app_path.display()
    );
    if let Err(e) = adhoc_sign_macos_app(&app_path, &spec.identifier) {
        eprintln!("libra package: {e}");
        return Some(ExitCode::from(1));
    }
    if spec.formats.iter().any(|f| f == "dmg") {
        if let Err(e) = rebuild_macos_dmgs(&app_path, &app_dir.join("dist"), &spec.product_name) {
            eprintln!("libra package: {e}");
            return Some(ExitCode::from(1));
        }
    }
    eprintln!(
        "libra package: hint — Gatekeeper on other Macs may still require Right-click → Open for unsigned builds; use [package.sign.macos] for distribution"
    );
    None
}

#[cfg(not(target_os = "macos"))]
fn finish_unsigned_macos_bundle(_app_dir: &Path, _spec: &PackagerSpec) -> Option<ExitCode> {
    None
}

#[cfg(target_os = "macos")]
fn adhoc_sign_macos_app(app_path: &Path, bundle_id: &str) -> Result<(), String> {
    let sign = Command::new("codesign")
        .arg("--force")
        .arg("--deep")
        .arg("--sign")
        .arg("-")
        .arg("--identifier")
        .arg(bundle_id)
        .arg(app_path)
        .output()
        .map_err(|e| format!("codesign: {e}"))?;
    if !sign.status.success() {
        let stderr = String::from_utf8_lossy(&sign.stderr);
        return Err(format!(
            "codesign failed on {}: {stderr}",
            app_path.display()
        ));
    }
    let verify = Command::new("codesign")
        .arg("--verify")
        .arg("--deep")
        .arg("--strict")
        .arg(app_path)
        .output()
        .map_err(|e| format!("codesign --verify: {e}"))?;
    if !verify.status.success() {
        let stderr = String::from_utf8_lossy(&verify.stderr);
        return Err(format!(
            "codesign verify failed on {}: {stderr}",
            app_path.display()
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn rebuild_macos_dmgs(app_path: &Path, dist_dir: &Path, volume_name: &str) -> Result<(), String> {
    let entries =
        std::fs::read_dir(dist_dir).map_err(|e| format!("read {}: {e}", dist_dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read dist entry: {e}"))?;
        let dmg_path = entry.path();
        if dmg_path.extension().is_none_or(|ext| ext != "dmg") {
            continue;
        }
        eprintln!(
            "libra package: rebuilding {} with signed app",
            dmg_path.display()
        );
        std::fs::remove_file(&dmg_path)
            .map_err(|e| format!("remove {}: {e}", dmg_path.display()))?;
        let status = Command::new("hdiutil")
            .arg("create")
            .arg("-volname")
            .arg(volume_name)
            .arg("-srcfolder")
            .arg(app_path)
            .arg("-ov")
            .arg("-format")
            .arg("UDZO")
            .arg(&dmg_path)
            .status()
            .map_err(|e| format!("hdiutil: {e}"))?;
        if !status.success() {
            return Err(format!("hdiutil create failed for {}", dmg_path.display()));
        }
    }
    Ok(())
}

fn log_package_plan(spec: &PackagerSpec, target: &str, app_dir: &Path) {
    eprintln!(
        "libra package: {} v{} → {} ({target})",
        spec.product_name,
        spec.version,
        app_dir.join("dist").display()
    );
    eprintln!("  bundle id: {}", spec.identifier);
    eprintln!("  formats: {}", spec.formats.join(", "));
    if target == "windows" {
        eprintln!("  app user model id: {}", spec.app_user_model_id);
    }
    if spec.macos_signing_identity.is_some() {
        eprintln!("  signing: macOS codesign + notarization (when credentials are available)");
        if let Some(profile) = &spec.macos_notary_profile {
            eprintln!("  notary profile: {profile} (→ APPLE_KEYCHAIN_PROFILE)");
        } else {
            eprintln!(
                "  hint: add [package.sign.macos].notary_profile or set APPLE_KEYCHAIN_PROFILE"
            );
        }
    }
    if spec.windows_certificate_thumbprint.is_some() {
        eprintln!("  signing: Windows Authenticode (certificate-thumbprint in Packager.toml)");
    }
    eprintln!(
        "  identity resource: {} (PLAN-NOTIFY NT3 / RESIDENCY R5)",
        spec.identity_resource.display()
    );
    eprintln!("  icon: {}", spec.icon.display());
    if spec.icons.len() > 1 {
        eprintln!(
            "  icons: {}",
            spec.icons
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if let Some(endpoint) = &spec.update_endpoint {
        eprintln!("  update feed: {endpoint}");
        if spec.update_pubkey.is_some() {
            eprintln!("  update pubkey: bundled in the identity resource");
        }
    }
    eprintln!(
        "  local backup: this command writes dist/ on this machine; libra publish uploads it"
    );
    eprintln!("  config: {}", app_dir.join("Packager.toml").display());
}

/// Bundled identity metadata for native integrations (PLAN-PACKAGE P6, PLAN-NOTIFY NT3).
fn write_identity_resource(
    app_dir: &Path,
    manifest: &LibraManifest,
    version: &str,
) -> Result<PathBuf, String> {
    let dir = app_dir.join("packaging");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let path = dir.join("libra-identity.toml");
    let body = identity_toml(manifest, version);
    std::fs::write(&path, body).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(path)
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
