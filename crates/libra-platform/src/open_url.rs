//! `Effect::OpenUrl` (PLAN-NATIVE-INTEGRATIONS N1, §12.1).
//!
//! The shell spawns the host opener with the url as one argument.
//! [`plan`] does not spawn; tests stop there. [`launch`] is the OS call.

use std::io::ErrorKind;
use std::process::Command;

use libra_core::OpenResult;

pub struct OpenPlan {
    pub program: &'static str,
    pub args: Vec<String>,
}

/// The process the shell will spawn. Validation failures return before
/// any program name is chosen.
pub fn plan(url: &str) -> Result<OpenPlan, OpenResult> {
    let url = libra_core::open_url_target(url)?;
    Ok(OpenPlan {
        program: opener(),
        args: opener_args(&url),
    })
}

pub fn launch(url: &str) -> OpenResult {
    let plan = match plan(url) {
        Ok(plan) => plan,
        Err(result) => return result,
    };
    match Command::new(plan.program).args(&plan.args).spawn() {
        Ok(mut child) => match child.wait() {
            Ok(status) if status.success() => OpenResult::Opened,
            Ok(status) => OpenResult::Failed(status.to_string()),
            Err(e) => OpenResult::Failed(e.to_string()),
        },
        Err(e) if e.kind() == ErrorKind::NotFound => {
            OpenResult::Unsupported(format!("{} is not available", plan.program))
        }
        Err(e) => OpenResult::Failed(e.to_string()),
    }
}

fn opener() -> &'static str {
    if cfg!(target_os = "windows") {
        "rundll32.exe"
    } else if cfg!(target_os = "macos") {
        "/usr/bin/open"
    } else {
        "xdg-open"
    }
}

fn opener_args(url: &str) -> Vec<String> {
    if cfg!(target_os = "windows") {
        vec!["url.dll,FileProtocolHandler".into(), url.into()]
    } else {
        vec![url.into()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bad_scheme_never_builds_a_command() {
        assert!(matches!(plan("file:///etc/passwd"), Err(OpenResult::Failed(_))));
        assert!(matches!(
            launch("javascript:alert(1)"),
            OpenResult::Failed(_)
        ));
    }

    #[test]
    fn the_url_is_a_single_argument_to_the_host_opener() {
        let plan = plan("https://example.com/a?b=1&c=2").unwrap();
        #[cfg(target_os = "macos")]
        assert_eq!(plan.program, "/usr/bin/open");
        #[cfg(target_os = "windows")]
        assert_eq!(plan.program, "rundll32.exe");
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        assert_eq!(plan.program, "xdg-open");
        assert_eq!(plan.args.last().unwrap(), "https://example.com/a?b=1&c=2");
        assert_eq!(plan.args.iter().filter(|a| a.contains('&')).count(), 1);
    }
}
