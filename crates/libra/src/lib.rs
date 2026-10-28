//! `libra` — the facade crate: depend on this to get the
//! `#[libra::state]` / `#[libra::actions]` / `libra::app!` macros at their
//! documented import path, plus common `libra-core` types re-exported.
//!
//! PLAN-SDK S2: generated tokens name `::libra::` (and
//! `::libra::__private::` for catalog IR). An app crate's
//! `[dependencies]` is `libra` + `serde` — serde cannot be hidden
//! because its derive expands `extern crate serde as _serde`.
//! `libra-build` (build-dep) and `libra-test` (dev-dep) stay separate.
//!
//! Default features include `app` (the windowed shell). Crates that only
//! derive reflection (the conformance suite) take
//! `default-features = false` so they do not pull winit.
//!
//! Architecture: ARCHITECTURE.md §21 (workspace & crate layout).

pub use libra_core::*;
pub use libra_reflect::{actions, app, state, verifies, LibraReflect};

#[cfg(feature = "app")]
pub use libra_platform::{run_rehydrating, run_with, Options, ViewDriver};

/// Paths emitted by `app!` / `libra-codegen` so release `view.rs` does
/// not require `libra-components` or `libra-platform` as direct app deps.
#[doc(hidden)]
pub mod __private {
    pub use libra_components as components;
    #[cfg(feature = "app")]
    pub use libra_platform as platform;
}
