//! Notes — the feature-parity reference app (folders sidebar · note list ·
//! editor), modeled after the vercel-labs/native notes example (capability
//! parity through Libra's own contracts). `Folder`/`Note` are the source of
//! truth; the `*_rows` / `has_trash` / … fields are §6.3 display projections
//! rebuilt by the `#[project]` hook. The store round-trips through one
//! `Effect::KvGet`/`KvSet` as versioned JSON; edits debounce through
//! `Effect::Timer` with exactly one save in flight.
//!
//! Module layout follows §5.9: `model.rs` (pure domain), `state.rs` (the
//! `#[libra::state]` struct), `actions.rs` (the `#[libra::actions]` block),
//! and this file (wiring: re-exports + `app!`). The re-exports below keep
//! `Action`/`reduce`/`Notes` at the crate root — the §5.9 flat surface.
//!
//! Architecture: ARCHITECTURE.md §5.9 (module layout), §6.3 (display
//! projections); plans PLAN-NOTES-APP, PLAN-TEXTAREA, PLAN-CATALOG.

mod actions;
mod model;
mod state;

pub use actions::{reduce, Action};
pub use model::{
    Folder, FolderRow, Note, NoteRow, AUTOSAVE_DEBOUNCE_MS, MAX_BODY_BYTES, MAX_FOLDERS,
    MAX_NOTES, RECENTLY_DELETED, STORE_KEY,
};
pub use state::Notes;

libra::app! {
    state: Notes,
    view: "../views/notes.lx",
    init: Notes::init,
    window: (900.0, 600.0),
    title: "Libra Notes",
}
