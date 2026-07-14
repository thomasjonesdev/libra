//! `libra` — the facade crate (ARCHITECTURE.md §21): depend on this
//! one crate to get the `#[libra::state]` / `#[libra::actions]` /
//! `libra::app!` macros at their documented import path, plus the
//! common `libra-core` types re-exported for convenience.
//!
//! **Direct-dependency note:** the code these macros generate
//! references `::libra_core::` and (for `app!`) `::libra_platform::`
//! paths directly — not `::libra::` — because that's what
//! `#[libra::state]`/`#[libra::actions]` already did before this
//! crate existed (PLAN-ERGONOMICS.md's Stage 4 landed first, tested
//! via the direct `libra_reflect::{state, actions}` import). So a
//! crate using these macros needs `libra-core` (always) and
//! `libra-platform` (if it uses `app!`) as *direct* Cargo.toml
//! dependencies too, alongside `libra` — the same dependency set
//! every example in this workspace already declares. `libra` doesn't
//! change that; it only adds the convenient macro import path.
pub use libra_core::*;
pub use libra_reflect::{actions, app, state, LibraReflect};
