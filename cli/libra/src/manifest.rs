//! `libra.toml` v1 parser (PLAN-PACKAGE P2).
//!
//! Identity + packaging fields only; window/title stay on `libra::app!`.

use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LibraManifest {
    pub app: AppSection,
    pub package: PackageSection,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AppSection {
    pub name: String,
    pub bundle_id: String,
    pub app_user_model_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PackageSection {
    pub icon: String,
    #[serde(default)]
    pub sign: Option<SignSection>,
    /// GitHub release upload. Absent means `libra package` is the whole path.
    #[serde(default)]
    pub publish: Option<PublishSection>,
    /// Signed `latest.json` feed. Absent means the shell never checks.
    #[serde(default)]
    pub update: Option<UpdateSection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PublishSection {
    pub provider: String,
    pub repo: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct UpdateSection {
    pub endpoint: String,
    pub pubkey: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct SignSection {
    pub macos: Option<MacOsSignSection>,
    pub windows: Option<WindowsSignSection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MacOsSignSection {
    pub identity: String,
    /// Keychain profile from `xcrun notarytool store-credentials` — forwarded
    /// as `APPLE_KEYCHAIN_PROFILE` when running cargo-packager.
    #[serde(default)]
    pub notary_profile: Option<String>,
    /// Apple provider short name; written to Packager.toml `[macos]`.
    #[serde(default)]
    pub provider_short_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct WindowsSignSection {
    pub thumbprint: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PackagerSpec {
    pub name: String,
    pub product_name: String,
    pub version: String,
    pub identifier: String,
    pub app_user_model_id: String,
    pub icon: PathBuf,
    /// PNG plus generated `.icns` / `.ico` when `libra icon` has written them.
    pub icons: Vec<PathBuf>,
    pub binary_name: String,
    pub binaries_dir: PathBuf,
    pub out_dir: PathBuf,
    pub formats: Vec<String>,
    pub before_packaging_command: String,
    /// Relative path (from app dir) bundled into every installer format.
    pub identity_resource: PathBuf,
    pub macos_signing_identity: Option<String>,
    pub macos_notary_profile: Option<String>,
    pub macos_provider_short_name: Option<String>,
    pub windows_certificate_thumbprint: Option<String>,
    pub update_endpoint: Option<String>,
    pub update_pubkey: Option<String>,
}

/// Load and validate `libra.toml` beside an app crate.
pub fn load(app_dir: &Path) -> Result<LibraManifest, String> {
    let path = app_dir.join("libra.toml");
    let source = std::fs::read_to_string(&path).map_err(|e| {
        format!(
            "{}: {e} (required for libra package — see PLAN-PACKAGE P2)",
            path.display()
        )
    })?;
    let manifest: LibraManifest =
        toml::from_str(&source).map_err(|e| format!("{}: parse error: {e}", path.display()))?;
    validate(&manifest, app_dir)?;
    Ok(manifest)
}

/// Optional icon path from `libra.toml` — used by `libra dev` (no error when absent).
pub fn try_icon_path(app_dir: &Path) -> Option<PathBuf> {
    let path = app_dir.join("libra.toml");
    let source = std::fs::read_to_string(&path).ok()?;
    let manifest: LibraManifest = toml::from_str(&source).ok()?;
    let icon = app_dir.join(&manifest.package.icon);
    icon.is_file().then_some(icon)
}

fn validate(manifest: &LibraManifest, app_dir: &Path) -> Result<(), String> {
    if manifest.app.name.trim().is_empty() {
        return Err("libra.toml [app].name must not be empty".into());
    }
    if manifest.app.bundle_id.trim().is_empty() {
        return Err("libra.toml [app].bundle_id must not be empty".into());
    }
    if manifest.app.app_user_model_id.trim().is_empty() {
        return Err("libra.toml [app].app_user_model_id must not be empty".into());
    }
    let icon = app_dir.join(&manifest.package.icon);
    if !icon.is_file() {
        return Err(format!(
            "libra.toml [package].icon not found: {}",
            icon.display()
        ));
    }
    if let Some(publish) = &manifest.package.publish {
        if publish.provider != "github" {
            return Err(format!(
                "libra.toml [package.publish].provider must be \"github\" (got {:?})",
                publish.provider
            ));
        }
        if !publish.repo.contains('/') || publish.repo.starts_with('/') {
            return Err("libra.toml [package.publish].repo must look like owner/name".into());
        }
    }
    if let Some(update) = &manifest.package.update {
        if !update.endpoint.starts_with("https://") {
            return Err("libra.toml [package.update].endpoint must be an https URL".into());
        }
        if update.pubkey.trim().is_empty() {
            return Err("libra.toml [package.update].pubkey must not be empty".into());
        }
    }
    Ok(())
}

/// Icon files cargo-packager should see. The configured PNG is always
/// included. `libra icon` writes `icon.icns` and `icon.ico` under
/// `<icon-dir>/icons/`; those are added when the files exist.
pub fn bundle_icons(app_dir: &Path, icon: &str) -> Vec<PathBuf> {
    let mut icons = vec![PathBuf::from(icon)];
    let icons_dir = app_dir.join(icon).parent().unwrap_or(app_dir).join("icons");
    for name in ["icon.icns", "icon.ico"] {
        let path = icons_dir.join(name);
        if path.is_file() {
            icons.push(match path.strip_prefix(app_dir) {
                Ok(rel) => rel.to_path_buf(),
                Err(_) => path,
            });
        }
    }
    icons
}

/// Bundled identity resource (PLAN-PACKAGE P6, PLAN-RELEASE).
pub fn identity_toml(manifest: &LibraManifest, version: &str) -> String {
    let mut body = format!(
        r#"# @generated by libra package — do not edit.
# Consumed by native shell integrations (PLAN-NOTIFY NT3, PLAN-RESIDENCY R5)
# and, when the update fields are set, by the shell's update check (PLAN-RELEASE).

app_user_model_id = {app_user_model_id:?}
bundle_id = {bundle_id:?}
product_name = {product_name:?}
version = {version:?}
"#,
        app_user_model_id = manifest.app.app_user_model_id,
        bundle_id = manifest.app.bundle_id,
        product_name = manifest.app.name,
        version = version,
    );
    if let Some(update) = &manifest.package.update {
        body.push_str(&format!(
            "update_endpoint = {:?}\nupdate_pubkey = {:?}\n",
            update.endpoint, update.pubkey
        ));
    }
    body
}

/// Map `libra.toml` + crate metadata into a cargo-packager config file body.
pub fn packager_toml(spec: &PackagerSpec, target: &str) -> String {
    let formats = spec
        .formats
        .iter()
        .map(|f| format!("\"{f}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let icons = spec
        .icons
        .iter()
        .map(|p| format!("{:?}", p.display().to_string()))
        .collect::<Vec<_>>()
        .join(", ");
    let resource = spec.identity_resource.display().to_string();
    let mut out = format!(
        r#"# @generated by libra package — do not edit.
name = {name:?}
product-name = {product_name:?}
version = {version:?}
identifier = {identifier:?}
out-dir = {out_dir:?}
binaries-dir = {binaries_dir:?}
before-packaging-command = {before:?}
formats = [{formats}]
icons = [{icons}]
resources = [{resource:?}]

[[binaries]]
path = {binary:?}
main = true
"#,
        name = spec.name,
        product_name = spec.product_name,
        version = spec.version,
        identifier = spec.identifier,
        out_dir = spec.out_dir.display().to_string(),
        binaries_dir = spec.binaries_dir.display().to_string(),
        before = spec.before_packaging_command,
        formats = formats,
        icons = icons,
        resource = resource,
        binary = spec.binary_name,
    );

    match target {
        "macos" => append_macos_sections(&mut out, spec),
        "windows" => append_windows_sections(&mut out, spec),
        "linux" => append_linux_sections(&mut out, spec),
        _ => {}
    }
    out
}

fn append_macos_sections(out: &mut String, spec: &PackagerSpec) {
    if let Some(identity) = &spec.macos_signing_identity {
        out.push_str(&format!("\n[macos]\nsigning-identity = {identity:?}"));
        if let Some(provider) = &spec.macos_provider_short_name {
            out.push_str(&format!("\nprovider-short-name = {provider:?}"));
        }
        out.push('\n');
    }
}

fn append_windows_sections(out: &mut String, spec: &PackagerSpec) {
    if let Some(thumb) = &spec.windows_certificate_thumbprint {
        out.push_str(&format!(
            "\n[windows]\ncertificate-thumbprint = {thumb:?}\n"
        ));
    }
    out.push_str(
        r#"
[nsis]
appdata-paths = ["$LOCALAPPDATA/$PUBLISHER/$PRODUCTNAME"]
"#,
    );
    let _ = &spec.app_user_model_id;
}

fn append_linux_sections(out: &mut String, spec: &PackagerSpec) {
    let _ = &spec.app_user_model_id;
    out.push_str(&format!(
        r#"
[deb]
section = "utils"
priority = "optional"
package-name = {name:?}

[linux]
appcategory = "Utility"
"#,
        name = spec.name,
    ));
}

pub fn version_from_cargo_manifest(source: &str) -> String {
    for line in source.lines() {
        let line = line.trim();
        if line.starts_with("version.workspace") {
            return "0.0.1".into();
        }
        if let Some(rest) = line.strip_prefix("version = ") {
            return rest.trim().trim_matches('"').to_string();
        }
    }
    "0.0.1".into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parses_minimal_manifest() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("icon.png"), b"png").unwrap();
        fs::write(
            dir.path().join("libra.toml"),
            r#"
[app]
name = "AWS Login"
bundle_id = "dev.libra.aws-login"
app_user_model_id = "Libra.AwsLogin"

[package]
icon = "icon.png"
"#,
        )
        .unwrap();
        let m = load(dir.path()).unwrap();
        assert_eq!(m.app.name, "AWS Login");
        assert_eq!(m.app.bundle_id, "dev.libra.aws-login");
    }

    fn packager_spec() -> PackagerSpec {
        PackagerSpec {
            name: "aws-login".into(),
            product_name: "AWS Login".into(),
            version: "0.0.1".into(),
            identifier: "dev.libra.aws-login".into(),
            app_user_model_id: "Libra.AwsLogin".into(),
            icon: PathBuf::from("assets/icon.png"),
            icons: vec![PathBuf::from("assets/icon.png")],
            binary_name: "aws-login".into(),
            binaries_dir: PathBuf::from("/tmp/target/release"),
            out_dir: PathBuf::from("dist"),
            formats: vec!["app".into(), "dmg".into()],
            before_packaging_command: "cargo build --release -p aws-login --bin aws-login".into(),
            identity_resource: PathBuf::from("packaging/libra-identity.toml"),
            macos_signing_identity: None,
            macos_notary_profile: None,
            macos_provider_short_name: None,
            windows_certificate_thumbprint: None,
            update_endpoint: None,
            update_pubkey: None,
        }
    }

    #[test]
    fn packager_toml_includes_macos_signing() {
        let mut spec = packager_spec();
        spec.macos_signing_identity = Some("Developer ID Application: Example (TEAM)".into());
        spec.macos_notary_profile = Some("libra-notary".into());
        spec.macos_provider_short_name = Some("ExampleTeam".into());
        let body = packager_toml(&spec, "macos");
        assert!(body.contains("signing-identity = \"Developer ID Application: Example (TEAM)\""));
        assert!(body.contains("provider-short-name = \"ExampleTeam\""));
        assert!(body.contains("resources = [\"packaging/libra-identity.toml\"]"));
    }

    #[test]
    fn packager_toml_includes_windows_sections() {
        let mut spec = packager_spec();
        spec.formats = vec!["nsis".into()];
        spec.binary_name = "aws-login.exe".into();
        spec.windows_certificate_thumbprint = Some("ABCD1234".into());
        let body = packager_toml(&spec, "windows");
        assert!(body.contains("certificate-thumbprint = \"ABCD1234\""));
        assert!(body.contains("[nsis]"));
        assert!(body.contains("appdata-paths"));
        assert!(body.contains("path = \"aws-login.exe\""));
    }

    #[test]
    fn packager_toml_includes_linux_sections() {
        let mut spec = packager_spec();
        spec.formats = vec!["deb".into(), "appimage".into()];
        let body = packager_toml(&spec, "linux");
        assert!(body.contains("[deb]"));
        assert!(body.contains("package-name = \"aws-login\""));
        assert!(body.contains("[linux]"));
        assert!(body.contains("appcategory = \"Utility\""));
    }

    #[test]
    fn try_icon_path_reads_manifest() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("icon.png"), b"png").unwrap();
        fs::write(
            dir.path().join("libra.toml"),
            r#"
[app]
name = "X"
bundle_id = "a.b"
app_user_model_id = "A.B"

[package]
icon = "icon.png"
"#,
        )
        .unwrap();
        assert_eq!(try_icon_path(dir.path()), Some(dir.path().join("icon.png")));
    }

    #[test]
    fn missing_icon_errors() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("libra.toml"),
            r#"
[app]
name = "X"
bundle_id = "a.b"
app_user_model_id = "A.B"

[package]
icon = "missing.png"
"#,
        )
        .unwrap();
        assert!(load(dir.path()).is_err());
    }

    #[test]
    fn bundle_icons_includes_generated_icns_and_ico() {
        let dir = tempfile::tempdir().unwrap();
        let icons = dir.path().join("assets/icons");
        fs::create_dir_all(&icons).unwrap();
        fs::write(dir.path().join("assets/icon.png"), b"png").unwrap();
        fs::write(icons.join("icon.icns"), b"icns").unwrap();
        fs::write(icons.join("icon.ico"), b"ico").unwrap();
        let found = bundle_icons(dir.path(), "assets/icon.png");
        assert_eq!(
            found,
            vec![
                PathBuf::from("assets/icon.png"),
                PathBuf::from("assets/icons/icon.icns"),
                PathBuf::from("assets/icons/icon.ico"),
            ]
        );
    }

    #[test]
    fn identity_toml_embeds_the_update_feed() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("icon.png"), b"png").unwrap();
        fs::write(
            dir.path().join("libra.toml"),
            r#"
[app]
name = "Lima"
bundle_id = "dev.libra.lima"
app_user_model_id = "Libra.Lima"

[package]
icon = "icon.png"

[package.publish]
provider = "github"
repo = "owner/lima"

[package.update]
endpoint = "https://github.com/owner/lima/releases/latest/download/latest.json"
pubkey = "PUBKEY"
"#,
        )
        .unwrap();
        let m = load(dir.path()).unwrap();
        let body = identity_toml(&m, "0.2.0");
        assert!(body.contains("version = \"0.2.0\""));
        assert!(body.contains("update_endpoint = \"https://github.com/owner/lima/releases/latest/download/latest.json\""));
        assert!(body.contains("update_pubkey = \"PUBKEY\""));
    }

    #[test]
    fn packager_toml_lists_every_icon() {
        let mut spec = packager_spec();
        spec.icons.push(PathBuf::from("assets/icons/icon.icns"));
        spec.icons.push(PathBuf::from("assets/icons/icon.ico"));
        let body = packager_toml(&spec, "macos");
        assert!(body.contains("assets/icon.png"));
        assert!(body.contains("assets/icons/icon.icns"));
        assert!(body.contains("assets/icons/icon.ico"));
    }
}
