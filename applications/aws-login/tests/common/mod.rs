//! Shared harness helpers for aws-login tests.

use aws_login::Action;
use aws_login::AwsLogin;
use aws_login::model::SavedConfig;
use libra_core::{ProcessResult, ProcessStatus};
use libra_test::{fx, Harness};

pub const FIXTURE_SESSION: &str = "aeos-govcloud";
pub const FIXTURE_START_URL: &str = "https://start.example.awsapps.com/directory/d-test";
pub const FIXTURE_SSO_REGION: &str = "us-gov-west-1";
pub const FIXTURE_AWS_REGION: &str = "us-gov-west-1";
pub const FIXTURE_SCOPES: &str = "sso:account:access";
pub const FIXTURE_ACCOUNT_ID: &str = "111122223333";
pub const FIXTURE_ROLE: &str = "Dev_Admin";
pub const FIXTURE_PROFILE: &str = "Dev_Admin-111122223333";

pub fn harness() -> Harness<AwsLogin> {
    Harness::with_viewport(aws_login::VIEW, 420, 640)
}

pub fn aws_ok(stdout: &[u8]) -> ProcessResult {
    ProcessResult {
        status: ProcessStatus::Exited(0),
        stdout: stdout.to_vec(),
        stderr: vec![],
        finished_at_unix_ms: 1,
    }
}

pub fn fixture_identity_result() -> ProcessResult {
    ProcessResult {
        status: ProcessStatus::Exited(0),
        stdout: br#"{"Account":"111122223333","Arn":"arn:aws-us-gov:sts::111122223333:assumed-role/Dev_Admin/user"}"#.to_vec(),
        stderr: vec![],
        finished_at_unix_ms: 1,
    }
}

pub fn fixture_sso_accounts_result() -> ProcessResult {
    ProcessResult {
        status: ProcessStatus::Exited(0),
        stdout: br#"{"accountList":[{"accountId":"111122223333","accountName":"test"}]}"#.to_vec(),
        stderr: vec![],
        finished_at_unix_ms: 1,
    }
}

pub fn fixture_sso_token_result() -> ProcessResult {
    ProcessResult {
        status: ProcessStatus::Exited(0),
        stdout: br#"{"accessToken":"test-access-token","expiresAt":"2099-01-01T00:00:00Z"}"#.to_vec(),
        stderr: vec![],
        finished_at_unix_ms: 1,
    }
}

pub fn fixture_saved_config() -> SavedConfig {
    SavedConfig::from_form(
        FIXTURE_SESSION,
        FIXTURE_START_URL,
        FIXTURE_SSO_REGION,
        FIXTURE_AWS_REGION,
        FIXTURE_SCOPES,
        FIXTURE_ACCOUNT_ID,
        FIXTURE_ROLE,
        FIXTURE_PROFILE,
        true,
    )
}

pub fn fill_onboarding_fixture(h: &mut Harness<AwsLogin>) {
    h.dispatch(Action::StartUrlChanged(FIXTURE_START_URL.into()));
    h.dispatch(Action::SsoRegionChanged(FIXTURE_SSO_REGION.into()));
    h.dispatch(Action::AwsRegionChanged(FIXTURE_AWS_REGION.into()));
    h.dispatch(Action::RegistrationScopesChanged(FIXTURE_SCOPES.into()));
    h.dispatch(Action::RoleNameChanged(FIXTURE_ROLE.into()));
}

pub fn through_preflight_to_kv(h: &mut Harness<AwsLogin>) {
    let aws = h.expect_effect(fx::process_program("aws"));
    h.resolve_process(aws, aws_ok(b"aws-cli/2.15.0"));
    let claude = h.expect_effect(fx::process_program("claude"));
    h.resolve_process(claude, aws_ok(b"claude 1.0.0"));
    let load = h.expect_effect(fx::any_kv_get());
    h.resolve_kv_get(load, None);
}

/// Complete the new-profile onboarding argv chain after `save-setup`.
pub fn finish_new_profile_onboarding(h: &mut Harness<AwsLogin>) {
    use aws_login::model::{config_profile_write_pairs, config_session_write_pairs};
    use libra_core::Effect;

    let cfg = fixture_saved_config();
    for (key, value) in config_session_write_pairs(
        FIXTURE_SESSION,
        FIXTURE_START_URL,
        FIXTURE_SSO_REGION,
        FIXTURE_SCOPES,
    ) {
        let fx = h.expect_effect(fx::process_program("aws"));
        let Effect::Process { command, .. } = &fx else { unreachable!() };
        assert_eq!(
            command.args,
            vec![
                "configure",
                "set",
                "--sso-session",
                FIXTURE_SESSION,
                key.as_str(),
                value.as_str(),
            ]
        );
        h.resolve_process(fx, aws_ok(&[]));
    }
    let session_login = h.expect_effect(fx::process_program("aws"));
    let Effect::Process { command, .. } = &session_login else { unreachable!() };
    assert!(command.args.windows(2).any(|w| w == ["sso", "login"]));
    assert!(command
        .args
        .windows(2)
        .any(|w| w == ["--sso-session", FIXTURE_SESSION]));
    h.resolve_process(session_login, aws_ok(&[]));
    let read_token = h.expect_effect(fx::process_program("cat"));
    h.resolve_process(read_token, fixture_sso_token_result());
    let list = h.expect_effect(fx::process_program("aws"));
    let Effect::Process { command, .. } = &list else { unreachable!() };
    assert!(command.args.windows(2).any(|w| w == ["sso", "list-accounts"]));
    assert!(command
        .args
        .windows(2)
        .any(|w| w == ["--access-token", "test-access-token"]));
    h.resolve_process(list, fixture_sso_accounts_result());
    for (key, value) in config_profile_write_pairs(&cfg) {
        let fx = h.expect_effect(fx::process_program("aws"));
        let Effect::Process { command, .. } = &fx else { unreachable!() };
        assert_eq!(command.args, vec!["configure", "set", key.as_str(), value.as_str()]);
        h.resolve_process(fx, aws_ok(&[]));
    }
}
