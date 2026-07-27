//! `libra dev` (ARCHITECTURE.md §18.1 / §18.2) — Phase 1 subset.
//!
//! Runs a Libra app crate in debug mode via `cargo run`. View hot reload
//! (mtime watch + interpreter engine) is already wired by `libra::app!`
//! and `libra-platform::Options::watch` in debug builds. When `libra.toml`
//! declares `[package].icon`, the path is forwarded to the app via
//! `libra_platform::ICON_PATH_ENV` so the window shows the same icon as
//! packaged builds. Inspector, automation server, and fuller manifest
//! integration are still open (§16, §18.4, §18.5).

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use crate::manifest::try_icon_path;

/// Split `libra dev [path] [-- cargo-args…]`.
pub fn parse_dev_args(args: &[String]) -> (PathBuf, Vec<String>) {
    let mut path = PathBuf::from(".");
    let mut cargo_args = Vec::new();
    let mut rest = args.iter();

    if let Some(first) = rest.next() {
        if first == "--" {
            cargo_args.extend(rest.cloned());
            return (path, cargo_args);
        }
        if !first.starts_with('-') {
            path = PathBuf::from(first);
            if let Some(second) = rest.next() {
                if second == "--" {
                    cargo_args.extend(rest.cloned());
                    return (path, cargo_args);
                }
                cargo_args.push(second.clone());
                cargo_args.extend(rest.cloned());
                return (path, cargo_args);
            }
            return (path, cargo_args);
        }
        cargo_args.push(first.clone());
        cargo_args.extend(rest.cloned());
    }

    (path, cargo_args)
}

/// Walk upward from `start` until a package `Cargo.toml` is found.
pub fn find_app_manifest(start: &Path) -> Result<PathBuf, String> {
    let start = if start.is_file() {
        start.parent().ok_or_else(|| format!("no parent directory for {}", start.display()))?
    } else {
        start
    };

    let mut dir = start
        .canonicalize()
        .map_err(|e| format!("{}: {e}", start.display()))?;

    loop {
        let manifest = dir.join("Cargo.toml");
        if manifest.is_file() {
            let source = std::fs::read_to_string(&manifest)
                .map_err(|e| format!("{}: {e}", manifest.display()))?;
            if is_package_manifest(&source) {
                if let Some(reason) = validate_libra_app(&source, &dir) {
                    return Err(format!(
                        "{}: {reason}",
                        manifest.display()
                    ));
                }
                return Ok(manifest);
            }
        }

        if !dir.pop() {
            break;
        }
    }

    Err(format!(
        "libra dev: no Libra app package found near {}",
        start.display()
    ))
}

fn is_package_manifest(source: &str) -> bool {
    source.lines().any(|line| line.trim() == "[package]")
}

/// Returns `Some(reason)` when the manifest is not a runnable Libra app.
fn validate_libra_app(manifest: &str, package_dir: &Path) -> Option<String> {
    if !manifest.contains("libra-platform") && !manifest.contains("libra.workspace") {
        return Some(
            "not a Libra app (missing libra-platform / libra dependency)".into(),
        );
    }

    let has_bin = package_dir.join("src/main.rs").is_file()
        || manifest.lines().any(|line| line.trim() == "[[bin]]");
    if !has_bin {
        return Some("no binary target (expected src/main.rs)".into());
    }

    None
}

pub fn package_name(manifest: &str) -> Option<String> {
    for line in manifest.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("name = ") {
            return rest.trim().trim_matches('"').to_string().into();
        }
    }
    None
}

pub fn dev(args: &[String]) -> ExitCode {
    let (root, cargo_args) = parse_dev_args(args);

    let manifest = match find_app_manifest(&root) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{e}");
            eprintln!("hint: run from an app directory, or pass a path — e.g. `libra dev examples/counter`");
            return ExitCode::from(2);
        }
    };

    let manifest_source = match std::fs::read_to_string(&manifest) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}: {e}", manifest.display());
            return ExitCode::from(2);
        }
    };
    let name = package_name(&manifest_source).unwrap_or_else(|| "app".into());
    let app_dir = manifest.parent().unwrap().to_path_buf();

    eprintln!(
        "libra dev: running {name} (debug, view hot reload via interpreter — §18.2)"
    );
    eprintln!("  manifest: {}", manifest.display());
    if !cargo_args.is_empty() {
        eprintln!("  args: {}", cargo_args.join(" "));
    }

    let mut cmd = Command::new("cargo");
    cmd.arg("run")
        .arg("--manifest-path")
        .arg(&manifest)
        .args(&cargo_args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    if let Some(icon) = try_icon_path(&app_dir) {
        eprintln!("  icon: {}", icon.display());
        cmd.env("LIBRA_ICON_PATH", icon);
    }

    match cmd.status() {
        Ok(status) => match status.code() {
            Some(0) => ExitCode::SUCCESS,
            Some(code) if (0..=255).contains(&code) => ExitCode::from(code as u8),
            _ => ExitCode::FAILURE,
        },
        Err(e) => {
            eprintln!("libra dev: failed to spawn cargo: {e}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("repo root")
    }

    #[test]
    fn parse_dev_args_defaults_to_cwd() {
        let (path, args) = parse_dev_args(&[]);
        assert_eq!(path, PathBuf::from("."));
        assert!(args.is_empty());
    }

    #[test]
    fn parse_dev_args_path_and_forwarded() {
        let args = vec![
            "examples/counter".into(),
            "--".into(),
            "themes/solarized.toml".into(),
        ];
        let (path, cargo_args) = parse_dev_args(&args);
        assert_eq!(path, PathBuf::from("examples/counter"));
        assert_eq!(cargo_args, vec!["themes/solarized.toml"]);
    }

    #[test]
    fn parse_dev_args_forward_only() {
        let args = vec!["--".into(), "bench".into()];
        let (path, cargo_args) = parse_dev_args(&args);
        assert_eq!(path, PathBuf::from("."));
        assert_eq!(cargo_args, vec!["bench"]);
    }

    #[test]
    fn find_app_manifest_in_nested_dir() {
        let counter_src = repo_root().join("examples/counter/src");
        let manifest = find_app_manifest(&counter_src).expect("counter app");
        assert!(manifest.ends_with("examples/counter/Cargo.toml"));
    }

    #[test]
    fn find_app_manifest_rejects_workspace_root() {
        assert!(find_app_manifest(&repo_root()).is_err());
    }

    #[test]
    fn validate_requires_libra_deps_and_main() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            r#"
[package]
name = "fake"
version = "0.0.1"
edition = "2021"

[dependencies]
serde = "1"
"#,
        )
        .unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();

        let source = fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
        assert!(validate_libra_app(&source, dir.path()).is_some());
    }
}
