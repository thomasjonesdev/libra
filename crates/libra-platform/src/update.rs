//! Signed update check (ARCHITECTURE.md §20, PLAN-RELEASE).
//!
//! The shell does this. App `update()` never sees it. A packaged app
//! with `update_endpoint` and `update_pubkey` in the bundled identity
//! resource is the only thing that checks. `libra dev` and `cargo run`
//! have no bundle, so they never ask.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateConfig {
    pub version: String,
    pub product_name: String,
    pub endpoint: String,
    pub pubkey: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateOffer {
    pub version: String,
    pub notes: String,
    pub url: String,
    pub signature: String,
    pub current: String,
    pub product_name: String,
}

#[derive(Debug, Deserialize)]
struct IdentityFile {
    #[serde(default)]
    version: String,
    #[serde(default)]
    product_name: String,
    #[serde(default)]
    update_endpoint: String,
    #[serde(default)]
    update_pubkey: String,
    #[serde(default)]
    app_user_model_id: String,
}

/// Windows toast identity from the packaged `libra-identity.toml`
/// (PLAN-NOTIFY NT3). Absent outside a bundle.
pub fn bundled_app_user_model_id() -> Option<String> {
    let path = bundled_identity_path()?;
    let text = std::fs::read_to_string(path).ok()?;
    app_user_model_id_from_str(&text)
}

pub fn app_user_model_id_from_str(text: &str) -> Option<String> {
    let file: IdentityFile = toml::from_str(text).ok()?;
    let id = file.app_user_model_id.trim();
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

#[derive(Debug, Deserialize)]
struct Feed {
    version: String,
    #[serde(default)]
    notes: String,
    platforms: std::collections::BTreeMap<String, FeedPlatform>,
}

#[derive(Debug, Deserialize)]
struct FeedPlatform {
    url: String,
    signature: String,
}

/// `Foo.app/Contents/MacOS/<bin>` → `Contents/Resources/libra-identity.toml`.
pub fn bundled_identity_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let macos = exe.parent()?;
    if macos.file_name().and_then(|s| s.to_str()) != Some("MacOS") {
        return None;
    }
    let path = macos
        .parent()?
        .join("Resources")
        .join("libra-identity.toml");
    path.is_file().then_some(path)
}

pub fn load_config(path: &Path) -> Result<Option<UpdateConfig>, String> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    config_from_str(&text)
}

pub fn config_from_str(text: &str) -> Result<Option<UpdateConfig>, String> {
    let file: IdentityFile = toml::from_str(text).map_err(|e| format!("identity: {e}"))?;
    if file.update_endpoint.is_empty() || file.update_pubkey.is_empty() {
        return Ok(None);
    }
    Ok(Some(UpdateConfig {
        version: file.version,
        product_name: file.product_name,
        endpoint: file.update_endpoint,
        pubkey: file.update_pubkey,
    }))
}

pub fn host_target() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "darwin-aarch64",
        ("macos", "x86_64") => "darwin-x86_64",
        ("windows", "aarch64") => "windows-aarch64",
        ("windows", _) => "windows-x86_64",
        (_, "aarch64") => "linux-aarch64",
        _ => "linux-x86_64",
    }
}

pub fn offer_from_feed(
    cfg: &UpdateConfig,
    target: &str,
    body: &str,
) -> Result<Option<UpdateOffer>, String> {
    let feed: Feed = serde_json::from_str(body).map_err(|e| format!("latest.json: {e}"))?;
    if !version_is_newer(&feed.version, &cfg.version) {
        return Ok(None);
    }
    let Some(platform) = feed.platforms.get(target) else {
        return Ok(None);
    };
    Ok(Some(UpdateOffer {
        version: feed.version,
        notes: feed.notes,
        url: platform.url.clone(),
        signature: platform.signature.clone(),
        current: cfg.version.clone(),
        product_name: cfg.product_name.clone(),
    }))
}

pub fn version_is_newer(candidate: &str, current: &str) -> bool {
    let a = version_parts(candidate);
    let b = version_parts(current);
    for i in 0..a.len().max(b.len()) {
        let av = a.get(i).copied().unwrap_or(0);
        let bv = b.get(i).copied().unwrap_or(0);
        if av != bv {
            return av > bv;
        }
    }
    false
}

fn version_parts(v: &str) -> Vec<u64> {
    v.trim()
        .trim_start_matches('v')
        .split('.')
        .map(|part| {
            part.chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse()
                .unwrap_or(0)
        })
        .collect()
}

pub fn verify_signature(pubkey: &str, bytes: &[u8], signature: &str) -> Result<(), String> {
    let key = pubkey_line(pubkey);
    let public =
        minisign_verify::PublicKey::from_base64(key).map_err(|e| format!("pubkey: {e}"))?;
    let sig =
        minisign_verify::Signature::decode(signature).map_err(|e| format!("signature: {e}"))?;
    public
        .verify(bytes, &sig, false)
        .map_err(|e| format!("signature mismatch: {e}"))
}

fn pubkey_line(raw: &str) -> &str {
    raw.lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with("untrusted comment:"))
        .unwrap_or(raw.trim())
}

pub fn fetch_text(url: &str) -> Result<String, String> {
    let response = ureq::get(url)
        .call()
        .map_err(|e| format!("GET {url}: {e}"))?;
    response
        .into_string()
        .map_err(|e| format!("read {url}: {e}"))
}

pub fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
    let response = ureq::get(url)
        .call()
        .map_err(|e| format!("GET {url}: {e}"))?;
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(512 * 1024 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("read {url}: {e}"))?;
    Ok(bytes)
}

/// Download the archive, verify it, and return the extracted `.app`.
pub fn stage(pubkey: &str, offer: &UpdateOffer) -> Result<PathBuf, String> {
    let bytes = fetch_bytes(&offer.url)?;
    verify_signature(pubkey, &bytes, &offer.signature)?;
    let staging = std::env::temp_dir().join(format!("libra-update-{}", std::process::id()));
    if staging.exists() {
        std::fs::remove_dir_all(&staging)
            .map_err(|e| format!("clear {}: {e}", staging.display()))?;
    }
    std::fs::create_dir_all(&staging).map_err(|e| format!("create {}: {e}", staging.display()))?;
    let archive = staging.join("update.tar.gz");
    std::fs::write(&archive, &bytes).map_err(|e| format!("write {}: {e}", archive.display()))?;
    let status = Command::new("tar")
        .arg("-xzf")
        .arg(&archive)
        .arg("-C")
        .arg(&staging)
        .env("COPYFILE_DISABLE", "1")
        .stdin(Stdio::null())
        .status()
        .map_err(|e| format!("tar: {e}"))?;
    if !status.success() {
        return Err(format!("tar exited with {status}"));
    }
    for entry in std::fs::read_dir(&staging).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().and_then(|e| e.to_str()) == Some("app") {
            return Ok(path);
        }
    }
    Err("the update archive did not contain a .app".into())
}

pub fn app_bundle(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let app = contents.parent()?;
    (app.extension().and_then(|e| e.to_str()) == Some("app")).then(|| app.to_path_buf())
}

pub fn relaunch_script(pid: u32, dest: &Path, staged: &Path) -> String {
    format!(
        "#!/bin/sh\nset -eu\nwhile kill -0 {pid} 2>/dev/null; do\n  sleep 0.2\ndone\nrm -rf {dest}\nmv {staged} {dest}\nopen {dest}\n",
        dest = sh_quote(dest),
        staged = sh_quote(staged),
    )
}

fn sh_quote(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "'\\''"))
}

pub fn check_enabled() -> bool {
    std::env::var("LIBRA_NO_UPDATE").ok().as_deref() != Some("1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_versions_compare_numerically() {
        assert!(version_is_newer("0.2.0", "0.1.9"));
        assert!(version_is_newer("0.1.10", "0.1.2"));
        assert!(!version_is_newer("0.1.1", "0.1.1"));
        assert!(!version_is_newer("0.1.0", "0.1.1"));
        assert!(version_is_newer("v0.3.0", "0.2.9"));
    }

    #[test]
    fn feed_offers_only_a_newer_build_for_this_target() {
        let cfg = UpdateConfig {
            version: "0.1.0".into(),
            product_name: "Lima".into(),
            endpoint: "https://example.com/latest.json".into(),
            pubkey: "PUB".into(),
        };
        let body = r#"{
            "version": "0.2.0",
            "notes": "faster",
            "platforms": {
                "darwin-aarch64": { "url": "https://example.com/Lima.app.tar.gz", "signature": "SIG" },
                "linux-x86_64": { "url": "https://example.com/lima.deb", "signature": "SIG2" }
            }
        }"#;
        let offer = offer_from_feed(&cfg, "darwin-aarch64", body)
            .unwrap()
            .unwrap();
        assert_eq!(offer.version, "0.2.0");
        assert_eq!(offer.current, "0.1.0");
        assert!(offer.url.ends_with("Lima.app.tar.gz"));
        assert!(offer_from_feed(&cfg, "windows-x86_64", body)
            .unwrap()
            .is_none());

        let same = UpdateConfig {
            version: "0.2.0".into(),
            ..cfg
        };
        assert!(offer_from_feed(&same, "darwin-aarch64", body)
            .unwrap()
            .is_none());
    }

    #[test]
    fn identity_without_an_endpoint_does_not_check() {
        let text = r#"
app_user_model_id = "Libra.Lima"
bundle_id = "dev.libra.lima"
product_name = "Lima"
version = "0.1.0"
"#;
        assert!(config_from_str(text).unwrap().is_none());
        let with = r#"
product_name = "Lima"
version = "0.1.0"
update_endpoint = "https://example.com/latest.json"
update_pubkey = "PUB"
"#;
        let cfg = config_from_str(with).unwrap().unwrap();
        assert_eq!(cfg.endpoint, "https://example.com/latest.json");
    }

    #[test]
    fn relaunch_script_waits_for_the_pid_then_swaps_the_bundle() {
        let script = relaunch_script(
            42,
            Path::new("/Applications/Lima.app"),
            Path::new("/tmp/Lima.app"),
        );
        assert!(script.contains("kill -0 42"));
        assert!(script.contains("'/Applications/Lima.app'"));
        assert!(script.contains("'/tmp/Lima.app'"));
        assert!(script.contains("open "));
    }

    #[test]
    fn minisign_roundtrip_rejects_a_tampered_archive() {
        let pair = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
        let public = pair.pk.to_base64();
        let signature = minisign::sign(
            None,
            &pair.sk,
            std::io::Cursor::new(b"archive-bytes"),
            None,
            Some("test".into()),
        )
        .unwrap()
        .to_string();
        verify_signature(&public, b"archive-bytes", &signature).unwrap();
        assert!(verify_signature(&public, b"tampered", &signature).is_err());
    }
}
