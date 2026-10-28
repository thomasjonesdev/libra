//! `libra dev` (ARCHITECTURE.md §18.1 / §18.2).
//!
//! Runs a Libra app crate in debug mode via `cargo run`. `.lx` edits
//! reload inside the running process (`Options::watch`). Rust edits
//! (`src/**/*.rs`, `build.rs`, `Cargo.toml`, `Cargo.lock`) kill that
//! process and start `cargo run` again — an incremental rebuild of the
//! app crate (§18.2). When `libra.toml` declares `[package].icon`, the
//! path is forwarded via `libra_platform::ICON_PATH_ENV`. Inspector,
//! automation server, and `libra eject` are still open (§16, §18.4).

use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant, SystemTime};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

/// Set by SIGINT so the supervisor stops instead of restarting.
static SHUTDOWN: AtomicBool = AtomicBool::new(false);
/// Process-group id of the running `cargo` (0 = none). The app binary
/// inherits the group, so one signal stops both.
static CHILD_PGID: AtomicU32 = AtomicU32::new(0);

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
        start
            .parent()
            .ok_or_else(|| format!("no parent directory for {}", start.display()))?
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
                    return Err(format!("{}: {reason}", manifest.display()));
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

/// PLAN-SDK S2/S4: a facade-only `libra-ui = { path|git|version }` line
/// is enough (`libra =` remains accepted). Do not match `libra-ui-core` /
/// `libra-test` / `libra-build` — those are not the app crate.
/// `libra-platform` stays for in-tree manifests that have not dropped it.
fn mentions_libra_app(manifest: &str) -> bool {
    manifest.lines().any(|line| {
        let t = line.trim();
        t.starts_with("libra-ui ")
            || t.starts_with("libra-ui=")
            || t.starts_with("libra-platform")
            || t.starts_with("libra.workspace")
            || t.starts_with("libra-ui.workspace")
            || t.starts_with("libra =")
    })
}

/// Returns `Some(reason)` when the manifest is not a runnable Libra app.
fn validate_libra_app(manifest: &str, package_dir: &Path) -> Option<String> {
    if !mentions_libra_app(manifest) {
        return Some("not a Libra app (missing libra / libra-platform dependency)".into());
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

/// A Rust edit invalidates the running process (§18.2). `.lx` and theme
/// TOML are not in this set — the shell reloads those in place.
pub fn is_rust_rebuild_input(path: &Path) -> bool {
    match path.file_name().and_then(|n| n.to_str()) {
        Some("Cargo.toml" | "Cargo.lock" | "build.rs") => true,
        _ => path.extension().is_some_and(|ext| ext == "rs"),
    }
}

/// Newest mtime among Rust rebuild inputs under `app_dir`.
/// `target/` and `.git/` are skipped.
pub fn rust_input_mtime(app_dir: &Path) -> Option<SystemTime> {
    let mut newest: Option<SystemTime> = None;
    let mut stack = vec![app_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            if path.is_dir() {
                if name == "target" || name == ".git" {
                    continue;
                }
                stack.push(path);
                continue;
            }
            if !is_rust_rebuild_input(&path) {
                continue;
            }
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            let Ok(mtime) = meta.modified() else {
                continue;
            };
            newest = Some(match newest {
                Some(prev) if prev >= mtime => prev,
                _ => mtime,
            });
        }
    }
    newest
}

fn rust_inputs_changed(app_dir: &Path, baseline: Option<SystemTime>) -> bool {
    match (baseline, rust_input_mtime(app_dir)) {
        (Some(base), Some(now)) => now > base,
        (None, Some(_)) => true,
        _ => false,
    }
}

const POLL: Duration = Duration::from_millis(200);
const DEBOUNCE: Duration = Duration::from_millis(300);

#[cfg(unix)]
extern "C" fn on_shutdown(_: libc::c_int) {
    SHUTDOWN.store(true, Ordering::SeqCst);
    let pgid = CHILD_PGID.load(Ordering::SeqCst);
    if pgid != 0 {
        unsafe {
            libc::kill(-(pgid as i32), libc::SIGTERM);
        }
    }
}

fn install_shutdown_handler() {
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGINT, on_shutdown as libc::sighandler_t);
        libc::signal(libc::SIGTERM, on_shutdown as libc::sighandler_t);
    }
}

fn spawn_cargo(
    manifest: &Path,
    cargo_args: &[String],
    icon: Option<&Path>,
) -> std::io::Result<Child> {
    let mut cmd = Command::new("cargo");
    cmd.arg("run")
        .arg("--manifest-path")
        .arg(manifest)
        .args(cargo_args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    if let Some(icon) = icon {
        cmd.env("LIBRA_ICON_PATH", icon);
    }
    // Own process group so a restart signal hits `cargo` and the app
    // binary together, and Ctrl-C stays with this supervisor.
    #[cfg(unix)]
    {
        cmd.process_group(0);
    }
    cmd.spawn()
}

fn stop_child(child: &mut Child) {
    let pid = child.id();
    #[cfg(unix)]
    unsafe {
        libc::kill(-(pid as i32), libc::SIGTERM);
    }
    #[cfg(not(unix))]
    {
        let _ = child.kill();
    }
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                CHILD_PGID.store(0, Ordering::SeqCst);
                return;
            }
            Ok(None) if start.elapsed() > Duration::from_secs(2) => {
                #[cfg(unix)]
                unsafe {
                    libc::kill(-(pid as i32), libc::SIGKILL);
                }
                #[cfg(not(unix))]
                {
                    let _ = child.kill();
                }
                let _ = child.wait();
                CHILD_PGID.store(0, Ordering::SeqCst);
                return;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(_) => {
                CHILD_PGID.store(0, Ordering::SeqCst);
                return;
            }
        }
    }
}

enum Step {
    Restart,
    Stop(ExitCode),
}

/// Run until the window closes, the user interrupts, or Rust inputs change.
fn supervise_once(
    manifest: &Path,
    app_dir: &Path,
    cargo_args: &[String],
    icon: Option<&Path>,
) -> Step {
    let baseline = rust_input_mtime(app_dir);
    let mut child = match spawn_cargo(manifest, cargo_args, icon) {
        Ok(child) => child,
        Err(e) => {
            eprintln!("libra dev: failed to spawn cargo: {e}");
            return Step::Stop(ExitCode::from(2));
        }
    };
    CHILD_PGID.store(child.id(), Ordering::SeqCst);

    loop {
        if SHUTDOWN.load(Ordering::SeqCst) {
            stop_child(&mut child);
            return Step::Stop(ExitCode::from(130));
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                CHILD_PGID.store(0, Ordering::SeqCst);
                if SHUTDOWN.load(Ordering::SeqCst) {
                    return Step::Stop(ExitCode::from(130));
                }
                if status.success() {
                    return Step::Stop(ExitCode::SUCCESS);
                }
                eprintln!("libra dev: cargo exited {status}; waiting for the next Rust save");
                return wait_for_rust_save(app_dir, baseline);
            }
            Ok(None) => {}
            Err(e) => {
                eprintln!("libra dev: {e}");
                CHILD_PGID.store(0, Ordering::SeqCst);
                return Step::Stop(ExitCode::FAILURE);
            }
        }
        if rust_inputs_changed(app_dir, baseline) && debounced_change(app_dir, baseline) {
            eprintln!("libra dev: Rust change — rebuilding");
            stop_child(&mut child);
            return Step::Restart;
        }
        std::thread::sleep(POLL);
    }
}

fn debounced_change(app_dir: &Path, baseline: Option<SystemTime>) -> bool {
    std::thread::sleep(DEBOUNCE);
    if SHUTDOWN.load(Ordering::SeqCst) {
        return false;
    }
    rust_inputs_changed(app_dir, baseline)
}

/// A failed `cargo run` stays down until the app's Rust inputs change,
/// so a compile error does not restart in a loop.
fn wait_for_rust_save(app_dir: &Path, baseline: Option<SystemTime>) -> Step {
    loop {
        if SHUTDOWN.load(Ordering::SeqCst) {
            return Step::Stop(ExitCode::from(130));
        }
        if rust_inputs_changed(app_dir, baseline) && debounced_change(app_dir, baseline) {
            eprintln!("libra dev: Rust change — rebuilding");
            return Step::Restart;
        }
        std::thread::sleep(POLL);
    }
}

pub fn dev(args: &[String]) -> ExitCode {
    let (root, cargo_args) = parse_dev_args(args);

    let manifest = match find_app_manifest(&root) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{e}");
            eprintln!(
                "hint: run from an app directory, or pass a path — e.g. `libra dev examples/counter`"
            );
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
    let icon = try_icon_path(&app_dir);

    eprintln!("libra dev: running {name} (debug — §18.2)");
    eprintln!("  manifest: {}", manifest.display());
    eprintln!("  .lx and theme edits reload in place; Rust edits rebuild and restart");
    if !cargo_args.is_empty() {
        eprintln!("  args: {}", cargo_args.join(" "));
    }
    if let Some(icon) = &icon {
        eprintln!("  icon: {}", icon.display());
    }

    install_shutdown_handler();
    loop {
        match supervise_once(&manifest, &app_dir, &cargo_args, icon.as_deref()) {
            Step::Restart => continue,
            Step::Stop(code) => return code,
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

    #[test]
    fn validate_accepts_facade_only_dep() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            r#"
[package]
name = "starter"
version = "0.1.0"
edition = "2021"

[dependencies]
libra-ui = { path = "../../crates/libra" }
serde = { version = "1", features = ["derive"] }
"#,
        )
        .unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();

        let source = fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
        assert_eq!(validate_libra_app(&source, dir.path()), None);
    }

    #[test]
    fn rust_mtime_ignores_views_and_build_artifacts() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("views")).unwrap();
        fs::create_dir_all(root.join("target/debug")).unwrap();
        fs::write(root.join("src/lib.rs"), "fn app() {}\n").unwrap();
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"a\"\n").unwrap();
        fs::write(root.join("build.rs"), "fn main() {}\n").unwrap();

        let before = rust_input_mtime(root).expect("rust inputs");

        fs::write(root.join("views/starter.lx"), "<text>hi</text>\n").unwrap();
        fs::write(root.join("target/debug/junk.rs"), "fn generated() {}\n").unwrap();
        assert_eq!(rust_input_mtime(root), Some(before));

        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(root.join("src/lib.rs"), "fn app() { let _ = 1; }\n").unwrap();
        let after = rust_input_mtime(root).expect("updated");
        assert!(after > before);
    }

    #[test]
    fn cargo_lock_is_a_rebuild_input() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("Cargo.lock"), "# lock\n").unwrap();
        assert!(rust_input_mtime(dir.path()).is_some());
        assert!(is_rust_rebuild_input(Path::new("Cargo.lock")));
        assert!(!is_rust_rebuild_input(Path::new("views/starter.lx")));
    }
}
