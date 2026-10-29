//! `libra publish` (ARCHITECTURE.md §20, PLAN-RELEASE).
//!
//! Uploads a bundle `libra package` already built. `libra package` stays
//! the local backup: it never calls `gh` and still notarizes from the
//! keychain when `[package.sign.macos]` is set.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use crate::app::resolve;
use crate::build::parse_build_args;
use crate::manifest::{version_from_cargo_manifest, LibraManifest};
use crate::package::package;

const CLI_VERSION: &str = env!("CARGO_PKG_VERSION");

struct Flags {
    ci: bool,
    no_upload: bool,
    from_dist: bool,
    keygen: bool,
    notes: Option<String>,
    rest: Vec<String>,
}

pub fn publish(args: &[String]) -> ExitCode {
    let flags = parse_flags(args);
    if flags.keygen {
        return keygen();
    }

    let (root, package_override, extra) = parse_build_args(&flags.rest);
    if !extra.is_empty() {
        eprintln!("libra publish: unknown argument {}", extra.first().unwrap());
        usage();
        return ExitCode::from(2);
    }

    let app = match resolve(root, package_override) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            eprintln!("hint: `libra publish -p <crate>` or run it from the app directory");
            eprintln!("local backup: libra package");
            return ExitCode::from(2);
        }
    };
    let manifest = match crate::manifest::load(&app.app_dir) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };

    if manifest.package.publish.is_none() {
        eprintln!("libra publish: [package.publish] is missing from libra.toml");
        eprintln!("local backup: libra package");
        return ExitCode::from(2);
    }

    if flags.ci {
        let workdir = workflow_workdir(&app.app_dir);
        let body = release_workflow(&manifest, &workdir);
        let path = app.app_dir.join(".github/workflows/release.yml");
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                eprintln!("libra publish: create {}: {e}", parent.display());
                return ExitCode::from(1);
            }
        }
        if let Err(e) = std::fs::write(&path, &body) {
            eprintln!("libra publish: write {}: {e}", path.display());
            return ExitCode::from(1);
        }
        eprintln!("libra publish: wrote {}", path.display());
        eprintln!("libra publish: local backup is still `libra package` on this Mac");
        eprintln!("  that writes dist/ without GitHub, including notarization when the keychain profile is set");
        return ExitCode::SUCCESS;
    }

    let publish = manifest.package.publish.clone().expect("checked above");

    if !flags.from_dist {
        let code = package(&flags.rest);
        if code != ExitCode::SUCCESS {
            return code;
        }
    }

    let cargo = std::fs::read_to_string(&app.cargo_manifest).unwrap_or_default();
    let version = version_from_cargo_manifest(&cargo);
    let dist = app.app_dir.join("dist");
    if !dist.is_dir() {
        eprintln!("libra publish: {} is missing", dist.display());
        eprintln!("local backup: libra package");
        return ExitCode::from(1);
    }

    let notes = flags
        .notes
        .clone()
        .unwrap_or_else(|| format!("Release {version}"));
    let prepared = match prepare_feed(&app.app_dir, &manifest, &version, &notes, &publish.repo) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("libra publish: {e}");
            return ExitCode::from(1);
        }
    };

    if flags.no_upload {
        eprintln!("libra publish: wrote the local feed and stopped before GitHub");
        for path in &prepared {
            eprintln!("  {}", path.display());
        }
        eprintln!(
            "local backup of the bundle itself: the files already in dist/ from libra package"
        );
        return ExitCode::SUCCESS;
    }

    if !has_gh() {
        eprintln!("libra publish: gh not found");
        eprintln!("install: https://cli.github.com/ then `gh auth login`");
        eprintln!("local backup: the bundle is already in {}", dist.display());
        return ExitCode::from(2);
    }

    let tag = format!("v{version}");
    match upload(&publish.repo, &tag, &notes, &prepared) {
        Ok(()) => {
            eprintln!("libra publish: uploaded {tag} to {}", publish.repo);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("libra publish: {e}");
            eprintln!("local backup: the bundle is still in {}", dist.display());
            ExitCode::from(1)
        }
    }
}

fn usage() {
    eprintln!("usage:");
    eprintln!("  libra publish [path] [-p crate] [--notes text]");
    eprintln!("  libra publish --no-upload     write latest.json next to dist/ and stop");
    eprintln!("  libra publish --from-dist     upload an existing dist/ (what CI runs)");
    eprintln!("  libra publish --ci            write .github/workflows/release.yml and stop");
    eprintln!("  libra publish --keygen        print an update keypair");
    eprintln!("local backup: libra package");
}

fn parse_flags(args: &[String]) -> Flags {
    let mut flags = Flags {
        ci: false,
        no_upload: false,
        from_dist: false,
        keygen: false,
        notes: None,
        rest: Vec::new(),
    };
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--ci" => flags.ci = true,
            "--no-upload" => flags.no_upload = true,
            "--from-dist" => flags.from_dist = true,
            "--keygen" => flags.keygen = true,
            "--notes" => flags.notes = iter.next().cloned(),
            other => flags.rest.push(other.to_string()),
        }
    }
    flags
}

fn keygen() -> ExitCode {
    match generate_keypair() {
        Ok((public, secret)) => {
            println!("pubkey (put this in [package.update].pubkey):");
            println!("{public}");
            println!();
            println!("secret (LIBRA_UPDATE_KEY — do not commit this):");
            println!("{secret}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("libra publish: {e}");
            ExitCode::from(1)
        }
    }
}

pub fn generate_keypair() -> Result<(String, String), String> {
    // Empty password keeps the key in the standard minisign box without
    // prompting. The box itself is the secret (`LIBRA_UPDATE_KEY`).
    let pair = minisign::KeyPair::generate_encrypted_keypair(Some(String::new()))
        .map_err(|e| e.to_string())?;
    let public = pair.pk.to_base64();
    let secret = pair.sk.to_box(None).map_err(|e| e.to_string())?.to_string();
    Ok((public, secret))
}

pub fn sign_bytes(secret: &str, bytes: &[u8]) -> Result<String, String> {
    let sk = load_secret(secret)?;
    let signature = minisign::sign(
        None,
        &sk,
        std::io::Cursor::new(bytes),
        None,
        Some("libra update".into()),
    )
    .map_err(|e| e.to_string())?;
    Ok(signature.to_string())
}

fn load_secret(raw: &str) -> Result<minisign::SecretKey, String> {
    let boxed = minisign::SecretKeyBox::from_string(raw.trim()).map_err(|e| e.to_string())?;
    boxed
        .into_secret_key(Some(String::new()))
        .map_err(|e| e.to_string())
}

/// Files to upload. The archive signature is inside `latest.json`.
pub fn prepare_feed(
    app_dir: &Path,
    manifest: &LibraManifest,
    version: &str,
    notes: &str,
    repo: &str,
) -> Result<Vec<PathBuf>, String> {
    let dist = app_dir.join("dist");
    let mut files = dist_artifacts(&dist)?;
    let Some(update) = &manifest.package.update else {
        if files.is_empty() {
            return Err(format!(
                "no installer files in {} — run libra package first",
                dist.display()
            ));
        }
        eprintln!("libra publish: no [package.update]; uploading the bundle without a feed");
        return Ok(files);
    };
    let secret = std::env::var("LIBRA_UPDATE_KEY").map_err(|_| {
        "LIBRA_UPDATE_KEY is not set (run `libra publish --keygen`, then export the secret). The local bundle is unchanged.".to_string()
    })?;
    let app = find_app_bundle(&dist)?;
    let archive_name = format!(
        "{}.tar.gz",
        app.file_name().and_then(|s| s.to_str()).unwrap_or("app")
    );
    let archive = dist.join(&archive_name);
    archive_app(&app, &archive)?;
    let bytes = std::fs::read(&archive).map_err(|e| format!("read {}: {e}", archive.display()))?;
    let signature = sign_bytes(&secret, &bytes)?;
    let target = feed_target();
    let url = format!("https://github.com/{repo}/releases/download/v{version}/{archive_name}");
    let json = latest_json(version, notes, target, &url, &signature);
    let json_path = dist.join("latest.json");
    std::fs::write(&json_path, json).map_err(|e| format!("write {}: {e}", json_path.display()))?;
    files.push(archive);
    files.push(json_path);
    let _ = update.pubkey;
    Ok(files)
}

pub fn latest_json(version: &str, notes: &str, target: &str, url: &str, signature: &str) -> String {
    let body = serde_json::json!({
        "version": version,
        "notes": notes,
        "pub_date": chrono_less_now(),
        "platforms": {
            target: {
                "url": url,
                "signature": signature,
            }
        }
    });
    serde_json::to_string_pretty(&body).unwrap_or_else(|_| "{}".into())
}

fn chrono_less_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_rfc3339(secs)
}

fn format_rfc3339(secs: u64) -> String {
    let days = secs / 86_400;
    let tod = secs % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = tod / 3600;
    let min = (tod % 3600) / 60;
    let sec = tod % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}Z")
}

fn civil_from_days(days: u64) -> (i32, u32, u32) {
    // Howard Hinnant civil_from_days, shifted to the Unix epoch.
    let z = days as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

fn feed_target() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "darwin-aarch64",
        ("macos", "x86_64") => "darwin-x86_64",
        ("windows", "aarch64") => "windows-aarch64",
        ("windows", _) => "windows-x86_64",
        (_, "aarch64") => "linux-aarch64",
        _ => "linux-x86_64",
    }
}

fn dist_artifacts(dist: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    let entries = std::fs::read_dir(dist).map_err(|e| format!("read {}: {e}", dist.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if name == "latest.json" || name.ends_with(".tar.gz") {
            continue;
        }
        if path.is_file() {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn find_app_bundle(dist: &Path) -> Result<PathBuf, String> {
    let entries = std::fs::read_dir(dist).map_err(|e| format!("read {}: {e}", dist.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().and_then(|e| e.to_str()) == Some("app") && path.is_dir() {
            return Ok(path);
        }
    }
    Err(format!(
        "no .app in {} — the update archive is built from the macOS bundle",
        dist.display()
    ))
}

fn archive_app(app: &Path, dest: &Path) -> Result<(), String> {
    let parent = app.parent().unwrap_or_else(|| Path::new("."));
    let name = app.file_name().unwrap_or_default();
    let status = Command::new("tar")
        .env("COPYFILE_DISABLE", "1")
        .arg("-czf")
        .arg(dest)
        .arg("-C")
        .arg(parent)
        .arg(name)
        .status()
        .map_err(|e| format!("tar: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("tar exited with {status}"))
    }
}

pub fn gh_create_argv(repo: &str, tag: &str, notes: &str, files: &[PathBuf]) -> Vec<String> {
    let mut argv = vec![
        "release".into(),
        "create".into(),
        tag.into(),
        "--repo".into(),
        repo.into(),
        "--title".into(),
        tag.into(),
        "--notes".into(),
        notes.into(),
    ];
    for file in files {
        argv.push(file.display().to_string());
    }
    argv
}

pub fn gh_upload_argv(repo: &str, tag: &str, files: &[PathBuf]) -> Vec<String> {
    let mut argv = vec![
        "release".into(),
        "upload".into(),
        tag.into(),
        "--repo".into(),
        repo.into(),
        "--clobber".into(),
    ];
    for file in files {
        argv.push(file.display().to_string());
    }
    argv
}

fn upload(repo: &str, tag: &str, notes: &str, files: &[PathBuf]) -> Result<(), String> {
    let create = gh_create_argv(repo, tag, notes, files);
    let status = gh(&create)?;
    if status {
        return Ok(());
    }
    eprintln!("libra publish: release {tag} already exists; uploading over it");
    let upload = gh_upload_argv(repo, tag, files);
    if gh(&upload)? {
        Ok(())
    } else {
        Err(format!("gh release upload failed for {tag}"))
    }
}

fn gh(args: &[String]) -> Result<bool, String> {
    let status = Command::new("gh")
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| format!("gh: {e}"))?;
    Ok(status.success())
}

fn has_gh() -> bool {
    Command::new("gh")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn workflow_workdir(app_dir: &Path) -> String {
    let mut dir = app_dir.to_path_buf();
    let app = dir.clone();
    loop {
        if dir.join(".git").exists() {
            return app
                .strip_prefix(&dir)
                .ok()
                .filter(|p| !p.as_os_str().is_empty())
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| ".".into());
        }
        if !dir.pop() {
            return ".".into();
        }
    }
}

/// GitHub Actions workflow. The runner calls `libra package` (the same
/// local command) and then `libra publish --from-dist`.
pub fn release_workflow(manifest: &LibraManifest, workdir: &str) -> String {
    let notary = manifest
        .package
        .sign
        .as_ref()
        .and_then(|s| s.macos.as_ref())
        .and_then(|m| m.notary_profile.clone());
    let signed = manifest
        .package
        .sign
        .as_ref()
        .and_then(|s| s.macos.as_ref())
        .is_some();
    let updates = manifest.package.update.is_some();
    let mut body = format!(
        r#"# Generated by `libra publish --ci`. Do not edit by hand; re-run the command.
# Local backup: `libra package` on a Mac writes dist/ without GitHub.
# `libra publish --no-upload` writes latest.json next to that bundle.
name: release
on:
  push:
    tags:
      - "v*"
permissions:
  contents: write
jobs:
  macos:
    runs-on: macos-latest
    defaults:
      run:
        working-directory: {workdir}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Install cargo-packager
        run: cargo install cargo-packager --locked
      - name: Install libra
        run: cargo install libra-ui-cli --version {version_cli} --locked
"#,
        workdir = workdir,
        version_cli = CLI_VERSION,
    );
    if signed {
        body.push_str(
            r#"
      - name: Import Developer ID
        env:
          APPLE_CERTIFICATE: ${{ secrets.APPLE_CERTIFICATE }}
          APPLE_CERTIFICATE_PASSWORD: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}
        run: |
          echo "$APPLE_CERTIFICATE" | base64 --decode > "$RUNNER_TEMP/cert.p12"
          security create-keychain -p actions build.keychain
          security default-keychain -s build.keychain
          security unlock-keychain -p actions build.keychain
          security import "$RUNNER_TEMP/cert.p12" -k build.keychain -P "$APPLE_CERTIFICATE_PASSWORD" -T /usr/bin/codesign
          security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k actions build.keychain
"#,
        );
    }
    if let Some(profile) = notary {
        body.push_str(&format!(
            r#"
      - name: Store notary credentials
        env:
          APPLE_ID: ${{{{ secrets.APPLE_ID }}}}
          APPLE_APP_PASSWORD: ${{{{ secrets.APPLE_APP_PASSWORD }}}}
          APPLE_TEAM_ID: ${{{{ secrets.APPLE_TEAM_ID }}}}
        run: xcrun notarytool store-credentials "{profile}" --apple-id "$APPLE_ID" --password "$APPLE_APP_PASSWORD" --team-id "$APPLE_TEAM_ID"
"#
        ));
    }
    body.push_str(
        r#"
      - name: Package
        run: libra package
"#,
    );
    if updates {
        body.push_str(
            r#"
      - name: Publish
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          LIBRA_UPDATE_KEY: ${{ secrets.LIBRA_UPDATE_KEY }}
        run: libra publish --from-dist
"#,
        );
    } else {
        body.push_str(
            r#"
      - name: Publish
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: libra publish --from-dist
"#,
        );
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest(update: bool, sign: bool) -> LibraManifest {
        let toml = format!(
            r#"
[app]
name = "Lima"
bundle_id = "dev.libra.lima"
app_user_model_id = "Libra.Lima"

[package]
icon = "icon.png"
{}
{}
"#,
            if sign {
                r#"
[package.sign.macos]
identity = "Developer ID Application: Example (TEAM)"
notary_profile = "libra-notary"
"#
            } else {
                ""
            },
            if update {
                r#"
[package.publish]
provider = "github"
repo = "owner/lima"

[package.update]
endpoint = "https://github.com/owner/lima/releases/latest/download/latest.json"
pubkey = "PUBKEY"
"#
            } else {
                r#"
[package.publish]
provider = "github"
repo = "owner/lima"
"#
            }
        );
        toml::from_str(&toml).unwrap()
    }

    #[test]
    fn latest_json_names_the_archive_and_its_signature() {
        let body = latest_json(
            "0.2.0",
            "notes",
            "darwin-aarch64",
            "https://github.com/owner/lima/releases/download/v0.2.0/Lima.app.tar.gz",
            "SIGNATURE",
        );
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["version"], "0.2.0");
        assert_eq!(v["platforms"]["darwin-aarch64"]["signature"], "SIGNATURE");
        assert!(v["platforms"]["darwin-aarch64"]["url"]
            .as_str()
            .unwrap()
            .ends_with("Lima.app.tar.gz"));
    }

    #[test]
    fn workflow_keeps_local_package_and_bakes_the_notary_profile() {
        let body = release_workflow(&sample_manifest(true, true), ".");
        assert!(body.contains("Local backup: `libra package`"));
        assert!(body.contains("run: libra package"));
        assert!(body.contains("libra publish --from-dist"));
        assert!(body.contains("store-credentials \"libra-notary\""));
        assert!(body.contains("${{ secrets.APPLE_CERTIFICATE }}"));
        assert!(body.contains("${{ secrets.GITHUB_TOKEN }}"));
        assert!(body.contains("LIBRA_UPDATE_KEY"));
        assert!(body.contains(&format!("libra-ui-cli --version {CLI_VERSION}")));
    }

    #[test]
    fn workflow_without_signing_omits_notarytool() {
        let body = release_workflow(&sample_manifest(false, false), "apps/lima");
        assert!(!body.contains("notarytool"));
        assert!(!body.contains("LIBRA_UPDATE_KEY"));
        assert!(body.contains("working-directory: apps/lima"));
        assert!(body.contains("run: libra package"));
    }

    #[test]
    fn gh_argv_names_the_repo_tag_and_files() {
        let files = vec![
            PathBuf::from("dist/Lima.dmg"),
            PathBuf::from("dist/latest.json"),
        ];
        let argv = gh_create_argv("owner/lima", "v0.2.0", "notes", &files);
        assert_eq!(argv[0], "release");
        assert_eq!(argv[1], "create");
        assert_eq!(argv[2], "v0.2.0");
        assert!(argv.iter().any(|a| a == "owner/lima"));
        assert!(argv.iter().any(|a| a.ends_with("latest.json")));
        let upload = gh_upload_argv("owner/lima", "v0.2.0", &files);
        assert!(upload.iter().any(|a| a == "--clobber"));
    }

    #[test]
    fn keygen_signature_roundtrip_starts_with_the_minisign_comment() {
        let (public, secret) = generate_keypair().unwrap();
        assert!(!public.is_empty());
        let signature = sign_bytes(&secret, b"hello").unwrap();
        assert!(signature.contains("untrusted comment:"));
        let _ = public;
    }
}
