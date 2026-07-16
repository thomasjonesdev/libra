//! The windowed counter — the same app the test suite exercises, in a
//! real window. 400×240 logical matches the snapshot-test viewport.
//! In debug builds `run()` watches the view file: edit counter.lx and
//! save to hot reload (state preserved).

fn main() {
    if let Err(e) = counter::run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
