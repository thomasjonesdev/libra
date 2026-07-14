//! Unit tests (§15.2): pure actions.

use libra_core::KvResult;
use notes::{reduce, Action, Notes};

#[test]
fn title_changed_is_an_event_not_a_setter_chain() {
    let (mut s, _) = Notes::init();
    assert!(reduce(&mut s, Action::TitleChanged("draft".into())).is_empty());
    assert_eq!((s.title.as_str(), s.status.as_str()), ("draft", "unsaved"));
    let fx = reduce(&mut s, Action::Save);
    assert_eq!(fx.len(), 1);
    reduce(&mut s, Action::Saved(KvResult::Ok));
    assert_eq!(s.status, "saved");
}
