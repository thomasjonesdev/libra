//! Component tests — semantics queries (§15.3).

mod common;

use aws_login::model::SavedConfig;
use aws_login::Action;
use libra_core::Effect;
use libra_test::{by, fx};

use common::{fixture_identity_result, harness, through_preflight_to_kv};

#[test]
fn preflight_shows_aws_check_first() {
    let h = harness();
    h.query(by::test_id("aws-status")).assert_text("Checking for AWS CLI v2…");
}

#[test]
fn missing_config_opens_onboarding() {
    let mut h = harness();
    through_preflight_to_kv(&mut h);
    h.query(by::test_id("session-name"));
    h.query(by::test_id("save-setup"));
    h.expect_no_effects();
}

#[test]
fn steady_state_shows_authenticate_button() {
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
    h.resolve_process(probe, fixture_identity_result());

    h.query(by::test_id("authenticate"));
    h.query(by::test_id("status")).assert_text("Active");
    h.has_source("interval");
}

#[test]
fn authenticate_emits_sso_login_then_identity_check() {
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
    h.resolve_process(probe, fixture_identity_result());

    h.click(by::test_id("authenticate"));
    let login = h.expect_effect(fx::process_program("aws"));
    let Effect::Process { command, .. } = &login else { unreachable!() };
    assert_eq!(command.program, "aws");
    assert!(command.args.windows(2).any(|w| w == ["sso", "login"]));
    h.resolve_process(
        login,
        common::aws_ok(&[]),
    );
    let verify = h.expect_effect(fx::process_program("aws"));
    let Effect::Process { command, .. } = &verify else { unreachable!() };
    assert!(command.args.iter().any(|a| a == "get-caller-identity"));
    h.resolve_process(verify, fixture_identity_result());
    h.query(by::test_id("status")).assert_text("Active");
    h.expect_no_effects();
}
