//! Windowed gallery — toggle the theme live; pass a theme file to
//! preview a custom table: `cargo run -p gallery -- my-theme.toml`
//! (edit + save while running: it hot-reloads).

use gallery::{Gallery, VIEW};
use libra_platform::Options;

fn main() {
    let mut opts = Options::new("Libra Gallery", (640.0, 360.0));
    if let Some(theme_path) = std::env::args().nth(1) {
        opts = opts.theme_file(theme_path);
    }
    if let Err(e) = libra_platform::run_with::<Gallery>(VIEW, opts) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
