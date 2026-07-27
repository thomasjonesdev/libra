//! Domain types and AWS CLI command builders for the AWS SSO session manager.
//!
//! Architecture: ARCHITECTURE.md §27.1; plan PLAN-AWS-LOGIN.

use std::fmt;
use std::path::PathBuf;

use libra_core::{CommandSpec, LibraReflect};
use serde::{Deserialize, Serialize};

pub const STORE_KEY: &str = "aws-sso-config";

pub const TICK_MS: u64 = 60_000;
pub const PROBE_INTERVAL_MS: u64 = 5 * 60 * 1000;
pub const REFRESH_INTERVAL_MS: u64 = 6 * 60 * 60 * 1000;
pub const LOGIN_TIMEOUT_MS: u64 = 10 * 60 * 1000;
pub const VERIFY_TIMEOUT_MS: u64 = 30_000;

/// Org-wide SSO defaults — same for every user at this org (§27.1 onboarding).
pub const DEFAULT_SESSION: &str = "aeos-govcloud";
pub const DEFAULT_START_URL: &str =
    "https://start.us-gov-west-1.us-gov-home.awsapps.com/directory/d-98671f268f";
pub const DEFAULT_SSO_REGION: &str = "us-gov-west-1";
pub const DEFAULT_AWS_REGION: &str = "us-gov-west-1";
pub const DEFAULT_SCOPES: &str = "sso:account:access";
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
    LoginSession,
    ReadAccessToken,
    ListAccounts,
    Login,
    Verify,
    Probe,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, LibraReflect)]
pub struct ConfigRow {
    pub key: String,
    pub value: String,
}

impl fmt::Display for ConfigRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.key.fmt(f)
    }
}

impl ConfigRow {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        ConfigRow { key: key.into(), value: value.into() }
    }
}

pub fn default_profile_name(role: &str, account: &str) -> String {
    match (role.is_empty(), account.is_empty()) {
        (true, _) | (_, true) => String::new(),
        (false, false) => [role, "-", account].concat(),
    }
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

/// Write one field on an `[sso-session …]` block. AWS CLI ≥ 2.34.22 requires
/// `--sso-session`; the legacy dotted key (`sso-session.NAME.field`) does not
/// create new session sections reliably.
pub fn aws_configure_sso_session_set(session: &str, key: &str, value: &str) -> CommandSpec {
    CommandSpec::new("aws")
        .args(["configure", "set", "--sso-session", session, key, value])
        .timeout_ms(10_000)
}

pub fn aws_list_profiles() -> CommandSpec {
    CommandSpec::new("aws")
        .args(["configure", "list-profiles"])
        .timeout_ms(10_000)
}

pub fn aws_sso_login_session(session: &str) -> CommandSpec {
    CommandSpec::new("aws")
        .args(["sso", "login", "--sso-session", session])
        .timeout_ms(LOGIN_TIMEOUT_MS)
}

/// AWS CLI SSO cache directory: `$HOME/.aws/sso/cache` (Unix) or
/// `%USERPROFILE%\.aws\sso\cache` (Windows).
fn aws_sso_cache_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .map(|home| home.join(".aws").join("sso").join("cache"))
}

fn sha1_hex(input: &str) -> String {
    use sha1::{Digest, Sha1};
    match Sha1::digest(input.as_bytes()) {
        digest => format!("{:x}", digest),
    }
}

/// Candidate SSO token cache files — session-name hash first (CLI ≥ 2.9), then
/// start-url hash (legacy profile logins).
pub fn sso_token_cache_paths(session: &str, start_url: &str) -> Vec<PathBuf> {
    let Some(dir) = aws_sso_cache_dir() else {
        return vec![];
    };
    let mut paths = vec![
        dir.join(format!("{}.json", sha1_hex(session))),
        dir.join(format!("{}.json", sha1_hex(start_url))),
    ];
    paths.dedup();
    paths
}

/// Read a local file via subprocess — `cat` on Unix, `cmd /C type` on Windows.
pub fn read_file_cmd(path: &str) -> CommandSpec {
    #[cfg(windows)]
    {
        CommandSpec::new("cmd")
            .args(["/C", "type", path])
            .timeout_ms(10_000)
    }
    #[cfg(not(windows))]
    {
        CommandSpec::new("cat").arg(path).timeout_ms(10_000)
    }
}

/// Pick an existing AWS profile for this role (e.g. `Dev_Admin-359247995521`).
pub fn find_profile_for_role(role: &str, profiles: &[String]) -> Option<String> {
    let prefix = format!("{role}-");
    profiles
        .iter()
        .find(|p| p.starts_with(&prefix))
        .cloned()
        .or_else(|| profiles.iter().find(|p| p.as_str() == role).cloned())
}

/// Extract a 12-digit account id from a `{role}-{account}` profile name.
pub fn account_id_from_profile(profile: &str) -> Option<String> {
    let id = profile.rsplit_once('-')?.1;
    if id.len() == 12 && id.chars().all(|c| c.is_ascii_digit()) {
        Some(id.to_string())
    } else {
        None
    }
}

pub fn aws_sso_list_accounts(access_token: &str, region: &str) -> CommandSpec {
    CommandSpec::new("aws")
        .args(["sso", "list-accounts", "--access-token", access_token, "--region", region])
        .timeout_ms(VERIFY_TIMEOUT_MS)
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

pub fn config_session_write_pairs(
    _session_name: &str,
    start_url: &str,
    sso_region: &str,
    registration_scopes: &str,
) -> Vec<(String, String)> {
    match (start_url, sso_region, registration_scopes) {
        (start_url, sso_region, registration_scopes) => vec![
            ("sso_start_url".into(), start_url.into()),
            ("sso_region".into(), sso_region.into()),
            ("sso_registration_scopes".into(), registration_scopes.into()),
        ],
    }
}

fn config_session_dotted_key(session_name: &str, field: &str) -> String {
    match (session_name, field) {
        (session_name, field) => format!("sso-session.{session_name}.{field}"),
    }
}

pub fn config_profile_write_pairs(cfg: &SavedConfig) -> Vec<(String, String)> {
    match cfg {
        cfg => vec![
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
            (
                format!("profile.{}.region", cfg.profile_name),
                cfg.aws_region.clone(),
            ),
        ],
    }
}

pub fn config_write_pairs(cfg: &SavedConfig) -> Vec<(String, String)> {
    let mut pairs = config_session_write_pairs(
        &cfg.session_name,
        &cfg.start_url,
        &cfg.sso_region,
        &cfg.registration_scopes,
    )
    .into_iter()
    .map(|(field, value)| (config_session_dotted_key(&cfg.session_name, &field), value))
    .collect::<Vec<_>>();
    pairs.extend(config_profile_write_pairs(cfg));
    pairs
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

pub fn parse_sso_account_ids(stdout: &[u8]) -> Result<Vec<String>, String> {
    let v: serde_json::Value =
        serde_json::from_slice(stdout).map_err(|e| format!("could not parse SSO accounts: {e}"))?;
    let list = v
        .get("accountList")
        .and_then(|a| a.as_array())
        .ok_or_else(|| "SSO account list missing from AWS response".to_string())?;
    let mut ids = Vec::new();
    for entry in list {
        if let Some(id) = entry.get("accountId").and_then(|v| v.as_str()) {
            ids.push(id.to_string());
        }
    }
    if ids.is_empty() {
        return Err("no AWS accounts returned for this SSO session".into());
    }
    Ok(ids)
}

pub fn parse_sso_access_token(stdout: &[u8]) -> Result<String, String> {
    let v: serde_json::Value = serde_json::from_slice(stdout)
        .map_err(|_| "no cached SSO access token found — try signing in again".to_string())?;
    v.get("accessToken")
        .and_then(|t| t.as_str())
        .map(str::to_string)
        .ok_or_else(|| "SSO cache file is missing an access token".to_string())
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
    match mins {
        0 => "verified just now".into(),
        m if m < 60 => format!("verified {m} minute{} ago", if m == 1 { "" } else { "s" }),
        _ => match mins / 60 {
            hours => format!("verified {hours} hour{} ago", if hours == 1 { "" } else { "s" }),
        },
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
    match mins {
        m if m < 60 => format!("next refresh in {m} minute{}", if m == 1 { "" } else { "s" }),
        _ => match mins / 60 {
            hours => format!("next refresh in {hours} hour{}", if hours == 1 { "" } else { "s" }),
        },
    }
}

pub fn validate_onboarding(
    session_name: &str,
    start_url: &str,
    role_name: &str,
) -> Result<(), String> {
    if session_name.trim().is_empty() {
        return Err("SSO session name is required".into());
    }
    if !start_url.starts_with("https://") {
        return Err("start URL must begin with https://".into());
    }
    if role_name.trim().is_empty() {
        return Err("role name is required".into());
    }
    Ok(())
}

/// AWS stores `sso_start_url` without a `#` fragment — strip it before writing config.
pub fn strip_url_fragment(start_url: &str) -> String {
    start_url.split('#').next().unwrap_or(start_url).to_string()
}

pub fn url_fragment_warning(start_url: &str) -> String {
    if start_url.contains('#') {
        "This URL contains a # fragment. It will be removed automatically when you save.".into()
    } else {
        String::new()
    }
}

pub fn export_profile_line(profile: &str) -> String {
    match profile {
        profile => format!("export AWS_PROFILE={profile}"),
    }
}

pub fn configure_sso_command() -> String {
    "aws configure sso".into()
}
