//! The libra CLI (ARCHITECTURE.md §18). Phase 1 subset:
//! - `libra check` — parse + validate every `.lx` under the given paths
//!   in milliseconds, without building anything. Grammar-level validation
//!   only for now; binding/handler validation against State/Action needs
//!   the app's reflected types in scope (the interpreter already does it
//!   at app load time via `#[derive(LibraReflect)]`/`#[libra::state]`/
//!   `#[libra::actions]`) — wiring that into `libra check` itself is
//!   still open.
//! - `libra dev` — run a Libra app crate in debug mode (§18.2's
//!   interpreter + view hot reload), set `LIBRA_AUTOMATION=1` for the
//!   §16 socket, and open the inspector window (§18.4).
//! - `libra build` — release binary with codegen views + LTO (PLAN-PACKAGE P1).
//! - `libra package` — platform bundles via cargo-packager (PLAN-PACKAGE P3–P6).
//!   This stays the local backup: it never calls GitHub.
//! - `libra icon` — one square PNG to the desktop icon set (PLAN-RELEASE).
//! - `libra publish` — upload that bundle, or write the release workflow.

mod app;
mod build;
mod dev;
mod docs;
mod icon;
mod lsp;
mod manifest;
mod new;
mod package;
mod publish;
mod test;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.split_first() {
        Some((cmd, rest)) if cmd == "new" => new::new(rest),
        Some((cmd, rest)) if cmd == "check" => check(rest),
        Some((cmd, rest)) if cmd == "dev" => dev::dev(rest),
        Some((cmd, rest)) if cmd == "build" => build::build(rest),
        Some((cmd, rest)) if cmd == "package" => package::package(rest),
        Some((cmd, rest)) if cmd == "icon" => icon::icon(rest),
        Some((cmd, rest)) if cmd == "publish" => publish::publish(rest),
        Some((cmd, rest)) if cmd == "test" => test::test(rest),
        Some((cmd, rest)) if cmd == "lsp" => lsp::lsp(),
        Some((cmd, rest)) if cmd == "docs" => docs::docs(rest),
        _ => {
            eprintln!("usage:");
            eprintln!("  libra new <path> [--name crate] [--path checkout | --git url]");
            eprintln!("  libra check [paths…]          validate .lx markup (default: workspace)");
            eprintln!("  libra dev [path] [-- args…]   run app in debug with view hot reload");
            eprintln!("  libra build [path] [-p crate] release binary (codegen + LTO)");
            eprintln!("  libra icon <png> [-o dir]     desktop icon set from one square PNG");
            eprintln!("  libra package [-p crate] [--target macos|windows|linux] [--format …]  platform bundle (local backup)");
            eprintln!("  libra publish [-p crate] [--ci | --no-upload | --from-dist | --keygen]");
            eprintln!("  libra test [path] [filter…]   run tests (no path = workspace)");
            eprintln!("  libra test --replay <file.lreplay>   replay one session (§15.6)");
            eprintln!("  libra lsp                     diagnostics language server (stdio)");
            eprintln!("  libra docs [out]              static gallery site");
            ExitCode::from(2)
        }
    }
}

fn check(paths: &[String]) -> ExitCode {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let roots = check_roots(paths, &cwd);

    let mut files = Vec::new();
    for root in &roots {
        collect_lx(root, &mut files);
    }
    if files.is_empty() {
        eprintln!("libra check: no .lx files found under {roots:?}");
        return ExitCode::from(2);
    }
    files.sort();

    let started = std::time::Instant::now();
    let mut errors = 0usize;
    for file in &files {
        match std::fs::read_to_string(file) {
            Ok(source) => match libra_markup::parse_file(&source) {
                Ok(libra_markup::LxFile::View(_)) => println!("  ok   {}", file.display()),
                Ok(libra_markup::LxFile::Component(_)) => {
                    println!("  ok   {} (component)", file.display())
                }
                Err(e) => {
                    errors += 1;
                    // The parser's spans are line:col; prefix the file for
                    // a clickable file:line:col diagnostic.
                    println!("  ERR  {}:{e}", file.display());
                }
            },
            Err(e) => {
                errors += 1;
                println!("  ERR  {}: {e}", file.display());
            }
        }
    }
    println!(
        "libra check: {} file(s), {} error(s), {:?}",
        files.len(),
        errors,
        started.elapsed()
    );
    if errors > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// No paths → the `[workspace]` root containing `cwd` (everything), or
/// `.` when there isn't one. Extra paths are a scope, same as
/// `libra test templates/app`.
fn check_roots(paths: &[String], cwd: &Path) -> Vec<PathBuf> {
    if paths.is_empty() {
        match app::find_workspace_root(cwd) {
            Ok(root) => vec![root],
            Err(_) => vec![PathBuf::from(".")],
        }
    } else {
        paths.iter().map(PathBuf::from).collect()
    }
}

fn collect_lx(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_file() {
        if path.extension().is_some_and(|e| e == "lx") {
            out.push(path.to_path_buf());
        }
        return;
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        // Skip build output and VCS internals.
        let name = entry.file_name();
        if name == "target" || name == ".git" {
            continue;
        }
        collect_lx(&p, out);
    }
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

    #[test]
    fn check_no_paths_uses_workspace_root() {
        let roots = check_roots(&[], &repo_root().join("examples/counter"));
        assert_eq!(roots, vec![repo_root()]);
    }

    #[test]
    fn check_paths_are_a_scope() {
        let roots = check_roots(&["templates/app".into()], &repo_root());
        assert_eq!(roots, vec![PathBuf::from("templates/app")]);
    }
}
