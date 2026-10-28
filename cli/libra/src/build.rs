//! `libra build` (ARCHITECTURE.md §18.1, PLAN-PACKAGE P1, PLAN-SDK S1) —
//! release binary with view codegen + workspace LTO. Pins
//! `--manifest-path` so out-of-tree apps do not depend on process cwd.

use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};

use crate::app::resolve;

/// `libra build [path] [-p crate] [-- cargo-args…]`
pub fn build(args: &[String]) -> ExitCode {
    let (root, package_override, cargo_args) = parse_build_args(args);

    let app = match resolve(root, package_override) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            eprintln!("hint: run from an app directory, pass a path, or use -p — e.g. `libra build -p aws-login`");
            return ExitCode::from(2);
        }
    };

    eprintln!(
        "libra build: release build of {} (codegen views + LTO — §18.2)",
        app.crate_name
    );
    eprintln!("  manifest: {}", app.workspace_manifest.display());

    // Pin the workspace the way `libra package` does. Without
    // `--manifest-path`, `cargo -p` uses process cwd and fails for an
    // out-of-tree app invoked as `libra build /path/to/app` (PLAN-SDK S1).
    let mut cmd = Command::new("cargo");
    cmd.args(release_build_args(&app))
        .args(&cargo_args)
        .current_dir(&app.workspace_root)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    match cmd.status() {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(status) => ExitCode::from(status.code().unwrap_or(1) as u8),
        Err(e) => {
            eprintln!("libra build: failed to run cargo: {e}");
            ExitCode::from(1)
        }
    }
}

pub fn parse_build_args(args: &[String]) -> (PathBuf, Option<String>, Vec<String>) {
    let mut root = PathBuf::from(".");
    let mut package = None;
    let mut cargo_args = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-p" | "--package" => {
                if let Some(v) = iter.next() {
                    package = Some(v.clone());
                }
            }
            s if s.starts_with("--package=") => {
                package = Some(s.trim_start_matches("--package=").to_string());
            }
            "--" => {
                cargo_args.extend(iter.cloned());
                break;
            }
            s if !s.starts_with('-') && root.as_os_str().is_empty() => {
                root = PathBuf::from(s);
            }
            s if !s.starts_with('-') && root == PathBuf::from(".") => {
                root = PathBuf::from(s);
            }
            other => cargo_args.push(other.to_string()),
        }
    }
    (root, package, cargo_args)
}

/// `cargo build --release --manifest-path <workspace> -p <crate> --bin <crate>`
pub fn release_build_args(app: &crate::app::ResolvedApp) -> Vec<String> {
    vec![
        "build".into(),
        "--release".into(),
        "--manifest-path".into(),
        app.workspace_manifest.display().to_string(),
        "-p".into(),
        app.crate_name.clone(),
        "--bin".into(),
        app.crate_name.clone(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn package_flag_and_path() {
        let args = vec![
            "applications/aws-login".into(),
            "-p".into(),
            "counter".into(),
        ];
        let (path, package, rest) = parse_build_args(&args);
        assert_eq!(path, PathBuf::from("applications/aws-login"));
        assert_eq!(package.as_deref(), Some("counter"));
        assert!(rest.is_empty());
    }

    #[test]
    fn default_root() {
        let (path, package, _) = parse_build_args(&["-p".into(), "notes".into()]);
        assert_eq!(path, PathBuf::from("."));
        assert_eq!(package.as_deref(), Some("notes"));
    }

    #[test]
    fn release_build_pins_workspace_manifest() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let template = root.join("templates/app");
        let app = crate::app::resolve(template, None).expect("templates/app");
        let args = release_build_args(&app);
        assert_eq!(args[0], "build");
        assert_eq!(args[1], "--release");
        assert_eq!(args[2], "--manifest-path");
        assert!(
            args[3].ends_with("templates/app/Cargo.toml"),
            "expected template workspace manifest, got {}",
            args[3]
        );
        assert_eq!(args[4], "-p");
        assert_eq!(args[5], "starter");
        assert_eq!(args[6], "--bin");
        assert_eq!(args[7], "starter");
    }
}
