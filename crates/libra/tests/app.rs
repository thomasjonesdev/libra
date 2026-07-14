//! End-to-end proof of the ergonomics layer (PLAN-ERGONOMICS.md E1):
//! `#[libra::state]` + `#[libra::actions]` + `libra::app!` wired
//! together, then driven through the real harness — exactly the
//! §5.8 "counter in ~10 lines" claim, exercised for real.

use libra::{actions, app, state};

#[state]
#[derive(Default)]
struct Counter {
    count: i64,
}

#[actions]
impl Counter {
    fn increment(&mut self) {
        self.count += 1;
    }
    fn decrement(&mut self) {
        self.count -= 1;
    }
    fn reset(&mut self) {
        self.count = 0;
    }
}

app! {
    state: Counter,
    view: "fixtures/counter.lx",
    window: (300.0, 200.0),
    title: "Test Counter",
}

#[test]
fn init_falls_back_to_default_with_no_effects() {
    let (c, effects) = <Counter as libra_core::App>::init();
    assert_eq!(c.count, 0);
    assert!(effects.is_empty());
}

#[test]
fn run_has_the_expected_signature_without_opening_a_window() {
    let _f: fn() -> Result<(), Box<dyn std::error::Error>> = run;
}

#[test]
fn the_generated_app_drives_the_real_view_through_the_real_harness() {
    let mut h = libra_test::Harness::<Counter>::render(VIEW);
    h.query(libra_test::by::test_id("count")).assert_text("0");

    h.click(libra_test::by::test_id("inc"));
    h.query(libra_test::by::test_id("count")).assert_text("1");

    h.click(libra_test::by::test_id("inc"));
    h.click(libra_test::by::test_id("inc"));
    h.query(libra_test::by::test_id("count")).assert_text("3");

    h.click(libra_test::by::test_id("dec"));
    h.query(libra_test::by::test_id("count")).assert_text("2");

    h.click(libra_test::by::test_id("reset"));
    h.query(libra_test::by::test_id("count")).assert_text("0");
}
