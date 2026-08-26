//! Gloss — first-party Libra app for Markdown vaults.
//!
//! The headless core lives in `gloss-core`. This crate is the editor
//! shell. Writes go out as Process effects to `gloss-cli`.
//!
//! Module layout follows §5.9: `model/` (pure domain), `state/` (the
//! `#[libra::state]` struct), `actions/` (the `#[libra::actions]` block),
//! and this file (wiring: re-exports + `app!`).
//!
//! Architecture: ARCHITECTURE.md §5.9, §27.

mod actions;
mod model;
mod state;

pub use actions::{reduce, Action};
pub use model::{
    assemble_note, default_daily_note_path, file_stem, journal_path, note_snippet, note_title,
    parse_list, parse_workspace_bytes, project_file_rows, project_preview, project_search_rows,
    resolve_daily_note, resolve_wikilink, split_note, vault_name, IoKind, Pending, SidebarView,
    VaultRecord, WorkspaceSettings, AUTOSAVE_MS, STORE_KEY,
};
pub use state::Gloss;

libra::app! {
    state: Gloss,
    view: "../views/gloss.lx",
    init: Gloss::init,
    shortcuts: Gloss::shortcuts,
    menu: Gloss::menu,
    context_menu: Gloss::context_menu,
    window: (960.0, 640.0),
    title: "Gloss",
}

pub fn app_options() -> libra_platform::Options {
    options()
        .app_id("gloss")
        .process_allowlist(["gloss", "gloss-cli"])
        .frameless(true)
}

pub fn run_app() -> Result<(), Box<dyn std::error::Error>> {
    libra_platform::run_rehydrating::<Gloss>(view_driver(), app_options())
}
