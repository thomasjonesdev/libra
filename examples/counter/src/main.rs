//! The windowed counter — the same app the tier tests exercise, in a
//! real window. 400×240 logical matches the Tier-4 snapshot viewport.

use counter::{Counter, VIEW};

fn main() {
    if let Err(e) = libra_platform::run::<Counter>(VIEW, "Libra Counter", (400.0, 240.0)) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
