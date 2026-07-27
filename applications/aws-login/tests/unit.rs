//! Unit tests — pure helpers and validation (§15.2).

use aws_login::model::{
    format_duration_ago, format_next_refresh, parse_aws_major_version, parse_caller_identity,
    url_fragment_warning, validate_onboarding, REFRESH_INTERVAL_MS,
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
fn validate_account_id_requires_twelve_digits() {
    assert!(validate_onboarding("123", "https://x.example", "p", &[]).is_err());
    assert!(validate_onboarding("123456789012", "https://x.example", "p", &[]).is_ok());
}

#[test]
fn validate_profile_collision() {
    assert!(validate_onboarding(
        "123456789012",
        "https://x.example",
        "existing",
        &["existing".into()]
    )
    .is_err());
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
