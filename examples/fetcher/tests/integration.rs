//! Integration tests (§15.4): full fetch→parse→persist→render flows,
//! synchronous, mock-free — the effects runtime's whole point.

use fetcher::{Fetcher, COUNT_KEY, VIEW};
use libra_core::{Effect, HttpResult, KvResult};
use libra_test::{by, fx, Harness};

fn harness() -> Harness<Fetcher> {
    Harness::with_viewport(VIEW, 460, 260)
}

#[test]
fn startup_restores_persisted_count() {
    let mut h = harness();
    let load = h.expect_effect(fx::kv_get(COUNT_KEY));
    h.resolve_kv_get(load, Some(5i64.to_le_bytes().to_vec()));
    h.query(by::test_id("count")).assert_text("5");
    h.query(by::test_id("status")).assert_text("restored");
    h.expect_no_effects();
}

#[test]
fn first_run_keeps_defaults() {
    let mut h = harness();
    let load = h.expect_effect(fx::any_kv_get());
    h.resolve_kv_get(load, None);
    h.query(by::test_id("count")).assert_text("0");
    h.expect_no_effects();
}

#[test]
fn increment_persists_and_acks() {
    let mut h = harness();
    let load = h.expect_effect(fx::any_kv_get());
    h.resolve_kv_get(load, None);

    h.click(by::test_id("inc"));
    h.query(by::test_id("count")).assert_text("1");
    let save = h.expect_effect(fx::kv_set(COUNT_KEY));
    // The effect carries the actual bytes to be persisted — assert on them.
    let Effect::KvSet { value, .. } = &save else { unreachable!() };
    assert_eq!(value, &1i64.to_le_bytes().to_vec());
    h.resolve_kv_set(save, KvResult::Ok);
    h.query(by::test_id("status")).assert_text("saved");
    h.expect_no_effects();
}

#[test]
fn full_fetch_flow_renders_the_quote() {
    let mut h = harness();
    let load = h.expect_effect(fx::any_kv_get());
    h.resolve_kv_get(load, None);

    h.click(by::test_id("fetch"));
    h.query(by::test_id("quote")).assert_text("fetching…");
    let req = h.expect_effect(fx::http());
    h.resolve_http(req, HttpResult::Response { status: 200, body: b"Mind the gap.".to_vec() });
    h.query(by::test_id("quote")).assert_text("Mind the gap.");
    h.expect_no_effects();
}

#[test]
fn save_failure_is_visible_state() {
    let mut h = harness();
    let load = h.expect_effect(fx::any_kv_get());
    h.resolve_kv_get(load, None);
    h.click(by::test_id("inc"));
    let save = h.expect_effect(fx::any_kv_set());
    h.resolve_kv_set(save, KvResult::Err("disk full".into()));
    h.query(by::test_id("status")).assert_text("save failed: disk full");
}

/// §12.3: cross-effect ordering is NOT guaranteed. Kick off a save and a
/// fetch, then resolve them in the "wrong" order — state must land in
/// the same place.
#[test]
fn adversarial_result_ordering_is_tolerated() {
    let mut h = harness();
    let load = h.expect_effect(fx::any_kv_get());
    h.resolve_kv_get(load, None);

    h.click(by::test_id("inc"));
    h.click(by::test_id("fetch"));
    let save = h.expect_effect(fx::any_kv_set());
    let req = h.expect_effect(fx::http());

    // HTTP resolves before the earlier-issued save acks:
    h.resolve_http(req, HttpResult::Response { status: 200, body: b"Zen.".to_vec() });
    h.resolve_kv_set(save, KvResult::Ok);

    h.query(by::test_id("count")).assert_text("1");
    h.query(by::test_id("quote")).assert_text("Zen.");
    h.query(by::test_id("status")).assert_text("saved");
    h.expect_no_effects();
}
