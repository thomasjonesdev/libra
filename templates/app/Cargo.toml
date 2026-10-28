# Standalone Libra app (PLAN-SDK S3). Not a member of the framework
# workspace — a nested `[workspace]` is a Cargo error.
#
# Copy this folder, or `libra new`. `libra new` with no flags uses path
# deps when this CLI was built from a checkout, otherwise crates.io
# (`libra-ui`). `--path` / `--git` still override.
#
# No dummy `[workspace]` table: `libra build` / `package` / `test` fall
# back to this package directory (`find_workspace_root`, §18.1).

[package]
name = "starter"
version = "0.1.0"
edition = "2021"
license = "Apache-2.0"

[dependencies]
# Package name on crates.io is `libra-ui` (§26 #7). The Rust crate name
# stays `libra` (`use libra::…`).
libra-ui = { path = "../../crates/libra" }
# serde cannot be hidden: `#[libra::state]` bundles its derive, which
# expands `extern crate serde as _serde`.
serde = { version = "1", features = ["derive"] }

[dev-dependencies]
libra-test = { path = "../../crates/libra-test" }

[build-dependencies]
libra-build = { path = "../../crates/libra-build" }

# Copied from the framework workspace. A standalone package is its own
# implicit workspace, so these profile tables apply. Without them,
# `libra dev` runs the paint path at opt-level 0 (unusable) and
# `libra build` has no LTO.
[profile.dev.package."*"]
opt-level = 2

[profile.dev.package.libra-raster]
opt-level = 2

[profile.dev.package.libra-draw]
opt-level = 2

[profile.dev.package.libra-data]
opt-level = 2

[profile.dev.package.libra-components]
opt-level = 2

[profile.dev.package.libra-platform]
opt-level = 2

[profile.release]
lto = "thin"
codegen-units = 1
