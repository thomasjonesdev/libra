//! The libra CLI (ARCHITECTURE.md §18). Phase 1 subset: `libra check` —
//! parse + validate every `.lx` under the given paths in milliseconds,
//! without building anything. Grammar-level validation only for now;
//! binding/handler validation against State/Action needs the app's
//! reflected types in scope (the interpreter already does it at app
//! load time via `#[derive(LibraReflect)]`/`#[libra::state]`/
//! `#[libra::actions]`) — wiring that into `libra check` itself is
//! still open.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.split_first() {
        Some((cmd, rest)) if cmd == "check" => check(rest),
        _ => {
            eprintln!("usage: libra check [paths…]   (default: current directory)");
            ExitCode::from(2)
        }
    }
}

fn check(paths: &[String]) -> ExitCode {
    let roots: Vec<PathBuf> = if paths.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        paths.iter().map(PathBuf::from).collect()
    };

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
            Ok(source) => match libra_markup::parse(&source) {
                Ok(_) => println!("  ok   {}", file.display()),
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

fn collect_lx(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_file() {
        if path.extension().is_some_and(|e| e == "lx") {
            out.push(path.to_path_buf());
        }
        return;
    }
    let Ok(entries) = std::fs::read_dir(path) else { return };
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
