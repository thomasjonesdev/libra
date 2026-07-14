//! Snapshot tests (§15.5): byte-exact rendering. Names unchanged from
//! before the test reorg — PNG identity must not depend on file layout.

use fetcher::{Fetcher, VIEW};
use libra_core::HttpResult;
use libra_test::{by, fx, Harness};

fn harness() -> Harness<Fetcher> {
    Harness::with_viewport(VIEW, 460, 260)
}

#[test]
fn snapshot_initial() {
    let h = harness();
    h.assert_snapshot("fetcher_initial");
}

#[test]
fn snapshot_fetched_state() {
    let mut h = harness();
    let load = h.expect_effect(fx::any_kv_get());
    h.resolve_kv_get(load, Some(3i64.to_le_bytes().to_vec()));
    h.click(by::test_id("fetch"));
    let req = h.expect_effect(fx::http());
    h.resolve_http(
        req,
        HttpResult::Response { status: 200, body: b"Keep it logically awesome.".to_vec() },
    );
    h.assert_snapshot("fetcher_zen");
}
