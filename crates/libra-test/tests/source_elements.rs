//! §12.5 source elements exercised through the real harness: mounted
//! declarations are queryable as component-test data
//! (`by::source`/`has_source`), and re-rendering after a state change
//! that toggles whether `<Frames>` appears makes it mount/unmount —
//! the same mechanism a real runtime's differ would observe.

use libra_core::{App, Effect, FrameInfo, ReflectAction, ReflectState};
use libra_test::{by, Harness};

// The grammar has no <if> yet (§26 open question), so there's no
// single view that toggles <Frames> based on state. Instead, this
// test compares two *different* view sources against the same app —
// the presence/absence check a real conditional would produce once
// <if> lands, exercised at the level that's actually implemented today.
const VIEW_NO_FRAMES: &str = r#"<Column>
  <Text testId="status">{status}</Text>
</Column>"#;

const VIEW_WITH_FRAMES: &str = r#"<Column>
  <Text testId="status">{status}</Text>
  <Frames onTick={Tick} />
</Column>"#;

#[derive(Debug, Clone, PartialEq)]
struct Counter {
    status: String,
    ticks: u32,
}

#[derive(Debug, Clone, PartialEq)]
enum Action {
    Tick(FrameInfo),
}

impl ReflectState for Counter {
    fn field(&self, path: &str) -> Option<String> {
        (path == "status").then(|| self.status.clone())
    }
    fn field_names() -> &'static [&'static str] {
        &["status"]
    }
}

impl ReflectAction for Action {
    fn action_by_name(_: &str) -> Option<Self> {
        None
    }
    fn action_with_frame_info(name: &str, info: FrameInfo) -> Option<Self> {
        (name == "Tick").then(|| Action::Tick(info))
    }
    fn action_names() -> &'static [&'static str] {
        &["Tick"]
    }
}

impl App for Counter {
    // App::Msg is the trait's permanent associated-type name (see its
    // doc comment in libra-core) — only the concrete type assigned to
    // it is named Action here.
    type Msg = Action;
    fn init() -> (Self, Vec<Effect<Action>>) {
        (Counter { status: "idle".into(), ticks: 0 }, vec![])
    }
    fn update(&mut self, action: Action) -> Vec<Effect<Action>> {
        match action {
            Action::Tick(info) => {
                self.ticks += 1;
                self.status = format!("tick {} at {}ms", self.ticks, info.t_ms);
            }
        }
        vec![]
    }
}

#[test]
fn a_mounted_frames_source_is_queryable_by_kind() {
    let h = Harness::<Counter>::render(VIEW_WITH_FRAMES);
    assert!(h.has_source("frames"));
    h.query(by::source("frames")); // doesn't panic: it's there
}

#[test]
fn no_frames_element_means_no_mounted_source() {
    let h = Harness::<Counter>::render(VIEW_NO_FRAMES);
    assert!(!h.has_source("frames"));
}

#[test]
fn source_element_is_invisible_but_dispatch_through_it_still_works() {
    // The source element itself carries no interactivity (no
    // on_press) — but the action it names is a completely ordinary
    // one, dispatchable exactly like any other (this is what the
    // real runtime's tick delivery does under the hood).
    let mut h = Harness::<Counter>::render(VIEW_WITH_FRAMES);
    h.query(by::test_id("status")).assert_text("idle");
    h.dispatch(Action::Tick(FrameInfo::at(16.0)));
    h.query(by::test_id("status")).assert_text("tick 1 at 16ms");
}
