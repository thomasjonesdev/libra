//! Snapshot tests: empty shell and a populated vault.

use crate::common::{assert_no_overflow, harness, harness_at, open_first_note};
use libra_test::by;

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

#[test]
fn snapshot_empty_720() {
    let h = harness_at(720, 640);
    assert_no_overflow(&h);
    h.assert_snapshot("gloss_empty_720");
}

#[test]
fn snapshot_empty_1280() {
    let h = harness_at(1280, 640);
    assert_no_overflow(&h);
    h.assert_snapshot("gloss_empty_1280");
}

#[test]
fn snapshot_note_open_720() {
    let mut h = harness_at(720, 640);
    open_first_note(
        &mut h,
        "Atlas.md\n10-Inbox/Welcome.md\n",
        "---\ntitle: Atlas\n---\n\nThe atlas.\n",
    );
    assert_no_overflow(&h);
    h.assert_snapshot("gloss_note_open_720");
}

#[test]
fn snapshot_note_open_1280() {
    let mut h = harness_at(1280, 640);
    open_first_note(
        &mut h,
        "Atlas.md\n10-Inbox/Welcome.md\n",
        "---\ntitle: Atlas\n---\n\nThe atlas.\n",
    );
    assert_no_overflow(&h);
    h.assert_snapshot("gloss_note_open_1280");
}

#[test]
fn snapshot_settings_drawer() {
    let mut h = harness();
    open_first_note(&mut h, "Atlas.md\n", "---\ntitle: Atlas\n---\n\nThe atlas.\n");
    h.dispatch(gloss::Action::OpenSettings);
    assert_no_overflow(&h);
    h.assert_snapshot("gloss_settings_drawer");
}

#[test]
fn chrome_fits_at_720_and_1280() {
    let mut h = harness_at(720, 640);
    open_first_note(
        &mut h,
        "Atlas.md\n10-Inbox/Welcome.md\n",
        "---\ntitle: Atlas\n---\n\nThe atlas.\n",
    );
    h.query(by::test_id("new-note"));
    h.query(by::test_id("undo"));
    h.query(by::test_id("browse-vault"));
    h.query(by::test_id("vault-name")).assert_text("gloss-fixture");
    assert_no_overflow(&h);
    h.set_viewport(1280, 640);
    h.query(by::test_id("new-note"));
    h.query(by::test_id("undo"));
    h.query(by::test_id("browse-vault"));
    h.query(by::test_id("vault-name")).assert_text("gloss-fixture");
    assert_no_overflow(&h);
}

#[test]
fn vault_chip_stays_below_the_filter_on_a_long_list() {
    let mut h = harness_at(720, 640);
    let listing = (0..80)
        .map(|i| format!("note-{i:02}.md"))
        .collect::<Vec<_>>()
        .join("\n");
    open_first_note(&mut h, &format!("{listing}\n"), "# n\n");
    h.query(by::test_id("vault-name")).assert_text("gloss-fixture");
    let laid = h.laid();
    let filter = laid
        .items
        .iter()
        .find(|i| i.semantics.test_id.as_deref() == Some("search"))
        .expect("filter");
    let chip = laid
        .items
        .iter()
        .find(|i| i.semantics.test_id.as_deref() == Some("vault-name"))
        .expect("vault name");
    assert!(
        chip.rect.y + chip.rect.h <= laid.height + 1.0,
        "vault name {} sits past viewport {}",
        chip.rect.y + chip.rect.h,
        laid.height
    );
    assert!(
        filter.rect.y + filter.rect.h <= chip.rect.y + 0.5,
        "filter bottom {} should be above vault name {}",
        filter.rect.y + filter.rect.h,
        chip.rect.y
    );
}
