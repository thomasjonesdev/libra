//! Gloss — first-party Libra app for Markdown vaults.
//!
//! The headless core lives in `gloss-core`. This crate is the editor
//! shell. Writes go out as Process effects to `gloss-cli`.
//!
//! Architecture: ARCHITECTURE.md §5.9, §27.

mod actions;
mod model;
mod state;

pub use actions::{reduce, Action};
pub use model::{
    assemble_note, default_daily_note_path, file_stem, journal_path, note_snippet, note_title,
    parse_list, project_file_rows, project_preview, resolve_daily_note, resolve_wikilink, split_note,
    vault_name, IoKind, Pending, STORE_KEY,
};
pub use state::Gloss;

libra::app! {
    state: Gloss,
    view: "../views/gloss.lx",
    init: Gloss::init,
    shortcuts: Gloss::shortcuts,
    window: (960.0, 640.0),
    title: "Gloss",
}

pub fn app_options() -> libra_platform::Options {
    options()
        .app_id("gloss")
        .process_allowlist(["gloss", "gloss-cli"])
}

pub fn run_app() -> Result<(), Box<dyn std::error::Error>> {
    libra_platform::run_rehydrating::<Gloss>(view_driver(), app_options())
}
