//! Integration tests — intercepted process effects, script parity (§15.4).

mod common;

use aws_login::model::{
    config_write_pairs, SavedConfig, DEFAULT_ACCOUNT_ID, DEFAULT_AWS_REGION, DEFAULT_ROLE,
    DEFAULT_SCOPES, DEFAULT_SESSION, DEFAULT_SSO_REGION, DEFAULT_START_URL,
};
use aws_login::Action;
use libra_core::{Effect, ProcessResult, ProcessStatus};
use libra_test::{by, fx};

use common::{aws_ok, harness, through_preflight_to_kv};

#[test]
fn onboarding_writes_the_same_configure_keys_as_the_bash_script() {
    let mut h = harness();
    through_preflight_to_kv(&mut h);

    h.click(by::test_id("save-setup"));
    let list = h.expect_effect(fx::process_program("aws"));
    h.resolve_process(list, aws_ok(b"other-profile\n"));

    let cfg = SavedConfig::from_form(
        DEFAULT_SESSION,
        DEFAULT_START_URL,
        DEFAULT_SSO_REGION,
        DEFAULT_AWS_REGION,
        DEFAULT_SCOPES,
        DEFAULT_ACCOUNT_ID,
        DEFAULT_ROLE,
        &format!("{DEFAULT_ROLE}-{DEFAULT_ACCOUNT_ID}"),
        true,
    );
    for (key, value) in config_write_pairs(&cfg) {
        let fx = h.expect_effect(fx::process_program("aws"));
        let Effect::Process { command, .. } = &fx else { unreachable!() };
        assert_eq!(command.args, vec!["configure", "set", key.as_str(), value.as_str()]);
        h.resolve_process(fx, aws_ok(&[]));
    }
    let save = h.expect_effect(fx::any_kv_set());
    h.resolve_kv_set(save, libra_core::KvResult::Ok);
    let login = h.expect_effect(fx::process_program("aws"));
    let Effect::Process { command, .. } = &login else { unreachable!() };
    assert!(command.args.windows(2).any(|w| w == ["sso", "login"]));
}

#[test]
fn login_failure_surfaces_stderr() {
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
        aws_ok(br#"{"Account":"111122223333","Arn":"arn:aws:sts::111122223333:assumed-role/x"}"#),
    );

    h.click(by::test_id("authenticate"));
    let login = h.expect_effect(fx::process_program("aws"));
    h.resolve_process(
        login,
        ProcessResult {
            status: ProcessStatus::Exited(1),
            stdout: vec![],
            stderr: b"Token has expired".to_vec(),
            finished_at_unix_ms: 1,
        },
    );
    h.query(by::test_id("error-detail"))
        .assert_text("aws sso login failed: Token has expired");
}

#[test]
fn copy_export_line_uses_clipboard_effect() {
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
        aws_ok(br#"{"Account":"111122223333","Arn":"arn:aws:sts::111122223333:assumed-role/x"}"#),
    );

    h.click(by::test_id("show-config"));
    while let Ok(fx) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        h.expect_effect(fx::process_program("aws"))
    })) {
        h.resolve_process(fx, aws_ok(b"us-gov-west-1"));
    }
    h.click(by::test_id("copy-export"));
    let clip = h.expect_effect(fx::clipboard_set());
    let Effect::ClipboardSet { text, .. } = &clip else { unreachable!() };
    assert_eq!(text, "export AWS_PROFILE=Dev_Admin-111122223333");
}
