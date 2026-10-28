//! Windowed workbench — toggle the theme live; pass a theme file to preview a
//! custom table: `cargo run -p gallery -- my-theme.toml` (edit + save while
//! running: it hot-reloads).
//!
//! Window size, title, and dev-watch come from `libra::app!` via
//! `gallery::options()` (the single source) — this file only *adds* the
//! optional `theme_file`.

use gallery::Gallery;

fn main() {
    let mut opts = gallery::options();
    if opts.app_id.is_none() {
        opts = opts.app_id(env!("CARGO_PKG_NAME"));
    }
    if let Some(theme_path) = std::env::args().nth(1) {
        opts = opts.theme_file(theme_path);
    }
    if let Err(e) = libra_platform::run_rehydrating::<Gallery>(gallery::view_driver(), opts) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
