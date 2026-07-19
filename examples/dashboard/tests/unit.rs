//! Unit tests (§15.2): pure actions.

use dashboard::{reduce, Action, Dashboard, SERIES};
use libra_core::{Effect, SeriesId};

#[test]
fn init_opens_sixteen_ingests() {
    let (s, fx) = Dashboard::init();
    assert_eq!(fx.len(), SERIES as usize);
    assert!(matches!(fx[0], Effect::IngestOpen { series: SeriesId(0), .. }));
    assert!(s.running);
}

#[test]
fn toggle_closes_ingest_and_clears_running() {
    let (mut s, _) = Dashboard::init();
    assert!(s.running);

    let fx = reduce(&mut s, Action::Toggle);
    assert!(!s.running);
    assert_eq!(fx.len(), SERIES as usize);
    assert!(matches!(fx[0], Effect::IngestClose { series: SeriesId(0) }));

    let fx = reduce(&mut s, Action::Toggle);
    assert!(s.running);
    assert!(matches!(fx[0], Effect::IngestOpen { .. }));
}

#[test]
fn toggle_sets_paused_then_live_status() {
    let (mut s, _) = Dashboard::init();
    assert_eq!(s.status, "live · 16 series @ 10 kHz");
    reduce(&mut s, Action::Toggle);
    assert_eq!(s.status, "paused");
    reduce(&mut s, Action::Toggle);
    assert_eq!(s.status, "live · 16 series @ 10 kHz");
}
