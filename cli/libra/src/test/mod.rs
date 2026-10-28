//! `libra test` (ARCHITECTURE.md §18.1, §15.8; PLAN-TEST-CLI) — the
//! centralized SDK test runner.
//!
//! A vitest-shaped front door over the whole workspace, backed by
//! `cargo nextest` + `cargo llvm-cov` when present and degrading to plain
//! `cargo test` otherwise. This module wires the pieces together; the
//! testable pure logic lives in the submodules:
//!
//! - [`args`] — flag parsing → [`TestPlan`] (a crate path is a scope)
//! - [`cmd`]  — [`TestPlan`] → `cargo …` argument vector
//! - [`tool`] — nextest / llvm-cov detection + fallback
//! - [`coverage`] — parse `llvm-cov --json`, split out the actions layer
//! - [`trace`] — `--trace-matrix` requirement → test → artifact bundle
//! - [`watch`] — `--watch` mtime-poll loop

mod args;
mod cmd;
mod coverage;
mod tool;
mod trace;
mod watch;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use args::{Coverage, TestPlan};
use tool::{Detected, Runner};

pub fn test(raw: &[String]) -> ExitCode {
    let plan = match TestPlan::parse(raw) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("libra test: {e}");
            return ExitCode::from(2);
        }
    };

    let cwd = std::env::current_dir().unwrap_or_default();
    let target = match resolve_target(&plan, &cwd) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("libra test: {e}");
            return ExitCode::from(2);
        }
    };

    let detected = Detected::probe();
    print_banner(&plan, &target, &detected);

    if plan.watch {
        watch::watch_loop(&target.workspace_root, || {
            let _ = run_once(&plan, &target, &detected);
        });
        return ExitCode::SUCCESS; // unreachable: watch_loop blocks
    }

    let code = run_once(&plan, &target, &detected);

    if let Some(out) = &plan.trace_matrix {
        let dir = out
            .clone()
            .unwrap_or_else(|| target.workspace_root.join("target/libra-test/trace-matrix"));
        emit_trace_matrix(&target.workspace_root, &dir);
    }

    code
}

fn run_once(plan: &TestPlan, target: &TestTarget, detected: &Detected) -> ExitCode {
    let runner = detected.runner();

    // Coverage needs both tools; it only ever wraps the nextest runner
    // (the one documented `cargo llvm-cov nextest` form).
    let coverage_on = plan.coverage != Coverage::Off && detected.nextest && detected.llvm_cov;
    if plan.coverage != Coverage::Off && !coverage_on {
        eprintln!("libra test: coverage needs cargo-nextest + cargo-llvm-cov — running without it");
    }

    let cov_path = coverage_on.then(|| {
        target
            .workspace_root
            .join("target/libra-test/coverage.json")
    });
    if let Some(p) = &cov_path {
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
    }

    let mut run_plan = plan.clone();
    run_plan.packages = target.packages.clone();
    let args = cmd::build(
        &run_plan,
        runner,
        cov_path.as_deref().and_then(|p| p.to_str()),
        Some(&target.workspace_manifest),
    );

    let mut command = Command::new("cargo");
    command
        .args(&args)
        .current_dir(&target.workspace_root)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    if plan.validate {
        command.env("LIBRA_VALIDATE", "1");
    }

    let status = match command.status() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("libra test: failed to spawn cargo: {e}");
            return ExitCode::from(2);
        }
    };

    if coverage_on && status.success() {
        if let Some(p) = &cov_path {
            report_coverage(&target.workspace_root, p, plan.coverage);
        }
    }

    match status.code() {
        Some(0) => ExitCode::SUCCESS,
        Some(code) if (0..=255).contains(&code) => ExitCode::from(code as u8),
        _ => ExitCode::FAILURE,
    }
}

fn report_coverage(root: &Path, json_path: &Path, mode: Coverage) {
    let Ok(raw) = std::fs::read_to_string(json_path) else {
        eprintln!(
            "libra test: coverage ran but {} is unreadable",
            json_path.display()
        );
        return;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        eprintln!("libra test: could not parse coverage JSON");
        return;
    };

    let suffixes = coverage::actions_file_suffixes(root);
    let report = coverage::parse(&value, |f| suffixes.iter().any(|s| f.ends_with(s.as_str())));

    println!("\n── coverage ─────────────────────────────");
    println!(
        "  actions layer   {:>5.1}%   ({}/{} lines)",
        report.actions.percent(),
        report.actions.covered,
        report.actions.total,
    );
    println!(
        "  overall         {:>5.1}%   ({}/{} lines)",
        report.overall.percent(),
        report.overall.covered,
        report.overall.total,
    );
    if mode == Coverage::Actions {
        for (file, lines) in &report.per_actions_file {
            println!("    {:<38} {:>5.1}%", file, lines.percent());
        }
    }
    println!("─────────────────────────────────────────");
}

fn emit_trace_matrix(root: &Path, out_dir: &Path) {
    let entries = trace::collect(root);
    match trace::write(&entries, out_dir) {
        Ok((json, html)) => {
            println!(
                "\nlibra test: trace matrix — {} annotated test(s)\n  {}\n  {}",
                entries.len(),
                json.display(),
                html.display()
            );
        }
        Err(e) => eprintln!("libra test: failed to write trace matrix: {e}"),
    }
}

fn print_banner(plan: &TestPlan, target: &TestTarget, detected: &Detected) {
    let runner = match detected.runner() {
        Runner::Nextest => "cargo nextest",
        Runner::CargoTest => "cargo test (fallback)",
    };
    eprintln!("libra test: runner = {runner}");
    if let Some(hint) = detected.install_hint() {
        eprintln!("  tip: for filtering + coverage, `{hint}`");
    }
    eprintln!("  manifest: {}", target.workspace_manifest.display());
    let scope = if target.packages.is_empty() {
        "workspace".to_string()
    } else {
        target.packages.join(", ")
    };
    eprintln!("  scope: {scope}");
    if let Some(c) = &plan.component {
        eprintln!("  component: {c}");
    }
}

/// Workspace + package list `libra test` will pass to cargo.
#[derive(Debug, Clone)]
struct TestTarget {
    workspace_root: PathBuf,
    workspace_manifest: PathBuf,
    /// Empty = whole workspace (`--workspace` / no `-p`).
    packages: Vec<String>,
}

/// No path → workspace containing `cwd` (everything). A path to a
/// member crate → that crate (`-p`) on its workspace, with the
/// manifest pinned. A path to a one-crate workspace (`templates/app`)
/// → that crate on that workspace.
fn resolve_target(plan: &TestPlan, cwd: &Path) -> Result<TestTarget, String> {
    match &plan.scope {
        Some(scope) => resolve_scope(scope, &plan.packages),
        None => {
            let root = crate::app::find_workspace_root(cwd)?;
            Ok(TestTarget {
                workspace_manifest: root.join("Cargo.toml"),
                workspace_root: root,
                packages: plan.packages.clone(),
            })
        }
    }
}

fn resolve_scope(scope: &Path, extra_packages: &[String]) -> Result<TestTarget, String> {
    let dir = if scope.is_file() {
        scope
            .parent()
            .ok_or_else(|| format!("no parent directory for {}", scope.display()))?
    } else {
        scope
    };
    let dir = dir
        .canonicalize()
        .map_err(|e| format!("{}: {e}", dir.display()))?;
    let manifest = dir.join("Cargo.toml");
    if !manifest.is_file() {
        return Err(format!("no Cargo.toml at {}", dir.display()));
    }
    let src =
        std::fs::read_to_string(&manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
    let workspace_root = crate::app::find_workspace_root(&dir)?;
    let has_package = src.lines().any(|l| l.trim() == "[package]");

    let mut packages = extra_packages.to_vec();
    // A path that names a package is a scope to that crate, even when
    // the package is its own implicit workspace (no `[workspace]` line).
    if packages.is_empty() && has_package {
        if let Some(name) = crate::dev::package_name(&src) {
            packages.push(name);
        }
    }

    Ok(TestTarget {
        workspace_manifest: workspace_root.join("Cargo.toml"),
        workspace_root,
        packages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("repo root")
    }

    fn plan_with_scope(path: PathBuf) -> TestPlan {
        let mut plan = TestPlan::default();
        plan.scope = Some(path);
        plan
    }

    #[test]
    fn no_path_is_the_whole_workspace() {
        let t = resolve_target(&TestPlan::default(), &repo_root()).unwrap();
        assert!(t.packages.is_empty());
        assert_eq!(t.workspace_root, repo_root());
    }

    #[test]
    fn path_to_a_member_crate_scopes_that_package() {
        let t = resolve_target(
            &plan_with_scope(repo_root().join("examples/counter")),
            &repo_root(),
        )
        .unwrap();
        assert_eq!(t.packages, vec!["counter"]);
        assert_eq!(t.workspace_root, repo_root());
    }

    #[test]
    fn path_to_the_standalone_template_pins_its_workspace() {
        let app = repo_root().join("templates/app");
        let t = resolve_target(&plan_with_scope(app.clone()), &repo_root()).unwrap();
        assert_eq!(t.packages, vec!["starter"]);
        assert_eq!(t.workspace_root, app);
    }
}
