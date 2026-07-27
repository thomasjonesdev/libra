//! `libra test` argument parsing (ARCHITECTURE.md §18.1, PLAN-TEST-CLI).
//!
//! Hand-rolled to match the rest of the CLI (see `dev::parse_dev_args`) —
//! no clap dependency. Produces a [`TestPlan`] the orchestrator in the
//! parent module turns into a `cargo nextest` / `cargo test` invocation.

use std::path::PathBuf;

/// The five §15 test kinds selectable with `--kind`. `Replay` is reserved
/// (§15.6 isn't built yet) and intentionally absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Unit,
    Component,
    Integration,
    Snapshot,
}

impl Kind {
    fn parse(s: &str) -> Result<Kind, String> {
        match s {
            "unit" => Ok(Kind::Unit),
            "component" | "components" => Ok(Kind::Component),
            "integration" => Ok(Kind::Integration),
            "snapshot" | "snapshots" => Ok(Kind::Snapshot),
            other => Err(format!(
                "unknown --kind `{other}` (expected unit|component|integration|snapshot)"
            )),
        }
    }

    /// The cargo `tests/<name>.rs` integration-test binary this kind maps
    /// to, or `None` for unit tests (which live in lib/bin targets).
    pub fn test_binary(self) -> Option<&'static str> {
        match self {
            Kind::Unit => None,
            Kind::Component => Some("components"),
            Kind::Integration => Some("integration"),
            Kind::Snapshot => Some("snapshots"),
        }
    }
}

/// How much coverage to measure. `Actions` (the default) reports the
/// actions-layer number per app; `Full` reports every crate; `Off` skips
/// instrumentation entirely (§15.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coverage {
    Actions,
    Full,
    Off,
}

/// The fully parsed `libra test` invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestPlan {
    /// Positional substring filters on test names (vitest: `libra test Button`).
    pub filters: Vec<String>,
    /// `-p/--package` scopes (repeatable); empty means the whole workspace.
    pub packages: Vec<String>,
    pub kind: Option<Kind>,
    /// `--component <Name>` — folded into `filters` as a lowercased
    /// substring, recorded here too for messaging.
    pub component: Option<String>,
    pub coverage: Coverage,
    pub watch: bool,
    pub validate: bool,
    /// `Some(out)` when `--trace-matrix` is present; `out` is the output
    /// directory (defaulted by the caller when `None`).
    pub trace_matrix: Option<Option<PathBuf>>,
    /// Everything after a literal `--` — forwarded to the inner runner.
    pub passthrough: Vec<String>,
}

impl Default for TestPlan {
    fn default() -> Self {
        TestPlan {
            filters: Vec::new(),
            packages: Vec::new(),
            kind: None,
            component: None,
            coverage: Coverage::Actions,
            watch: false,
            validate: false,
            trace_matrix: None,
            passthrough: Vec::new(),
        }
    }
}

impl TestPlan {
    /// Parse `libra test`'s arguments (everything after `test`).
    pub fn parse(args: &[String]) -> Result<TestPlan, String> {
        let mut plan = TestPlan::default();
        let mut it = args.iter().peekable();

        while let Some(arg) = it.next() {
            match arg.as_str() {
                "--" => {
                    plan.passthrough.extend(it.cloned());
                    break;
                }
                "-p" | "--package" => {
                    let v = it.next().ok_or_else(|| format!("{arg} needs a crate name"))?;
                    plan.packages.push(v.clone());
                }
                "--kind" => {
                    let v = it.next().ok_or("--kind needs a value")?;
                    plan.kind = Some(Kind::parse(v)?);
                }
                "--component" => {
                    let v = it.next().ok_or("--component needs a component name")?;
                    plan.component = Some(v.clone());
                    plan.filters.push(v.to_lowercase());
                }
                "--no-coverage" => plan.coverage = Coverage::Off,
                "--watch" => plan.watch = true,
                "--validate" => plan.validate = true,
                other => {
                    if let Some(rest) = other.strip_prefix("--package=") {
                        plan.packages.push(rest.to_string());
                    } else if let Some(rest) = other.strip_prefix("--kind=") {
                        plan.kind = Some(Kind::parse(rest)?);
                    } else if let Some(rest) = other.strip_prefix("--component=") {
                        plan.component = Some(rest.to_string());
                        plan.filters.push(rest.to_lowercase());
                    } else if other == "--coverage" {
                        plan.coverage = Coverage::Actions;
                    } else if let Some(rest) = other.strip_prefix("--coverage=") {
                        plan.coverage = parse_coverage(rest)?;
                    } else if other == "--trace-matrix" {
                        // Optional value: consume the next arg only if it
                        // isn't itself a flag.
                        let take = it.peek().is_some_and(|n| !n.starts_with('-'));
                        let out = if take {
                            it.next().map(PathBuf::from)
                        } else {
                            None
                        };
                        plan.trace_matrix = Some(out);
                    } else if let Some(rest) = other.strip_prefix("--trace-matrix=") {
                        plan.trace_matrix = Some(Some(PathBuf::from(rest)));
                    } else if other.starts_with('-') {
                        return Err(format!("unknown flag `{other}`"));
                    } else {
                        plan.filters.push(other.to_string());
                    }
                }
            }
        }

        if plan.watch && plan.coverage != Coverage::Off {
            // Instrumented rebuilds are too heavy for an iterate loop
            // (§15.8) — watch always runs uninstrumented.
            plan.coverage = Coverage::Off;
        }
        Ok(plan)
    }

    /// Trailing filter args a runner appends to select tests by name —
    /// the positional substrings, shared by both nextest and cargo test.
    pub fn name_filters(&self) -> &[String] {
        &self.filters
    }
}

fn parse_coverage(s: &str) -> Result<Coverage, String> {
    match s {
        "actions" => Ok(Coverage::Actions),
        "full" => Ok(Coverage::Full),
        "off" | "none" => Ok(Coverage::Off),
        other => Err(format!(
            "unknown --coverage `{other}` (expected actions|full|off)"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> TestPlan {
        TestPlan::parse(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap()
    }

    #[test]
    fn defaults_to_actions_coverage_no_filters() {
        let p = parse(&[]);
        assert_eq!(p.coverage, Coverage::Actions);
        assert!(p.filters.is_empty());
        assert!(p.packages.is_empty());
        assert_eq!(p.kind, None);
    }

    #[test]
    fn positional_args_are_name_filters() {
        let p = parse(&["Button", "Badge"]);
        assert_eq!(p.filters, vec!["Button", "Badge"]);
    }

    #[test]
    fn component_folds_into_lowercased_filter() {
        let p = parse(&["--component", "Slider"]);
        assert_eq!(p.component.as_deref(), Some("Slider"));
        assert_eq!(p.filters, vec!["slider"]);
    }

    #[test]
    fn kind_and_package_forms() {
        let p = parse(&["--kind", "integration", "-p", "notes"]);
        assert_eq!(p.kind, Some(Kind::Integration));
        assert_eq!(p.packages, vec!["notes"]);
        // `=` forms too
        let p = parse(&["--kind=snapshot", "--package=counter"]);
        assert_eq!(p.kind, Some(Kind::Snapshot));
        assert_eq!(p.packages, vec!["counter"]);
    }

    #[test]
    fn coverage_variants() {
        assert_eq!(parse(&["--coverage=full"]).coverage, Coverage::Full);
        assert_eq!(parse(&["--coverage"]).coverage, Coverage::Actions);
        assert_eq!(parse(&["--no-coverage"]).coverage, Coverage::Off);
        assert_eq!(parse(&["--coverage=off"]).coverage, Coverage::Off);
    }

    #[test]
    fn watch_forces_coverage_off() {
        let p = parse(&["--watch", "--coverage=full"]);
        assert!(p.watch);
        assert_eq!(p.coverage, Coverage::Off);
    }

    #[test]
    fn trace_matrix_optional_value() {
        assert_eq!(parse(&["--trace-matrix"]).trace_matrix, Some(None));
        assert_eq!(
            parse(&["--trace-matrix", "/tmp/tm"]).trace_matrix,
            Some(Some(PathBuf::from("/tmp/tm")))
        );
        assert_eq!(
            parse(&["--trace-matrix=/tmp/tm"]).trace_matrix,
            Some(Some(PathBuf::from("/tmp/tm")))
        );
        // A following flag is not swallowed as the value.
        let p = parse(&["--trace-matrix", "--validate"]);
        assert_eq!(p.trace_matrix, Some(None));
        assert!(p.validate);
    }

    #[test]
    fn double_dash_forwards_verbatim() {
        let p = parse(&["Button", "--", "--nocapture", "-j1"]);
        assert_eq!(p.filters, vec!["Button"]);
        assert_eq!(p.passthrough, vec!["--nocapture", "-j1"]);
    }

    #[test]
    fn unknown_flag_errs() {
        let e = TestPlan::parse(&["--bogus".to_string()]).unwrap_err();
        assert!(e.contains("bogus"));
    }
}
