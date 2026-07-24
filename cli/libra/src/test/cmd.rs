//! Turn a [`TestPlan`] into the concrete `cargo …` argument vector for the
//! chosen runner (PLAN-TEST-CLI). Kept pure (no process spawning) so the
//! nextest / cargo-test / llvm-cov command shapes are unit-testable — the
//! parts that can't be run in every CI environment.

use super::args::{Kind, TestPlan};
use super::tool::Runner;

/// The nextest filterset expression (`-E` value) selecting a `--kind`.
/// Unit tests live in lib targets; the other kinds map to their
/// `tests/<name>.rs` integration binary.
fn nextest_kind_expr(kind: Kind) -> String {
    match kind.test_binary() {
        None => "kind(lib)".to_string(),
        Some(bin) => format!("binary({bin})"),
    }
}

/// Cargo target-selection flags for a `--kind` on the `cargo test` path.
fn cargo_kind_flags(kind: Kind) -> Vec<String> {
    match kind {
        Kind::Unit => vec!["--lib".into(), "--bins".into()],
        Kind::Component => vec!["--test".into(), "components".into()],
        Kind::Integration => vec!["--test".into(), "integration".into()],
        Kind::Snapshot => vec!["--test".into(), "snapshots".into()],
    }
}

/// Build the full `cargo` argument vector (everything after `cargo`).
///
/// `coverage_json` is `Some(path)` when the run should be wrapped in
/// `cargo llvm-cov` and export its JSON to `path`.
pub fn build(plan: &TestPlan, runner: Runner, coverage_json: Option<&str>) -> Vec<String> {
    let mut args = Vec::new();

    // Coverage prefix: `cargo llvm-cov --json --output-path <p> …`.
    if let Some(path) = coverage_json {
        args.push("llvm-cov".into());
        args.push("--json".into());
        args.push("--output-path".into());
        args.push(path.into());
    }

    match runner {
        Runner::Nextest => {
            args.push("nextest".into());
            args.push("run".into());
            for p in &plan.packages {
                args.push("-p".into());
                args.push(p.clone());
            }
            if let Some(kind) = plan.kind {
                args.push("-E".into());
                args.push(nextest_kind_expr(kind));
            }
            args.extend(plan.passthrough.iter().cloned());
            // Positional substring filters select tests by name.
            args.extend(plan.name_filters().iter().cloned());
        }
        Runner::CargoTest => {
            args.push("test".into());
            if plan.packages.is_empty() {
                args.push("--workspace".into());
            } else {
                for p in &plan.packages {
                    args.push("-p".into());
                    args.push(p.clone());
                }
            }
            if let Some(kind) = plan.kind {
                args.extend(cargo_kind_flags(kind));
            }
            // libtest args go after `--`.
            args.push("--".into());
            args.extend(plan.name_filters().iter().cloned());
            args.extend(plan.passthrough.iter().cloned());
        }
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(args: &[&str]) -> TestPlan {
        TestPlan::parse(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap()
    }

    #[test]
    fn nextest_positional_filters_and_package() {
        let a = build(&plan(&["Button", "-p", "gallery"]), Runner::Nextest, None);
        assert_eq!(a, vec!["nextest", "run", "-p", "gallery", "Button"]);
    }

    #[test]
    fn nextest_kind_maps_to_filterset() {
        let a = build(&plan(&["--kind", "component"]), Runner::Nextest, None);
        assert_eq!(a, vec!["nextest", "run", "-E", "binary(components)"]);
        let a = build(&plan(&["--kind", "unit"]), Runner::Nextest, None);
        assert_eq!(a, vec!["nextest", "run", "-E", "kind(lib)"]);
    }

    #[test]
    fn cargo_test_fallback_workspace_and_kind() {
        let a = build(&plan(&["--kind", "integration", "Foo"]), Runner::CargoTest, None);
        assert_eq!(
            a,
            vec!["test", "--workspace", "--test", "integration", "--", "Foo"]
        );
    }

    #[test]
    fn cargo_test_scoped_package() {
        let a = build(&plan(&["-p", "notes"]), Runner::CargoTest, None);
        assert_eq!(a, vec!["test", "-p", "notes", "--"]);
    }

    #[test]
    fn coverage_wraps_with_llvm_cov() {
        let a = build(&plan(&["Button"]), Runner::Nextest, Some("/tmp/c.json"));
        assert_eq!(
            a,
            vec![
                "llvm-cov", "--json", "--output-path", "/tmp/c.json", "nextest", "run", "Button"
            ]
        );
    }

    #[test]
    fn component_flag_becomes_lowercased_filter() {
        let a = build(&plan(&["--component", "Slider"]), Runner::Nextest, None);
        assert_eq!(a, vec!["nextest", "run", "slider"]);
    }
}
