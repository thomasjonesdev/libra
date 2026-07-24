//! `libra` — the facade crate: depend on this to get the
//! `#[libra::state]` / `#[libra::actions]` / `libra::app!` macros at their
//! documented import path, plus common `libra-core` types re-exported.
//!
//! Direct-dependency note: the generated code references `::libra_core::` and
//! (for `app!`) `::libra_platform::` directly, so a crate using these macros
//! needs `libra-core` (always) and `libra-platform` (if it uses `app!`) as
//! direct dependencies too, alongside `libra`.
//!
//! Architecture: ARCHITECTURE.md §21 (workspace & crate layout).

pub use libra_core::*;
pub use libra_reflect::{actions, app, state, verifies, LibraReflect};
