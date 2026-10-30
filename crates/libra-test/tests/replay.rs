//! `.lreplay` round trip (§15.6). No window, no OS.

use std::path::PathBuf;

use libra_core::{App, Effect, ReflectAction, ReflectState};
use libra_test::{Harness, Replay, ReplayStep};

const VIEW: &str = r#"<Column><Text testId="n">{n}</Text></Column>"#;

#[derive(Debug, Clone, PartialEq)]
struct Model {
    n: i64,
}

#[derive(Debug, Clone, PartialEq)]
enum Msg {
    Increment,
}

impl ReflectState for Model {
    fn field(&self, path: &str) -> Option<String> {
        (path == "n").then(|| self.n.to_string())
    }
    fn field_names() -> &'static [&'static str] {
        &["n"]
    }
}

impl ReflectAction for Msg {
    fn action_by_name(name: &str) -> Option<Self> {
        (name == "increment").then_some(Msg::Increment)
    }
    fn action_names() -> &'static [&'static str] {
        &["increment"]
    }
}

impl App for Model {
    type Msg = Msg;
    fn init() -> (Self, Vec<Effect<Msg>>) {
        (Model { n: 0 }, vec![])
    }
    fn update(&mut self, msg: Msg) -> Vec<Effect<Msg>> {
        match msg {
            Msg::Increment => self.n += 1,
        }
        vec![]
    }
}

fn fresh() -> Harness<Model> {
    Harness::render(VIEW)
}

fn recorded() -> Replay {
    let mut h = fresh();
    h.dispatch(Msg::Increment);
    h.dispatch(Msg::Increment);
    Replay {
        prelude_discard: 0,
        steps: vec![
            ReplayStep {
                action: "increment".into(),
                arg: None,
                discard: 0,
            },
            ReplayStep {
                action: "increment".into(),
                arg: None,
                discard: 0,
            },
        ],
        state_hash: h.state_hash(),
        frame: h.frame_hash(),
    }
}

#[test]
fn lreplay_two_increments_match_state_and_frame() {
    let replay = recorded();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/increment.lreplay");
    if !path.exists() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, replay.to_json()).unwrap();
    }
    let loaded = Replay::load(&path).unwrap();
    assert_eq!(loaded.state_hash, replay.state_hash);
    assert_eq!(loaded.frame, replay.frame);
    let mut h = fresh();
    h.replay(&loaded).unwrap();
    assert_eq!(h.state().n, 2);
    let mut bad = loaded.clone();
    bad.frame = "nope".into();
    assert!(fresh().replay(&bad).unwrap_err().contains("frame hash"));
}
