//! `libra test` (ARCHITECTURE.md §18.1, §15.8; PLAN-TEST-CLI) — the
//! centralized SDK test runner.
//!
//! A vitest-shaped front door over the whole workspace, backed by
//! `cargo nextest` + `cargo llvm-cov` when present and degrading to plain
//! `cargo test` otherwise. This module wires the pieces together; the
//! testable pure logic lives in the submodules:
//!
//! - [`args`] — flag parsing → [`TestPlan`]
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

    let root = match find_workspace_root(&std::env::current_dir().unwrap_or_default()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("libra test: {e}");
            return ExitCode::from(2);
        }
    };

    let detected = Detected::probe();
    print_banner(&plan, &detected);

    if plan.watch {
        watch::watch_loop(&root, || {
            let _ = run_once(&plan, &root, &detected);
        });
        return ExitCode::SUCCESS; // unreachable: watch_loop blocks
    }

    let code = run_once(&plan, &root, &detected);

    if let Some(out) = &plan.trace_matrix {
        let dir = out
            .clone()
            .unwrap_or_else(|| root.join("target/libra-test/trace-matrix"));
        emit_trace_matrix(&root, &dir);
    }

    code
}

fn run_once(plan: &TestPlan, root: &Path, detected: &Detected) -> ExitCode {
    let runner = detected.runner();

    // Coverage needs both tools; it only ever wraps the nextest runner
    // (the one documented `cargo llvm-cov nextest` form).
    let coverage_on =
        plan.coverage != Coverage::Off && detected.nextest && detected.llvm_cov;
    if plan.coverage != Coverage::Off && !coverage_on {
        eprintln!(
            "libra test: coverage needs cargo-nextest + cargo-llvm-cov — running without it"
        );
    }

    let cov_path = coverage_on.then(|| root.join("target/libra-test/coverage.json"));
    if let Some(p) = &cov_path {
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
    }

    let args = cmd::build(
        plan,
        runner,
        cov_path.as_deref().and_then(|p| p.to_str()),
    );

    let mut command = Command::new("cargo");
    command
        .args(&args)
        .current_dir(root)
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
            report_coverage(root, p, plan.coverage);
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
        eprintln!("libra test: coverage ran but {} is unreadable", json_path.display());
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

fn print_banner(plan: &TestPlan, detected: &Detected) {
    let runner = match detected.runner() {
        Runner::Nextest => "cargo nextest",
        Runner::CargoTest => "cargo test (fallback)",
    };
    eprintln!("libra test: runner = {runner}");
    if let Some(hint) = detected.install_hint() {
        eprintln!("  tip: for filtering + coverage, `{hint}`");
    }
    let scope = if plan.packages.is_empty() {
        "workspace".to_string()
    } else {
        plan.packages.join(", ")
    };
    eprintln!("  scope: {scope}");
    if let Some(c) = &plan.component {
        eprintln!("  component: {c}");
    }
}

/// Walk upward from `start` to the workspace root (the `Cargo.toml` that
/// declares `[workspace]`).
fn find_workspace_root(start: &Path) -> Result<PathBuf, String> {
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
