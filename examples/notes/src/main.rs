//! Windowed notes — click the field and type for real: the shell's
//! keyboard model drives the same editing core the tests drive.

use libra_platform::Options;
use notes::{Notes, VIEW};

fn main() {
    let mut opts = Options::new("Libra Notes", (520.0, 200.0)).app_id("libra-notes");
    let dev_view = std::path::Path::new("examples/notes/views/notes.lx");
    if dev_view.exists() {
        opts = opts.watch(dev_view);
    }
    if let Err(e) = libra_platform::run_with::<Notes>(VIEW, opts) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
