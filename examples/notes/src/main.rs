//! Windowed notes — folders sidebar, note list, editor. Window size,
//! title, and dev-watch come from `libra::app!` via `notes::options()`;
//! this file only adds the KV `app_id` so boot `KvGet` and autosave
//! `KvSet` hit the platform app-data store.

fn main() {
    let opts = notes::options().app_id("libra-notes");
    if let Err(e) = libra_platform::run_with::<notes::Notes>(notes::VIEW, opts) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
