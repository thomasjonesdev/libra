//! The windowed fetcher — effects run for real here: KV persists to the
//! platform app-data dir, HTTP hits api.github.com/zen. When run from
//! the repo, the view file is watched: edit fetcher.lx and save to hot
//! reload (state preserved); break it to see the error overlay. State
//! rehydrates across restarts (§18.2) — fetch a quote, close the
//! window, relaunch: the quote is still there.

use fetcher::{Fetcher, VIEW};
use libra_platform::Options;

fn main() {
    let mut opts = Options::new("Libra Fetcher", (460.0, 260.0)).app_id("libra-fetcher");
    let dev_view = std::path::Path::new("examples/fetcher/views/fetcher.lx");
    if dev_view.exists() {
        opts = opts.watch(dev_view);
    }
    if let Err(e) = libra_platform::run_rehydrating::<Fetcher>(VIEW, opts) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
