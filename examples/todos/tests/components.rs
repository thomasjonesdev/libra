//! Component tests (§15.3): the §6.2 item-scoped field handler through
//! the real harness. Each todo row is a `<Button onPress={complete(t.id)}>`
//! inside a `<for>`; clicking *that row* dispatches the String-payload
//! action carrying the row's own id — the capability this example
//! exists to prove. Clicks route through real hit-testing and the app's
//! real `reduce`.

use libra_test::{by, Harness};
use todos::{Todo, Todos, VIEW};

fn harness() -> Harness<Todos> {
    Harness::<Todos>::with_viewport(VIEW, 460, 520)
}

fn add(h: &mut Harness<Todos>, text: &str) {
    h.type_text(by::test_id("draft"), text);
    h.click(by::test_id("add"));
}

#[test]
fn adding_a_todo_shows_it_as_an_active_row() {
    let mut h = harness();
    add(&mut h, "Buy milk");
    // The row renders with its text as the button label.
    h.query(by::text("Buy milk")).assert_text("Buy milk");
    h.query(by::test_id("summary")).assert_text("0 of 1 done");
    assert_eq!(h.state().draft, "", "draft clears after add");
}

#[test]
fn clicking_a_row_completes_that_specific_todo() {
    let mut h = harness();
    add(&mut h, "first");
    add(&mut h, "second");
    // Click the *second* row — the item-scoped payload must be id "1",
    // not the first row's.
    h.click(by::text("second"));
    assert_eq!(
        h.state().done,
        vec![Todo { id: 1, text: "second".into(), done: true }]
    );
    assert_eq!(
        h.state().active,
        vec![Todo { id: 0, text: "first".into(), done: false }]
    );
    h.query(by::test_id("summary")).assert_text("1 of 2 done");
}

#[test]
fn clicking_a_done_row_uncompletes_it() {
    let mut h = harness();
    add(&mut h, "task");
    h.click(by::text("task")); // complete → moves to Done
    h.click(by::text("task")); // now the Done row → uncomplete
    assert!(h.state().done.is_empty());
    assert_eq!(
        h.state().active,
        vec![Todo { id: 0, text: "task".into(), done: false }]
    );
}

#[test]
fn clicking_delete_removes_that_row() {
    let mut h = harness();
    add(&mut h, "keep");
    add(&mut h, "toss");
    // The first "×" belongs to the first active row ("keep", id 0).
    h.click(by::text("×"));
    assert_eq!(
        h.state().active,
        vec![Todo { id: 1, text: "toss".into(), done: false }]
    );
}

#[test]
fn clear_completed_button_drops_done_rows() {
    let mut h = harness();
    add(&mut h, "a");
    add(&mut h, "b");
    h.click(by::text("a")); // complete a
    h.click(by::test_id("clear"));
    assert_eq!(
        h.state().active,
        vec![Todo { id: 1, text: "b".into(), done: false }]
    );
    assert!(h.state().done.is_empty());
}
