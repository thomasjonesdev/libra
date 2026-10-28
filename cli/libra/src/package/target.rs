//! Host target detection and per-platform bundle defaults (PLAN-PACKAGE P6).

pub fn host_target() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    }
}

pub fn default_formats(target: &str) -> Vec<&'static str> {
    match target {
        "macos" => vec!["app", "dmg"],
        "windows" => vec!["nsis"],
        "linux" => vec!["deb", "appimage"],
        _ => vec!["app", "dmg"],
    }
}

pub fn formats_for_target(target: &str, format: &str) -> Vec<String> {
    if format != "default" {
        return vec![format.to_string()];
    }
    default_formats(target)
        .into_iter()
        .map(str::to_string)
        .collect()
}

/// Release binary filename inside `target/release/`.
pub fn release_binary_name(crate_name: &str, target: &str) -> String {
    if target == "windows" {
        format!("{crate_name}.exe")
    } else {
        crate_name.to_string()
    }
}

pub fn validate_package_target(target: &str) -> Result<(), String> {
    match target {
        "macos" | "windows" | "linux" => {}
        other => {
            return Err(format!(
                "libra package: unknown --target '{other}' (expected macos, windows, or linux)"
            ));
        }
    }
    let host = host_target();
    if target != host {
        return Err(format!(
            "libra package: --target {target} on a {host} host is not supported yet — run on {target} or omit --target"
        ));
    }
    Ok(())
}

pub fn parse_package_flags(args: Vec<String>) -> (String, String, Vec<String>) {
    let mut target = host_target().to_string();
    let mut format = "default".to_string();
    let mut out = Vec::new();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--target" => {
                if let Some(v) = iter.next() {
                    target = v;
                }
            }
            s if s.starts_with("--target=") => {
                target = s.trim_start_matches("--target=").to_string();
            }
            "--format" => {
                if let Some(v) = iter.next() {
                    format = v;
                }
            }
            s if s.starts_with("--format=") => {
                format = s.trim_start_matches("--format=").to_string();
            }
            other => out.push(other.to_string()),
        }
    }
    (target, format, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_macos_formats() {
        assert_eq!(formats_for_target("macos", "default"), vec!["app", "dmg"]);
    }

    #[test]
    fn default_windows_formats() {
        assert_eq!(formats_for_target("windows", "default"), vec!["nsis"]);
    }

    #[test]
    fn default_linux_formats() {
        assert_eq!(
            formats_for_target("linux", "default"),
            vec!["deb", "appimage"]
        );
    }

    #[test]
    fn explicit_format() {
        assert_eq!(formats_for_target("macos", "app"), vec!["app"]);
    }

    #[test]
    fn windows_binary_name() {
        assert_eq!(release_binary_name("aws-login", "windows"), "aws-login.exe");
    }

    #[test]
    fn unix_binary_name() {
        assert_eq!(release_binary_name("aws-login", "macos"), "aws-login");
    }
}
