//! The windowed fetcher — effects run for real here (KV → platform app-data
//! dir, HTTP → api.github.com/zen), the view file hot-reloads on save, and
//! state rehydrates across restarts.
//!
//! Window size, title, and dev-watch come from `libra::app!` via
//! `fetcher::options()` (the single source); this file only adds the KV
//! `app_id` and the rehydrating run.
//!
//! Architecture: ARCHITECTURE.md §18.2 (the iteration loop), §13.2 (state
//! snapshots).

use fetcher::{Fetcher, VIEW};

fn main() {
    let opts = fetcher::options().app_id("libra-fetcher");
    if let Err(e) = libra_platform::run_rehydrating::<Fetcher>(VIEW, opts) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
