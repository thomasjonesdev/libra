//! Integration tests — intercepted process effects, script parity (§15.4).

mod common;

use libra_core::{Effect, ProcessStatus};
use libra_test::{by, fx};

use common::{
    aws_ok, fill_onboarding_fixture, finish_new_profile_onboarding, fixture_saved_config, harness,
    through_preflight_to_kv, FIXTURE_PROFILE, FIXTURE_SCOPES, FIXTURE_SESSION, FIXTURE_SSO_REGION,
    FIXTURE_START_URL,
};

#[test]
fn onboarding_writes_session_then_discovers_account_from_aws() {
    let mut h = harness();
    through_preflight_to_kv(&mut h);
    fill_onboarding_fixture(&mut h);

    h.click(by::test_id("save-setup"));
    let list = h.expect_effect(fx::process_program("aws"));
    h.resolve_process(list, aws_ok(b"other-profile\n"));

    finish_new_profile_onboarding(&mut h);

    let save = h.expect_effect(fx::any_kv_set());
    h.resolve_kv_set(save, libra_core::KvResult::Ok);
    let login = h.expect_effect(fx::process_program("aws"));
    let Effect::Process { command, .. } = &login else {
        unreachable!()
    };
    assert!(command.args.windows(2).any(|w| w == ["sso", "login"]));
    assert!(command
        .args
        .windows(2)
        .any(|w| w == ["--profile", FIXTURE_PROFILE]));
}

#[test]
fn onboarding_adopts_an_existing_profile_without_configure_set() {
    let mut h = harness();
    through_preflight_to_kv(&mut h);
    fill_onboarding_fixture(&mut h);

    h.click(by::test_id("save-setup"));
    let list = h.expect_effect(fx::process_program("aws"));
    h.resolve_process(
        list,
        aws_ok(format!("other-profile\n{FIXTURE_PROFILE}\n").as_bytes()),
    );

    use aws_login::model::config_session_write_pairs;
    for (_key, _value) in config_session_write_pairs(
        FIXTURE_SESSION,
        FIXTURE_START_URL,
        FIXTURE_SSO_REGION,
        FIXTURE_SCOPES,
    ) {
        let fx = h.expect_effect(fx::process_program("aws"));
        h.resolve_process(fx, aws_ok(&[]));
    }
    let session_login = h.expect_effect(fx::process_program("aws"));
    h.resolve_process(session_login, aws_ok(&[]));
    // Existing profile — account is derived from the profile name, no token read.
    let save = h.expect_effect(fx::any_kv_set());
    h.resolve_kv_set(save, libra_core::KvResult::Ok);
    let login = h.expect_effect(fx::process_program("aws"));
    let Effect::Process { command, .. } = &login else {
        unreachable!()
    };
    assert!(command.args.windows(2).any(|w| w == ["sso", "login"]));
    assert!(command
        .args
        .windows(2)
        .any(|w| w == ["--profile", FIXTURE_PROFILE]));
}

#[test]
fn login_failure_surfaces_stderr() {
    let mut h = harness();
    through_preflight_to_kv(&mut h);
    h.dispatch(aws_login::Action::ConfigLoaded(Some(
        serde_json::to_vec(&fixture_saved_config()).unwrap(),
    )));
    let probe = h.expect_effect(fx::process_program("aws"));
    h.resolve_process(
        probe,
        aws_ok(br#"{"Account":"111122223333","Arn":"arn:aws:sts::111122223333:assumed-role/x"}"#),
    );

    h.click(by::test_id("authenticate"));
    let login = h.expect_effect(fx::spawn_program("aws"));
    let Effect::Spawn { command, .. } = &login else {
        unreachable!()
    };
    assert!(command.args.iter().any(|a| a == "--use-device-code"));
    h.close_stream(
        "login",
        libra_core::StreamClose::Process(ProcessStatus::Exited(1)),
    );
    h.query(by::test_id("error-detail"))
        .assert_text("Sign-in did not finish");
}

#[test]
fn copy_export_line_uses_clipboard_effect() {
    let mut h = harness();
    through_preflight_to_kv(&mut h);
    h.dispatch(aws_login::Action::ConfigLoaded(Some(
        serde_json::to_vec(&fixture_saved_config()).unwrap(),
    )));
    let probe = h.expect_effect(fx::process_program("aws"));
    h.resolve_process(
        probe,
        aws_ok(br#"{"Account":"111122223333","Arn":"arn:aws:sts::111122223333:assumed-role/x"}"#),
    );

    h.click(by::test_id("copy-export"));
    let clip = h.expect_effect(fx::clipboard_set());
    let Effect::ClipboardSet { text, .. } = &clip else {
        unreachable!()
    };
    assert_eq!(text, "export AWS_PROFILE=Dev_Admin-111122223333");
}
