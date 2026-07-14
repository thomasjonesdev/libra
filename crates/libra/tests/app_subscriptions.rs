//! Covers `libra::app!`'s optional `subscriptions:` field — needed by
//! any app with state-dependent ambient event sources (a dashboard's
//! "only tick while running"), since §12.5 source elements can't yet
//! be conditionally mounted (no `<if>` in the grammar).

use libra::{actions, app, state};
use libra_core::{App, Sub};

#[derive(Default)]
#[state]
struct Player {
    playing: bool,
}

#[actions]
impl Player {
    fn toggle(&mut self) {
        self.playing = !self.playing;
    }
    fn tick(&mut self, _info: libra_core::FrameInfo) {}
}

fn subs(state: &Player) -> Vec<Sub<Action>> {
    if state.playing {
        vec![Sub::frames(Action::Tick)]
    } else {
        vec![]
    }
}

app! {
    state: Player,
    view: "fixtures/counter.lx",
    subscriptions: subs,
}

#[test]
fn subscriptions_are_empty_until_toggled_on() {
    let mut p = Player::default();
    assert!(p.subscriptions().is_empty());
    p.playing = true;
    assert_eq!(p.subscriptions().len(), 1);
}
