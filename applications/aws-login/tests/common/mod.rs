//! Shared harness helpers for aws-login tests.

use aws_login::AwsLogin;
use libra_core::{ProcessResult, ProcessStatus};
use libra_test::{fx, Harness};

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

pub fn through_preflight_to_kv(h: &mut Harness<AwsLogin>) {
    let aws = h.expect_effect(fx::process_program("aws"));
    h.resolve_process(aws, aws_ok(b"aws-cli/2.15.0"));
    let claude = h.expect_effect(fx::process_program("claude"));
    h.resolve_process(claude, aws_ok(b"claude 1.0.0"));
    let load = h.expect_effect(fx::any_kv_get());
    h.resolve_kv_get(load, None);
}
