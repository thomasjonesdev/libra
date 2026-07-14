//! Dashboard tier suite — including the §15.5 flagship: recorded tape
//! in, rendered chart bytes out. Regression tests for visualizations.

use dashboard::{Dashboard, Msg, SERIES, VIEW};
use libra_core::{App, Cmd, FrameInfo, SeriesId, Sub};
use libra_test::{by, Harness};

// ---------------------------------------------------------------------
// Tier 1 — pure update + pure subscriptions
// ---------------------------------------------------------------------

#[test]
fn tier1_init_opens_sixteen_ingests() {
    let (m, cmds) = Dashboard::init();
    assert_eq!(cmds.len(), SERIES as usize);
    assert!(matches!(cmds[0], Cmd::IngestOpen { series: SeriesId(0), .. }));
    assert!(m.running);
}

#[test]
fn tier1_toggle_closes_ingest_and_drops_the_frames_sub() {
    let (mut m, _) = Dashboard::init();
    assert_eq!(m.subscriptions().len(), 1, "running → Sub::Frames active");

    let cmds = m.update(Msg::Toggle);
    assert!(!m.running);
    assert_eq!(cmds.len(), SERIES as usize);
    assert!(matches!(cmds[0], Cmd::IngestClose { series: SeriesId(0) }));
    assert!(m.subscriptions().is_empty(), "paused → no subscriptions");

    let cmds = m.update(Msg::Toggle);
    assert!(matches!(cmds[0], Cmd::IngestOpen { .. }));
    assert!(matches!(m.subscriptions()[0], Sub::Frames { .. }));
}

#[test]
fn tier1_frame_time_arrives_inside_the_msg() {
    let (mut m, _) = Dashboard::init();
    m.update(Msg::Frame(FrameInfo { t_ms: 2500.0, index: 150 }));
    assert_eq!(m.status, "live · t=2.5s · frame 150");
}

// ---------------------------------------------------------------------
// Tier 2 — headless view queries
// ---------------------------------------------------------------------

#[test]
fn tier2_renders_the_chart_wall() {
    let h = Harness::<Dashboard>::with_viewport(VIEW, 900, 620);
    h.query(by::test_id("status")).assert_text("starting…");
    h.query(by::test_id("toggle")).assert_text("Pause");
    h.query(by::test_id("c0")); // chart c0 exists in the semantics tree
}

#[test]
fn tier2_toggle_click_pauses_through_real_hit_testing() {
    let mut h = Harness::<Dashboard>::with_viewport(VIEW, 900, 620);
    h.click(by::test_id("toggle"));
    h.query(by::test_id("status")).assert_text("paused");
    assert!(!h.model().running);
}

// ---------------------------------------------------------------------
// Tier 4 — tape → byte-exact chart pixels (§15.5)
// ---------------------------------------------------------------------

#[test]
fn tier4_tape_renders_byte_exact_chart() {
    let mut h = Harness::<Dashboard>::with_viewport(VIEW, 900, 620);
    let n = h.feed_tape(SeriesId(0), "tests/fixtures/wave.ltape");
    assert_eq!(n, 1500);
    h.dispatch(Msg::Frame(FrameInfo::at(3000.0))); // injected time, not read
    h.assert_snapshot("dashboard_wave");
}

#[test]
fn tier4_same_tape_same_bytes_within_process() {
    let render = || {
        let mut h = Harness::<Dashboard>::with_viewport(VIEW, 900, 620);
        h.feed_tape(SeriesId(0), "tests/fixtures/wave.ltape");
        h.dispatch(Msg::Frame(FrameInfo::at(3000.0)));
        h.frame_png()
    };
    assert_eq!(render(), render(), "tape → pixels must be a pure function");
}
