//! Unit tests (§15.2): pure actions and pure subscriptions.

use dashboard::{reduce, subscriptions, Action, Dashboard, SERIES};
use libra_core::{Effect, FrameInfo, SeriesId, Sub};

#[test]
fn init_opens_sixteen_ingests() {
    let (s, fx) = Dashboard::init();
    assert_eq!(fx.len(), SERIES as usize);
    assert!(matches!(fx[0], Effect::IngestOpen { series: SeriesId(0), .. }));
    assert!(s.running);
}

#[test]
fn toggle_closes_ingest_and_drops_the_frames_sub() {
    let (mut s, _) = Dashboard::init();
    assert_eq!(subscriptions(&s).len(), 1, "running → Sub::Frames active");

    let fx = reduce(&mut s, Action::Toggle);
    assert!(!s.running);
    assert_eq!(fx.len(), SERIES as usize);
    assert!(matches!(fx[0], Effect::IngestClose { series: SeriesId(0) }));
    assert!(subscriptions(&s).is_empty(), "paused → no subscriptions");

    let fx = reduce(&mut s, Action::Toggle);
    assert!(matches!(fx[0], Effect::IngestOpen { .. }));
    assert!(matches!(subscriptions(&s)[0], Sub::Frames { .. }));
}

#[test]
fn frame_time_arrives_inside_the_action() {
    let (mut s, _) = Dashboard::init();
    reduce(&mut s, Action::Frame(FrameInfo { t_ms: 2500.0, index: 150 }));
    assert_eq!(s.status, "live · t=2.5s · frame 150");
}
