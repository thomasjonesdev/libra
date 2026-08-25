//! Snapshot tests: empty shell and a populated vault.

use crate::common::{harness, open_first_note};

mod common;

#[test]
fn snapshot_empty() {
    let h = harness();
    h.assert_snapshot("gloss_empty");
}

#[test]
fn snapshot_note_open() {
    let mut h = harness();
    open_first_note(
        &mut h,
        "Atlas.md\n",
        "---\ntitle: Atlas\n---\n\nThe atlas.\n",
    );
    h.assert_snapshot("gloss_note_open");
}
