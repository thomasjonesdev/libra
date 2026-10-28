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
    h.query(by::test_id("vault-picker"));
    h.query(by::test_id("vault-name")).assert_text("gloss-fixture");
    assert_no_overflow(&h);
    h.set_viewport(1280, 640);
    h.query(by::test_id("new-note"));
    h.query(by::test_id("undo"));
    h.query(by::test_id("vault-picker"));
    h.query(by::test_id("vault-name")).assert_text("gloss-fixture");
    assert_no_overflow(&h);
}

#[test]
fn vault_name_shows_in_the_status_bar() {
    let mut h = harness_at(720, 640);
    let listing = (0..80)
        .map(|i| format!("note-{i:02}.md"))
        .collect::<Vec<_>>()
        .join("\n");
    open_first_note(&mut h, &format!("{listing}\n"), "# n\n");
    h.query(by::test_id("vault-name")).assert_text("gloss-fixture");
    let laid = h.laid();
    let status_bar = laid
        .items
        .iter()
        .find(|i| i.semantics.test_id.as_deref() == Some("status-bar"))
        .expect("status bar");
    let chip = laid
        .items
        .iter()
        .find(|i| i.semantics.test_id.as_deref() == Some("vault-name"))
        .expect("vault name");
    assert!(
        chip.rect.y >= status_bar.rect.y - 1.0
            && chip.rect.y + chip.rect.h <= status_bar.rect.y + status_bar.rect.h + 1.0,
        "vault name should sit inside the status bar"
    );
    assert!(
        chip.rect.x >= status_bar.rect.x,
        "vault name should live inside the status bar"
    );

    h.click(by::test_id("sidebar-search"));
    h.query(by::test_id("vault-name")).assert_text("gloss-fixture");
    h.query(by::test_id("search"));
}
