//! Component tests (§15.3): headless view queries.

use fetcher::{Fetcher, VIEW};
use libra_test::{by, Harness};

fn harness() -> Harness<Fetcher> {
    Harness::with_viewport(VIEW, 460, 260)
}

#[test]
fn initial_render() {
    let h = harness();
    h.query(by::test_id("count")).assert_text("0");
    h.query(by::test_id("quote")).assert_text("press fetch");
    h.query(by::test_id("status")).assert_text("not saved yet");
}

#[test]
fn click_fetch_shows_loading_through_real_hit_testing() {
    let mut h = harness();
    h.click(by::test_id("fetch"));
    h.query(by::test_id("quote")).assert_text("fetching…");
}
