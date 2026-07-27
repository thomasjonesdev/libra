//! `libra build` (ARCHITECTURE.md §18.1, PLAN-PACKAGE P1) — release binary
//! with view codegen + workspace LTO.

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
    eprintln!("  manifest: {}", app.cargo_manifest.display());

    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .arg("--release")
        .arg("-p")
        .arg(&app.crate_name)
        .arg("--bin")
        .arg(&app.crate_name)
        .args(&cargo_args)
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
}
