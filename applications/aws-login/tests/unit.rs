//! Unit tests — pure helpers and validation (§15.2).

use aws_login::model::{
    account_id_from_profile, find_profile_for_role, format_duration_ago, format_next_refresh,
    parse_aws_major_version, parse_caller_identity, parse_sso_access_token, parse_sso_account_ids,
    sso_token_cache_paths, strip_url_fragment, url_fragment_warning, validate_onboarding,
    REFRESH_INTERVAL_MS,
};

#[test]
fn parse_aws_v2_major_version() {
    let out = b"aws-cli/2.15.0 Python/3.11.6 Darwin/23.0.0";
    assert_eq!(parse_aws_major_version(out), Some(2));
}

#[test]
fn parse_aws_v1_is_not_v2() {
    let out = b"aws-cli/1.32.0 Python/3.11.6";
    assert_eq!(parse_aws_major_version(out), Some(1));
}

#[test]
fn validate_requires_session_and_role() {
    assert!(validate_onboarding("", "https://x.example", "Dev_Admin").is_err());
    assert!(validate_onboarding("my-session", "https://x.example", "").is_err());
    assert!(validate_onboarding("my-session", "https://x.example", "Dev_Admin").is_ok());
}

#[test]
fn parse_sso_accounts_json() {
    let json = br#"{"accountList":[{"accountId":"111122223333","accountName":"test"}]}"#;
    assert_eq!(
        parse_sso_account_ids(json).unwrap(),
        vec!["111122223333".to_string()]
    );
}

#[test]
fn sso_token_cache_paths_use_session_then_start_url() {
    if std::env::var_os("HOME").is_none() && std::env::var_os("USERPROFILE").is_none() {
        return;
    }
    let paths = sso_token_cache_paths("my-session", "https://start.example.awsapps.com/start");
    assert_eq!(paths.len(), 2);
    assert_eq!(paths[0].extension(), Some(std::ffi::OsStr::new("json")));
    assert_eq!(paths[1].extension(), Some(std::ffi::OsStr::new("json")));
    assert_ne!(paths[0], paths[1]);
}

#[test]
fn account_id_from_role_profile_name() {
    assert_eq!(
        account_id_from_profile("Dev_Admin-111122223333"),
        Some("111122223333".into())
    );
    assert!(account_id_from_profile("Dev_Admin").is_none());
}

#[test]
fn find_profile_for_role_prefers_role_account_pattern() {
    let profiles = vec![
        "other".into(),
        "Dev_Admin-111122223333".into(),
        "Dev_Admin".into(),
    ];
    assert_eq!(
        find_profile_for_role("Dev_Admin", &profiles),
        Some("Dev_Admin-111122223333".into())
    );
}

#[test]
fn parse_sso_access_token_json() {
    let json = br#"{"accessToken":"tok-abc","expiresAt":"2099-01-01T00:00:00Z"}"#;
    assert_eq!(parse_sso_access_token(json).unwrap(), "tok-abc");
}

#[test]
fn parse_sso_access_token_missing_field_errs() {
    assert!(parse_sso_access_token(br#"{"expiresAt":"2099-01-01T00:00:00Z"}"#).is_err());
}

#[test]
fn strip_url_fragment_removes_hash_suffix() {
    assert_eq!(
        strip_url_fragment("https://start.example/d-abc#/?tab=accounts"),
        "https://start.example/d-abc"
    );
    assert_eq!(strip_url_fragment("https://start.example"), "https://start.example");
}

#[test]
fn url_fragment_warning_only_when_present() {
    assert!(url_fragment_warning("https://x#tab").contains("fragment"));
    assert!(url_fragment_warning("https://x").is_empty());
}

#[test]
fn parse_caller_identity_json() {
    let json = br#"{"Account":"111122223333","Arn":"arn:aws-us-gov:sts::111122223333:assumed-role/Dev_Admin/user"}"#;
    let (acct, arn) = parse_caller_identity(json).expect("parses");
    assert_eq!(acct, "111122223333");
    assert!(arn.contains("assumed-role"));
}

#[test]
fn duration_and_refresh_labels_are_deterministic() {
    assert_eq!(format_duration_ago(120_000.0, 60_000.0), "verified 1 minute ago");
    assert_eq!(
        format_next_refresh(REFRESH_INTERVAL_MS as f64 + 60_001.0, 60_000.0, true),
        "refresh due"
    );
}
