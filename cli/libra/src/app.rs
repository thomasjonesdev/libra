//! Shared app-crate resolution for `libra dev` / `libra build` / `libra package`.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::dev::{find_app_manifest, package_name};

#[derive(Debug, Clone)]
pub struct ResolvedApp {
    pub crate_name: String,
    pub app_dir: PathBuf,
    pub cargo_manifest: PathBuf,
    pub workspace_root: PathBuf,
    pub workspace_manifest: PathBuf,
}

/// Resolve a workspace app crate from an optional path root and/or `-p` override.
pub fn resolve(root: PathBuf, package_override: Option<String>) -> Result<ResolvedApp, String> {
    let app = if let Some(name) = package_override {
        if root == PathBuf::from(".") {
            let manifest = find_crate_manifest(&name)?;
            let app_dir = manifest.parent().unwrap().to_path_buf();
            ResolvedAppParts {
                crate_name: name,
                app_dir,
                cargo_manifest: manifest,
            }
        } else {
            let manifest = find_app_manifest(&root)?;
            let app_dir = manifest.parent().unwrap().to_path_buf();
            ResolvedAppParts {
                crate_name: name,
                app_dir,
                cargo_manifest: manifest,
            }
        }
    } else {
        let manifest = find_app_manifest(&root)?;
        let source = std::fs::read_to_string(&manifest)
            .map_err(|e| format!("{}: {e}", manifest.display()))?;
        let app_dir = manifest.parent().unwrap().to_path_buf();
        let crate_name = package_name(&source).unwrap_or_else(|| "app".into());
        ResolvedAppParts {
            crate_name,
            app_dir,
            cargo_manifest: manifest,
        }
    };

    let workspace_root = find_workspace_root(&app.app_dir)?;
    Ok(ResolvedApp {
        crate_name: app.crate_name,
        app_dir: app.app_dir,
        cargo_manifest: app.cargo_manifest,
        workspace_manifest: workspace_root.join("Cargo.toml"),
        workspace_root,
    })
}

struct ResolvedAppParts {
    crate_name: String,
    app_dir: PathBuf,
    cargo_manifest: PathBuf,
}

fn find_crate_manifest(crate_name: &str) -> Result<PathBuf, String> {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version=1", "--no-deps"])
        .output()
        .map_err(|e| format!("cargo metadata failed: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("cargo metadata json: {e}"))?;
    let packages = json["packages"].as_array().ok_or("cargo metadata: no packages")?;
    for pkg in packages {
        if pkg["name"].as_str() == Some(crate_name) {
            let path = pkg["manifest_path"]
                .as_str()
                .ok_or("cargo metadata: missing manifest_path")?;
            return Ok(PathBuf::from(path));
        }
    }
    Err(format!("no workspace package named '{crate_name}'"))
}

/// Walk upward from `start` to the workspace root (the `Cargo.toml` that
/// declares `[workspace]`).
pub fn find_workspace_root(start: &Path) -> Result<PathBuf, String> {
    let mut dir = start
        .canonicalize()
        .map_err(|e| format!("{}: {e}", start.display()))?;
    loop {
        let manifest = dir.join("Cargo.toml");
        if manifest.is_file() {
            if let Ok(src) = std::fs::read_to_string(&manifest) {
                if src.lines().any(|l| l.trim() == "[workspace]") {
                    return Ok(dir);
                }
            }
        }
        if !dir.pop() {
            return Err(format!(
                "no Cargo workspace found at or above {}",
                start.display()
            ));
        }
    }
}

/// Cargo's active `target_directory` for the workspace (respects `CARGO_TARGET_DIR`).
pub fn cargo_target_directory(workspace_root: &Path) -> Result<PathBuf, String> {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version=1", "--no-deps"])
        .current_dir(workspace_root)
        .output()
        .map_err(|e| format!("cargo metadata failed: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("cargo metadata json: {e}"))?;
    json["target_directory"]
        .as_str()
        .map(PathBuf::from)
        .ok_or_else(|| "cargo metadata: missing target_directory".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_aws_login_by_path() {
        let root = PathBuf::from("applications/aws-login");
        if !root.join("Cargo.toml").is_file() {
            return;
        }
        let app = resolve(root, None).unwrap();
        assert_eq!(app.crate_name, "aws-login");
    }

    #[test]
    fn resolves_aws_login_by_crate_name() {
        let app = resolve(PathBuf::from("."), Some("aws-login".into()));
        if app.is_err() {
            return;
        }
        assert_eq!(app.unwrap().crate_name, "aws-login");
    }
}
