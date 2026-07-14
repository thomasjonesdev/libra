//! Unit tests (§15.2): pure actions, no harness, no rendering.

use fetcher::{reduce, Action, Fetcher, COUNT_KEY, ZEN_URL};
use libra_core::{Effect, HttpResult, Remote};

#[test]
fn increment_returns_persist_effect() {
    let (mut s, init_fx) = Fetcher::init();
    assert_eq!(init_fx, vec![Effect::kv_get(COUNT_KEY, Action::CountLoaded)]);
    let fx = reduce(&mut s, Action::Increment);
    assert_eq!(s.count, 1);
    assert_eq!(fx, vec![Effect::kv_set_acked(COUNT_KEY, 1i64.to_le_bytes().to_vec(), Action::Saved)]);
}

#[test]
fn fetch_goes_loading_and_requests_zen() {
    let (mut s, _) = Fetcher::init();
    let fx = reduce(&mut s, Action::FetchQuote);
    assert!(s.quote_state.is_loading());
    assert_eq!(s.quote, "fetching…"); // the §6.3 projection tracks the state
    let Effect::Http { req, .. } = &fx[0] else { panic!("expected Http, got {fx:?}") };
    assert_eq!(req.url, ZEN_URL);
}

#[test]
fn every_http_branch_is_one_more_fixture() {
    let (mut s, _) = Fetcher::init();
    reduce(&mut s, Action::QuoteArrived(HttpResult::Response { status: 500, body: vec![] }));
    assert_eq!(s.quote_state, Remote::Failure("HTTP 500".into()));
    reduce(&mut s, Action::QuoteArrived(HttpResult::Failed("dns: no such host".into())));
    assert_eq!(s.quote_state, Remote::Failure("dns: no such host".into()));
    reduce(&mut s, Action::QuoteArrived(HttpResult::Response { status: 200, body: b"Zen.".to_vec() }));
    assert_eq!(s.quote_state, Remote::Success("Zen.".into()));
    assert_eq!(s.quote, "Zen.");
}

#[test]
fn corrupt_persisted_bytes_are_ignored() {
    let (mut s, _) = Fetcher::init();
    reduce(&mut s, Action::CountLoaded(Some(vec![1, 2, 3]))); // not 8 bytes
    assert_eq!(s.count, 0);
}
