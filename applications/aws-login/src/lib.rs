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
    tray: tray,
}

fn tray(app: &AwsLogin) -> Option<libra_core::TrayDecl> {
    use crate::model::AuthStatus;
    use libra_core::{TrayDecl, TrayIcon, TrayItem};
    let icon = match app.auth {
        AuthStatus::Active => TrayIcon::Ok,
        AuthStatus::Failed => TrayIcon::Error,
        AuthStatus::LoggedOut => TrayIcon::Warn,
        _ => TrayIcon::Default,
    };
    Some(TrayDecl::new(
        "AWS Session",
        icon,
        vec![
            TrayItem::action("Authenticate", "authenticate").enabled(!app.is_busy),
            TrayItem::action("Show window", "showWindow"),
            TrayItem::separator(),
            TrayItem::action("Quit", "quit"),
        ],
    ))
}

/// Window options with KV identity and the §12.6 process allowlist.
pub fn app_options() -> libra_platform::Options {
    options()
        .app_id("aws-login")
        .process_allowlist(["aws", "claude", "sh"])
        .window_policy(libra_core::WindowPolicy::CloseToTray)
        .single_instance(true)
}

pub fn run_app() -> Result<(), Box<dyn std::error::Error>> {
    libra_platform::run_rehydrating::<AwsLogin>(view_driver(), app_options())
}
