//! Tool detection for `libra test` (PLAN-TEST-CLI).
//!
//! `libra test` prefers `cargo nextest` (filtering, parallelism, a
//! vitest-shaped reporter) + `cargo llvm-cov` (coverage), but must stay
//! runnable on a bare checkout — so it detects both and degrades:
//! nextest → plain `cargo test`, and coverage → skipped with a hint.

use std::process::{Command, Stdio};

/// Which inner test runner `libra test` will drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Runner {
    /// `cargo nextest run …`
    Nextest,
    /// Fallback: `cargo test …`
    CargoTest,
}

/// What's installed on this machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Detected {
    pub nextest: bool,
    pub llvm_cov: bool,
}

impl Detected {
    pub fn probe() -> Detected {
        Detected {
            nextest: has_cargo_subcommand("nextest"),
            llvm_cov: has_cargo_subcommand("llvm-cov"),
        }
    }

    pub fn runner(&self) -> Runner {
        if self.nextest {
            Runner::Nextest
        } else {
            Runner::CargoTest
        }
    }

    /// The `cargo install …` line to print when tooling is missing.
    pub fn install_hint(&self) -> Option<String> {
        let mut missing = Vec::new();
        if !self.nextest {
            missing.push("cargo-nextest");
        }
        if !self.llvm_cov {
            missing.push("cargo-llvm-cov");
        }
        if missing.is_empty() {
            None
        } else {
            Some(format!("cargo install {}", missing.join(" ")))
        }
    }
}

/// True if `cargo <name>` resolves — probed via `cargo <name> --version`.
fn has_cargo_subcommand(name: &str) -> bool {
    Command::new("cargo")
        .arg(name)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runner_prefers_nextest() {
        let d = Detected {
            nextest: true,
            llvm_cov: true,
        };
        assert_eq!(d.runner(), Runner::Nextest);
        assert_eq!(d.install_hint(), None);
    }

    #[test]
    fn falls_back_when_nextest_absent() {
        let d = Detected {
            nextest: false,
            llvm_cov: false,
        };
        assert_eq!(d.runner(), Runner::CargoTest);
        let hint = d.install_hint().unwrap();
        assert!(hint.contains("cargo-nextest"));
        assert!(hint.contains("cargo-llvm-cov"));
    }

    #[test]
    fn hint_names_only_the_missing_tool() {
        let d = Detected {
            nextest: true,
            llvm_cov: false,
        };
        assert_eq!(
            d.install_hint().as_deref(),
            Some("cargo install cargo-llvm-cov")
        );
    }
}
