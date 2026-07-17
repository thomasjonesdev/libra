//! Notes — the feature-parity reference app for [[PLAN-NOTES-APP]]:
//! folders sidebar · note list · editor, modeled after the
//! `vercel-labs/native` notes example (capability parity, not their
//! APIs — every feature lands through Libra's own contracts).
//!
//! **M0:** `Folder`/`Note` are the source of truth (`#[libra(skip)]`);
//! `folder_rows`/`note_rows`/`has_trash`/… are §6.3 display
//! projections, rebuilt after every action by the `#[project]` hook so
//! title/snippet/word-count/relative-time can never go stale. Soft
//! delete (`deleted_ms`) backs "Recently Deleted". The whole store
//! round-trips through one `Effect::KvGet`/`KvSet` (`STORE_KEY`) as
//! versioned JSON with v1→v2 migration; boot restores before first
//! render; edits debounce through `Effect::Timer` with exactly one
//! save in flight (`save_gen` supersession, per §12.3).
//!
//! **M1:** a three-pane view built from shipped components
//! (`Column`/`Row`/`Text`/`Button`/`Input`, `<for>`, `<if>`,
//! `<Interval>`). Interim limits upgraded by later stages: the editor
//! is a single-line `Input` until `Textarea` lands ([[PLAN-TEXTAREA]]),
//! rows are `Button` + `Text` until `ListItem` lands ([[PLAN-CATALOG]]).
//!
//! Module layout follows §5.9: `model.rs` (pure domain — types, store
//! format, derivations), `state.rs` (the `#[libra::state]` struct +
//! helpers), `actions.rs` (the one `#[libra::actions]` block), and
//! this file (wiring: re-exports + `app!`). The re-exports below are
//! the §5.9 contract — `app!` and the tests resolve `Action`/`reduce`/
//! `Notes` at the crate root, same flat surface as a single-file app.

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
