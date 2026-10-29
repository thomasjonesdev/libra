//! Component tests — semantics queries (§15.3).

mod common;

use aws_login::Action;
use libra_core::Effect;
use libra_test::{by, fx};

use common::{fixture_identity_result, fixture_saved_config, harness, through_preflight_to_kv};

#[test]
fn preflight_shows_aws_check_first() {
    let h = harness();
    h.query(by::test_id("aws-status"))
        .assert_text("Checking for AWS CLI v2…");
}

#[test]
fn retry_preflight_reruns_aws_version_check() {
    let mut h = harness();
    let aws = h.expect_effect(fx::process_program("aws"));
    h.resolve_process(
        aws,
        libra_core::ProcessResult {
            status: libra_core::ProcessStatus::SpawnFailed("not found".into()),
            stdout: vec![],
            stderr: vec![],
            finished_at_unix_ms: 1,
        },
    );
    h.query(by::test_id("aws-missing"));
    h.query(by::test_id("retry-preflight"));
    h.click(by::test_id("retry-preflight"));
    h.query(by::test_id("aws-status"))
        .assert_text("Checking for AWS CLI v2…");
    let retry = h.expect_effect(fx::process_program("aws"));
    let libra_core::Effect::Process { command, .. } = &retry else {
        unreachable!()
    };
    assert!(command.args.iter().any(|a| a == "--version"));
}

#[test]
fn missing_config_opens_onboarding() {
    let mut h = harness();
    through_preflight_to_kv(&mut h);
    h.query(by::test_id("session-name"));
    h.query(by::test_id("role-name"));
    h.query(by::test_id("save-setup"));
    h.expect_no_effects();
}

#[test]
fn steady_state_shows_authenticate_button() {
    let mut h = harness();
    through_preflight_to_kv(&mut h);

    let cfg = fixture_saved_config();
    h.dispatch(Action::ConfigLoaded(Some(
        serde_json::to_vec(&cfg).unwrap(),
    )));

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
    let cfg = fixture_saved_config();
    h.dispatch(Action::ConfigLoaded(Some(
        serde_json::to_vec(&cfg).unwrap(),
    )));
    let probe = h.expect_effect(fx::process_program("aws"));
    h.resolve_process(probe, fixture_identity_result());

    h.click(by::test_id("authenticate"));
    let login = h.expect_effect(fx::spawn_program("aws"));
    let Effect::Spawn {
        command, stream, ..
    } = &login
    else {
        unreachable!()
    };
    assert_eq!(stream, "login");
    assert!(command.args.iter().any(|a| a == "--use-device-code"));
    assert!(command.args.iter().any(|a| a == "--no-browser"));
    h.emit_stream(
        "login",
        libra_core::StreamEvent::Line {
            text: "https://device.sso.example.amazonaws.com/".into(),
            stderr: false,
        },
    );
    h.emit_stream(
        "login",
        libra_core::StreamEvent::Line {
            text: "Then enter the code: ABCD-EFGH".into(),
            stderr: false,
        },
    );
    h.query(by::test_id("device-code")).assert_text("ABCD-EFGH");
    h.query(by::test_id("device-url"))
        .assert_text("https://device.sso.example.amazonaws.com/");
    h.close_stream(
        "login",
        libra_core::StreamClose::Process(libra_core::ProcessStatus::Exited(0)),
    );
    let verify = h.expect_effect(fx::process_program("aws"));
    let Effect::Process { command, .. } = &verify else {
        unreachable!()
    };
    assert!(command.args.iter().any(|a| a == "get-caller-identity"));
    h.resolve_process(verify, fixture_identity_result());
    h.query(by::test_id("status")).assert_text("Active");
    h.expect_no_effects();
}

#[test]
fn a_dead_session_notifies_once_and_the_click_raises_the_window() {
    use aws_login::model::PROBE_INTERVAL_MS;
    let mut h = harness();
    through_preflight_to_kv(&mut h);
    let cfg = fixture_saved_config();
    h.dispatch(Action::ConfigLoaded(Some(
        serde_json::to_vec(&cfg).unwrap(),
    )));
    let probe = h.expect_effect(fx::process_program("aws"));
    h.resolve_process(probe, fixture_identity_result());

    h.dispatch(Action::Tick(libra_core::Instant::at(
        PROBE_INTERVAL_MS as f64 + 1.0,
    )));
    let probe = h.expect_effect(fx::process_program("aws"));
    h.resolve_process(
        probe,
        libra_core::ProcessResult {
            status: libra_core::ProcessStatus::Exited(255),
            stdout: vec![],
            stderr: b"expired".to_vec(),
            finished_at_unix_ms: 1,
        },
    );
    let note = h.expect_effect(fx::notify_tag("aws-session"));
    let libra_core::Effect::Notify { note: banner, .. } = &note else {
        unreachable!()
    };
    let text = format!("{} {}", banner.title, banner.body);
    assert!(!text.contains("111122223333"));
    assert!(!text.to_ascii_lowercase().contains("arn"));
    assert!(!text.contains("Dev_Admin"));
    h.activate_notification(note);
    let raised = h.expect_effect(fx::window_op());
    assert!(matches!(
        raised,
        libra_core::Effect::WindowOp {
            op: libra_core::WindowOp::Raise
        }
    ));
    let login = h.expect_effect(fx::process_program("aws"));
    let libra_core::Effect::Process { command, .. } = &login else {
        unreachable!()
    };
    assert!(command.args.windows(2).any(|w| w == ["sso", "login"]));
}

#[test]
fn tray_menu_raises_and_quits() {
    let mut h = harness();
    through_preflight_to_kv(&mut h);
    let cfg = fixture_saved_config();
    h.dispatch(Action::ConfigLoaded(Some(
        serde_json::to_vec(&cfg).unwrap(),
    )));
    let probe = h.expect_effect(fx::process_program("aws"));
    h.resolve_process(probe, fixture_identity_result());

    let tray = h.tray().expect("tray");
    assert!(matches!(tray.icon, libra_core::TrayIcon::Ok));
    h.click_tray("showWindow");
    let raised = h.expect_effect(fx::window_op());
    assert!(matches!(
        raised,
        libra_core::Effect::WindowOp {
            op: libra_core::WindowOp::Raise
        }
    ));
    h.click_tray("quit");
    let quit = h.expect_effect(fx::window_op());
    assert!(matches!(
        quit,
        libra_core::Effect::WindowOp {
            op: libra_core::WindowOp::Quit
        }
    ));
}

#[test]
fn launch_at_login_switch_emits_the_real_effect() {
    let mut h = harness();
    through_preflight_to_kv(&mut h);
    let cfg = fixture_saved_config();
    h.dispatch(Action::ConfigLoaded(Some(
        serde_json::to_vec(&cfg).unwrap(),
    )));
    let probe = h.expect_effect(fx::process_program("aws"));
    h.resolve_process(probe, fixture_identity_result());

    h.click(by::test_id("launch-at-login"));
    let fx = h.expect_effect(fx::login_item());
    let libra_core::Effect::LoginItem { enabled, .. } = &fx else {
        unreachable!()
    };
    assert!(*enabled);
    h.resolve_login_item(fx, libra_core::LoginItemResult::Enabled);
}
