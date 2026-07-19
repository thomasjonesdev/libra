//! libra workbench — a Storybook-style component browser (PLAN-GALLERY): a
//! grouped sidebar of every catalog component; select one to see it in
//! isolation with its variants and (where it has a text prop) a live control.
//! Built entirely on the closed grammar — each component's story is static
//! markup gated by a `#[project]`-derived `is_*` bool; no dynamic dispatch.
//!
//! Module layout follows §5.9: `model.rs` (the component registry), `state.rs`
//! (the `#[libra::state]` struct), `actions.rs` (the `#[libra::actions]` block
//! + `#[project]`), and this file (wiring: re-exports + `app!`).
//!
//! Architecture: ARCHITECTURE.md §18 (developer experience), §7.1 (catalog),
//! Appendix C (Storybook); plans PLAN-GALLERY, PLAN-COMPONENTS.

mod actions;
mod model;
mod state;

pub use actions::{reduce, Action};
pub use model::ComponentRow;
pub use state::{Gallery, WAVE};

libra::app! {
    state: Gallery,
    view: "../views/gallery.lx",
    init: Gallery::init,
    window: (1200.0, 760.0),
    title: "Libra Workbench",
}
