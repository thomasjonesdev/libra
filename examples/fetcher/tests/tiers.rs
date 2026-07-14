//! The fetcher's tier suite (ARCHITECTURE.md §15) — Phase 1's point:
//! full fetch→parse→persist→render flows, synchronous, mock-free.

use fetcher::{Fetcher, Msg, COUNT_KEY, VIEW, ZEN_URL};
use libra_core::{App, Cmd, HttpResult, KvResult, Remote};
use libra_test::{by, cmd, Harness};

fn harness() -> Harness<Fetcher> {
    Harness::with_viewport(VIEW, 460, 260)
}

// ---------------------------------------------------------------------
// Tier 1 — pure update (§15.2)
// ---------------------------------------------------------------------

#[test]
fn tier1_increment_returns_persist_cmd() {
    let (mut m, init_cmds) = Fetcher::init();
    assert_eq!(init_cmds, vec![Cmd::kv_get(COUNT_KEY, Msg::CountLoaded)]);
    let cmds = m.update(Msg::Increment);
    assert_eq!(m.count, 1);
    assert_eq!(cmds, vec![Cmd::kv_set_acked(COUNT_KEY, 1i64.to_le_bytes().to_vec(), Msg::Saved)]);
}

#[test]
fn tier1_fetch_goes_loading_and_requests_zen() {
    let (mut m, _) = Fetcher::init();
    let cmds = m.update(Msg::FetchQuote);
    assert!(m.quote_state.is_loading());
    assert_eq!(m.quote, "fetching…"); // the §6.3 projection tracks the state
    let Cmd::Http { req, .. } = &cmds[0] else { panic!("expected Http, got {cmds:?}") };
    assert_eq!(req.url, ZEN_URL);
}

#[test]
fn tier1_every_http_branch_is_one_more_fixture() {
    let (mut m, _) = Fetcher::init();
    m.update(Msg::QuoteArrived(HttpResult::Response { status: 500, body: vec![] }));
    assert_eq!(m.quote_state, Remote::Failure("HTTP 500".into()));
    m.update(Msg::QuoteArrived(HttpResult::Failed("dns: no such host".into())));
    assert_eq!(m.quote_state, Remote::Failure("dns: no such host".into()));
    m.update(Msg::QuoteArrived(HttpResult::Response { status: 200, body: b"Zen.".to_vec() }));
    assert_eq!(m.quote_state, Remote::Success("Zen.".into()));
    assert_eq!(m.quote, "Zen.");
}

#[test]
fn tier1_corrupt_persisted_bytes_are_ignored() {
    let (mut m, _) = Fetcher::init();
    m.update(Msg::CountLoaded(Some(vec![1, 2, 3]))); // not 8 bytes
    assert_eq!(m.count, 0);
}

// ---------------------------------------------------------------------
// Tier 2 — headless view queries (§15.3)
// ---------------------------------------------------------------------

#[test]
fn tier2_initial_render() {
    let h = harness();
    h.query(by::test_id("count")).assert_text("0");
    h.query(by::test_id("quote")).assert_text("press fetch");
    h.query(by::test_id("status")).assert_text("not saved yet");
}

#[test]
fn tier2_click_fetch_shows_loading_through_real_hit_testing() {
    let mut h = harness();
    h.click(by::test_id("fetch"));
    h.query(by::test_id("quote")).assert_text("fetching…");
}

// ---------------------------------------------------------------------
// Tier 3 — effect-script tests: full flows, no mocks (§15.4)
// ---------------------------------------------------------------------

#[test]
fn tier3_startup_restores_persisted_count() {
    let mut h = harness();
    let load = h.expect_cmd(cmd::kv_get(COUNT_KEY));
    h.resolve_kv_get(load, Some(5i64.to_le_bytes().to_vec()));
    h.query(by::test_id("count")).assert_text("5");
    h.query(by::test_id("status")).assert_text("restored");
    h.expect_no_cmds();
}

#[test]
fn tier3_first_run_keeps_defaults() {
    let mut h = harness();
    let load = h.expect_cmd(cmd::any_kv_get());
    h.resolve_kv_get(load, None);
    h.query(by::test_id("count")).assert_text("0");
    h.expect_no_cmds();
}

#[test]
fn tier3_increment_persists_and_acks() {
    let mut h = harness();
    let load = h.expect_cmd(cmd::any_kv_get());
    h.resolve_kv_get(load, None);

    h.click(by::test_id("inc"));
    h.query(by::test_id("count")).assert_text("1");
    let save = h.expect_cmd(cmd::kv_set(COUNT_KEY));
    // The cmd carries the actual bytes to be persisted — assert on them.
    let Cmd::KvSet { value, .. } = &save else { unreachable!() };
    assert_eq!(value, &1i64.to_le_bytes().to_vec());
    h.resolve_kv_set(save, KvResult::Ok);
    h.query(by::test_id("status")).assert_text("saved");
    h.expect_no_cmds();
}

#[test]
fn tier3_full_fetch_flow_renders_the_quote() {
    let mut h = harness();
    let load = h.expect_cmd(cmd::any_kv_get());
    h.resolve_kv_get(load, None);

    h.click(by::test_id("fetch"));
    h.query(by::test_id("quote")).assert_text("fetching…");
    let req = h.expect_cmd(cmd::http());
    h.resolve_http(req, HttpResult::Response { status: 200, body: b"Mind the gap.".to_vec() });
    h.query(by::test_id("quote")).assert_text("Mind the gap.");
    h.expect_no_cmds();
}

#[test]
fn tier3_save_failure_is_visible_state() {
    let mut h = harness();
    let load = h.expect_cmd(cmd::any_kv_get());
    h.resolve_kv_get(load, None);
    h.click(by::test_id("inc"));
    let save = h.expect_cmd(cmd::any_kv_set());
    h.resolve_kv_set(save, KvResult::Err("disk full".into()));
    h.query(by::test_id("status")).assert_text("save failed: disk full");
}

/// §12.3: cross-effect ordering is NOT guaranteed. Kick off a save and a
/// fetch, then resolve them in the "wrong" order — the model must land
/// in the same place.
#[test]
fn tier3_adversarial_result_ordering_is_tolerated() {
    let mut h = harness();
    let load = h.expect_cmd(cmd::any_kv_get());
    h.resolve_kv_get(load, None);

    h.click(by::test_id("inc"));
    h.click(by::test_id("fetch"));
    let save = h.expect_cmd(cmd::any_kv_set());
    let req = h.expect_cmd(cmd::http());

    // HTTP resolves before the earlier-issued save acks:
    h.resolve_http(req, HttpResult::Response { status: 200, body: b"Zen.".to_vec() });
    h.resolve_kv_set(save, KvResult::Ok);

    h.query(by::test_id("count")).assert_text("1");
    h.query(by::test_id("quote")).assert_text("Zen.");
    h.query(by::test_id("status")).assert_text("saved");
    h.expect_no_cmds();
}

// ---------------------------------------------------------------------
// Tier 4 — byte-exact snapshots (§15.5)
// ---------------------------------------------------------------------

#[test]
fn tier4_snapshot_initial() {
    let h = harness();
    h.assert_snapshot("fetcher_initial");
}

#[test]
fn tier4_snapshot_fetched_state() {
    let mut h = harness();
    let load = h.expect_cmd(cmd::any_kv_get());
    h.resolve_kv_get(load, Some(3i64.to_le_bytes().to_vec()));
    h.click(by::test_id("fetch"));
    let req = h.expect_cmd(cmd::http());
    h.resolve_http(
        req,
        HttpResult::Response { status: 200, body: b"Keep it logically awesome.".to_vec() },
    );
    h.assert_snapshot("fetcher_zen");
}
