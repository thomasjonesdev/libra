//! Domain types and AWS CLI command builders for the AWS SSO session manager.
//!
//! Architecture: ARCHITECTURE.md §27.1; plan PLAN-AWS-LOGIN.

use libra_core::CommandSpec;
use serde::{Deserialize, Serialize};

pub const STORE_KEY: &str = "aws-sso-config";

pub const TICK_MS: u64 = 60_000;
pub const PROBE_INTERVAL_MS: u64 = 5 * 60 * 1000;
pub const REFRESH_INTERVAL_MS: u64 = 6 * 60 * 60 * 1000;
pub const LOGIN_TIMEOUT_MS: u64 = 10 * 60 * 1000;
pub const VERIFY_TIMEOUT_MS: u64 = 30_000;

pub const DEFAULT_SESSION: &str = "aeos-govcloud";
pub const DEFAULT_START_URL: &str =
    "https://start.us-gov-west-1.us-gov-home.awsapps.com/directory/d-98671f268f#/?tab=accounts";
pub const DEFAULT_SSO_REGION: &str = "us-gov-west-1";
pub const DEFAULT_AWS_REGION: &str = "us-gov-west-1";
pub const DEFAULT_SCOPES: &str = "sso:account:access";
pub const DEFAULT_ACCOUNT_ID: &str = "359247995521";
pub const DEFAULT_ROLE: &str = "Dev_Admin";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AppScreen {
    #[default]
    Preflight,
    Onboarding,
    Steady,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AuthStatus {
    #[default]
    Unknown,
    LoggedOut,
    LoggingIn,
    Verifying,
    Active,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ProcessStep {
    #[default]
    Idle,
    PreflightAws,
    PreflightClaude,
    LoadConfig,
    ConfigWrite,
    Login,
    Verify,
    Probe,
    ReadConfigKeys,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedConfig {
    pub session_name: String,
    pub start_url: String,
    pub sso_region: String,
    pub aws_region: String,
    pub registration_scopes: String,
    pub account_id: String,
    pub role_name: String,
    pub profile_name: String,
    pub auto_refresh: bool,
}

impl SavedConfig {
    pub fn from_form(
        session_name: &str,
        start_url: &str,
        sso_region: &str,
        aws_region: &str,
        registration_scopes: &str,
        account_id: &str,
        role_name: &str,
        profile_name: &str,
        auto_refresh: bool,
    ) -> Self {
        SavedConfig {
            session_name: session_name.into(),
            start_url: start_url.into(),
            sso_region: sso_region.into(),
            aws_region: aws_region.into(),
            registration_scopes: registration_scopes.into(),
            account_id: account_id.into(),
            role_name: role_name.into(),
            profile_name: profile_name.into(),
            auto_refresh,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, libra_core::LibraReflect)]
pub struct ConfigRow {
    pub key: String,
    pub value: String,
}

impl std::fmt::Display for ConfigRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.key)
    }
}

impl ConfigRow {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        ConfigRow { key: key.into(), value: value.into() }
    }
}

pub fn default_profile_name(role: &str, account: &str) -> String {
    format!("{role}-{account}")
}

pub fn aws_version_cmd() -> CommandSpec {
    CommandSpec::new("aws").args(["--version"]).timeout_ms(5_000)
}

pub fn claude_version_cmd() -> CommandSpec {
    CommandSpec::new("claude").args(["--version"]).timeout_ms(5_000)
}

pub fn aws_configure_set(key: &str, value: &str) -> CommandSpec {
    CommandSpec::new("aws")
        .args(["configure", "set", key, value])
        .timeout_ms(10_000)
}

pub fn aws_configure_get(key: &str) -> CommandSpec {
    CommandSpec::new("aws")
        .args(["configure", "get", key])
        .timeout_ms(10_000)
}

pub fn aws_list_profiles() -> CommandSpec {
    CommandSpec::new("aws")
        .args(["configure", "list-profiles"])
        .timeout_ms(10_000)
}

pub fn aws_sso_login(profile: &str) -> CommandSpec {
    CommandSpec::new("aws")
        .args(["sso", "login", "--profile", profile])
        .timeout_ms(LOGIN_TIMEOUT_MS)
}

pub fn aws_caller_identity(profile: &str) -> CommandSpec {
    CommandSpec::new("aws")
        .args(["sts", "get-caller-identity", "--profile", profile])
        .timeout_ms(VERIFY_TIMEOUT_MS)
}

pub fn config_write_pairs(cfg: &SavedConfig) -> Vec<(String, String)> {
    vec![
        (
            format!("sso-session.{}.sso_start_url", cfg.session_name),
            cfg.start_url.clone(),
        ),
        (
            format!("sso-session.{}.sso_region", cfg.session_name),
            cfg.sso_region.clone(),
        ),
        (
            format!("sso-session.{}.sso_registration_scopes", cfg.session_name),
            cfg.registration_scopes.clone(),
        ),
        (
            format!("profile.{}.sso_session", cfg.profile_name),
            cfg.session_name.clone(),
        ),
        (
            format!("profile.{}.sso_account_id", cfg.profile_name),
            cfg.account_id.clone(),
        ),
        (
            format!("profile.{}.sso_role_name", cfg.profile_name),
            cfg.role_name.clone(),
        ),
        (format!("profile.{}.region", cfg.profile_name), cfg.aws_region.clone()),
    ]
}

pub fn config_read_key_list(cfg: &SavedConfig) -> Vec<String> {
    config_write_pairs(cfg).into_iter().map(|(k, _)| k).collect()
}

pub fn parse_aws_major_version(stdout: &[u8]) -> Option<u32> {
    let text = String::from_utf8_lossy(stdout);
    for part in text.split_whitespace() {
        if let Some(rest) = part.strip_prefix("aws-cli/") {
            let major = rest.split('.').next()?.parse().ok()?;
            return Some(major);
        }
    }
    None
}

pub fn parse_caller_identity(stdout: &[u8]) -> Option<(String, String)> {
    let v: serde_json::Value = serde_json::from_slice(stdout).ok()?;
    let account = v.get("Account")?.as_str()?.to_string();
    let arn = v.get("Arn")?.as_str()?.to_string();
    Some((account, arn))
}

pub fn format_duration_ago(t_ms: f64, then_t_ms: f64) -> String {
    if then_t_ms <= 0.0 {
        return "not yet verified".into();
    }
    let mins = ((t_ms - then_t_ms) / 60_000.0).max(0.0) as u64;
    if mins < 1 {
        "verified just now".into()
    } else if mins < 60 {
        format!("verified {mins} minute{} ago", if mins == 1 { "" } else { "s" })
    } else {
        let hours = mins / 60;
        format!("verified {hours} hour{} ago", if hours == 1 { "" } else { "s" })
    }
}

pub fn format_next_refresh(t_ms: f64, last_login_t_ms: f64, auto_refresh: bool) -> String {
    if !auto_refresh {
        return "automatic refresh off".into();
    }
    if last_login_t_ms <= 0.0 {
        return "next refresh after first login".into();
    }
    let remaining = REFRESH_INTERVAL_MS as f64 - (t_ms - last_login_t_ms);
    if remaining <= 0.0 {
        return "refresh due".into();
    }
    let mins = (remaining / 60_000.0).ceil() as u64;
    if mins < 60 {
        format!("next refresh in {mins} minute{}", if mins == 1 { "" } else { "s" })
    } else {
        let hours = mins / 60;
        format!("next refresh in {hours} hour{}", if hours == 1 { "" } else { "s" })
    }
}

pub fn validate_onboarding(
    account_id: &str,
    start_url: &str,
    profile_name: &str,
    existing_profiles: &[String],
) -> Result<(), String> {
    if account_id.len() != 12 || !account_id.chars().all(|c| c.is_ascii_digit()) {
        return Err("account id must be 12 digits".into());
    }
    if !start_url.starts_with("https://") {
        return Err("start URL must begin with https://".into());
    }
    if profile_name.trim().is_empty() {
        return Err("profile name is required".into());
    }
    if existing_profiles.iter().any(|p| p == profile_name) {
        return Err(format!("profile '{profile_name}' already exists — choose another name"));
    }
    Ok(())
}

pub fn url_fragment_warning(start_url: &str) -> String {
    if start_url.contains('#') {
        "This URL contains a # fragment. AWS usually stores sso_start_url without it — you can strip it if login fails.".into()
    } else {
        String::new()
    }
}

pub fn export_profile_line(profile: &str) -> String {
    format!("export AWS_PROFILE={profile}")
}

pub fn configure_sso_command() -> String {
    "aws configure sso".into()
}
