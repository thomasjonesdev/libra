use std::process::ExitCode;

fn main() {
    let mut args = std::env::args();
    let _bin = args.next();
    if let Some(cmd) = args.next() {
        if gloss_cli::is_cli_command(&cmd) {
            let rest = std::iter::once(cmd).chain(args);
            let code = gloss_cli::run_args(rest);
            if code != ExitCode::SUCCESS {
                std::process::exit(1);
            }
            return;
        }
    }
    prepend_exe_dir_to_path();
    if let Err(e) = gloss::run_app() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn prepend_exe_dir_to_path() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let Some(dir) = exe.parent() else { return };
    let extra = dir.display().to_string();
    match std::env::var("PATH") {
        Ok(p) => std::env::set_var("PATH", format!("{extra}:{p}")),
        Err(_) => std::env::set_var("PATH", extra),
    }
}
