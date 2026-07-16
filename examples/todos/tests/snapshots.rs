//! Snapshot tests (§15.5): a populated list renders byte-exactly, in
//! both themes. Missing PNGs are recorded on first run; thereafter the
//! bytes are the cross-machine determinism proof.

use libra_test::{by, Harness};
use todos::{Todos, VIEW};

fn populated() -> Harness<Todos> {
    let mut h = Harness::<Todos>::with_viewport(VIEW, 460, 520);
    for t in ["Buy milk", "Walk the dog", "Ship v1"] {
        h.type_text(by::test_id("draft"), t);
        h.click(by::test_id("add"));
    }
    // Complete the middle one so the snapshot shows both sections.
    h.click(by::text("Walk the dog"));
    h
}

#[test]
fn snapshot_mixed_list_dark() {
    let h = populated();
    h.assert_snapshot("todos_dark");
}

#[test]
fn snapshot_mixed_list_light() {
    let h = populated().theme("libra-light");
    h.assert_snapshot("todos_light");
}

#[test]
fn snapshot_empty() {
    let mut h = Harness::<Todos>::with_viewport(VIEW, 460, 520);
    h.blur();
    h.assert_snapshot("todos_empty");
}
