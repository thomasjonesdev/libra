//! Snapshot tests (§15.5): recorded tape in, rendered chart bytes out
//! — regression tests for visualizations. Names unchanged from before
//! the test reorg.

use dashboard::{Action, Dashboard, VIEW};
use libra_core::{FrameInfo, SeriesId};
use libra_test::Harness;

#[test]
fn tape_renders_byte_exact_chart() {
    let mut h = Harness::<Dashboard>::with_viewport(VIEW, 900, 620);
    let n = h.feed_tape(SeriesId(0), "tests/fixtures/wave.ltape");
    assert_eq!(n, 1500);
    h.dispatch(Action::Frame(FrameInfo::at(3000.0))); // injected time, not read
    h.assert_snapshot("dashboard_wave");
}

#[test]
fn same_tape_same_bytes_within_process() {
    let render = || {
        let mut h = Harness::<Dashboard>::with_viewport(VIEW, 900, 620);
        h.feed_tape(SeriesId(0), "tests/fixtures/wave.ltape");
        h.dispatch(Action::Frame(FrameInfo::at(3000.0)));
        h.frame_png()
    };
    assert_eq!(render(), render(), "tape → pixels must be a pure function");
}
