//! Snapshot tests (§15.5).

mod common;

use aws_login::model::SavedConfig;
use aws_login::Action;
use libra_core::{ProcessResult, ProcessStatus};
use libra_test::{by, fx};

use common::{harness, through_preflight_to_kv};

#[test]
fn snapshot_preflight_checking() {
    let h = harness();
    h.assert_snapshot("aws_login_preflight");
}

#[test]
fn snapshot_onboarding_defaults() {
    let mut h = harness();
    through_preflight_to_kv(&mut h);
    h.assert_snapshot("aws_login_onboarding");
}

#[test]
fn snapshot_steady_active() {
    let mut h = harness();
    through_preflight_to_kv(&mut h);
    let cfg = SavedConfig::from_form(
        "aeos-govcloud",
        "https://start.example",
        "us-gov-west-1",
        "us-gov-west-1",
        "sso:account:access",
        "111122223333",
        "Dev_Admin",
        "Dev_Admin-111122223333",
        true,
    );
    h.dispatch(Action::ConfigLoaded(Some(serde_json::to_vec(&cfg).unwrap())));
    let probe = h.expect_effect(fx::process_program("aws"));
    h.resolve_process(
        probe,
        ProcessResult {
            status: ProcessStatus::Exited(0),
            stdout: br#"{"Account":"111122223333","Arn":"arn:aws-us-gov:sts::111122223333:assumed-role/Dev_Admin/user"}"#.to_vec(),
            stderr: vec![],
            finished_at_unix_ms: 1,
        },
    );
    h.assert_snapshot("aws_login_steady_active");
}

#[test]
fn snapshot_steady_signed_out() {
    let mut h = harness();
    through_preflight_to_kv(&mut h);
    let cfg = SavedConfig::from_form(
        "aeos-govcloud",
        "https://start.example",
        "us-gov-west-1",
        "us-gov-west-1",
        "sso:account:access",
        "111122223333",
        "Dev_Admin",
        "Dev_Admin-111122223333",
        false,
    );
    h.dispatch(Action::ConfigLoaded(Some(serde_json::to_vec(&cfg).unwrap())));
    let probe = h.expect_effect(fx::process_program("aws"));
    h.resolve_process(
        probe,
        ProcessResult {
            status: ProcessStatus::Exited(255),
            stdout: vec![],
            stderr: b"expired".to_vec(),
            finished_at_unix_ms: 1,
        },
    );
    h.query(by::test_id("status")).assert_text("Signed out");
    h.assert_snapshot("aws_login_steady_signed_out");
}
