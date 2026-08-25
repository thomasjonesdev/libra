//! AWS SSO session manager — onboarding once, one Authenticate button, and
//! automatic refresh on a six-hour cadence (§27.1).
//!
//! Architecture: ARCHITECTURE.md §27.1; plan PLAN-AWS-LOGIN.

mod actions;
pub mod model;
mod state;

pub use actions::{reduce, Action};
pub use model::STORE_KEY;
pub use state::AwsLogin;

libra::app! {
    state: AwsLogin,
    view: "../views/aws-login.lx",
    init: AwsLogin::init,
    window: (420.0, 640.0),
    title: "AWS Session",
}

/// Window options with KV identity and the §12.6 process allowlist.
pub fn app_options() -> libra_platform::Options {
    options()
        .app_id("aws-login")
        .process_allowlist(["aws", "claude", "sh"])
}

pub fn run_app() -> Result<(), Box<dyn std::error::Error>> {
    libra_platform::run_rehydrating::<AwsLogin>(view_driver(), app_options())
}
