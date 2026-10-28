//! Harness delivery for `<Stream>` (PLAN-STREAM-DELIVERY D4).
//! Events are fixtures. Nothing is spawned.

use libra_core::{
    App, Capture, CommandSpec, Effect, ProcessStatus, ReflectAction, ReflectState, StreamClose,
    StreamEvent,
};
use libra_test::{by, fx, Harness};

const VIEW: &str = r#"<Column>
  <Text testId="log">{log}</Text>
  <if test={open}>
    <Stream id={stream_id} onEvent={onLine} />
  </if>
</Column>"#;

#[derive(Debug, Clone, PartialEq)]
struct Log {
    open: bool,
    stream_id: String,
    log: String,
    lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
enum Action {
    Open,
    OnLine(StreamEvent),
}

impl ReflectState for Log {
    fn field(&self, path: &str) -> Option<String> {
        match path {
            "open" => Some(self.open.to_string()),
            "stream_id" => Some(self.stream_id.clone()),
            "log" => Some(self.log.clone()),
            _ => None,
        }
    }
    fn field_names() -> &'static [&'static str] {
        &["open", "stream_id", "log"]
    }
}

impl ReflectAction for Action {
    fn action_by_name(name: &str) -> Option<Self> {
        (name == "open").then_some(Action::Open)
    }
    fn action_with_stream_event(name: &str, event: StreamEvent) -> Option<Self> {
        (name == "onLine").then_some(Action::OnLine(event))
    }
    fn action_names() -> &'static [&'static str] {
        &["open", "onLine"]
    }
}

impl App for Log {
    type Msg = Action;
    fn init() -> (Self, Vec<Effect<Action>>) {
        (
            Log {
                open: false,
                stream_id: "log".into(),
                log: String::new(),
                lines: Vec::new(),
            },
            vec![],
        )
    }
    fn update(&mut self, action: Action) -> Vec<Effect<Action>> {
        match action {
            Action::Open => {
                self.open = true;
                vec![Effect::spawn(
                    self.stream_id.clone(),
                    CommandSpec::new("echo").arg("hi"),
                    Capture::Stdout,
                )]
            }
            Action::OnLine(StreamEvent::Line { text, .. }) => {
                self.lines.push(text);
                self.log = self.lines.join("\n");
                vec![]
            }
            Action::OnLine(StreamEvent::Closed(_)) => {
                self.lines.push("closed".into());
                self.log = self.lines.join("\n");
                vec![]
            }
            Action::OnLine(_) => vec![],
        }
    }
}

#[test]
fn emit_stream_delivers_in_order_once_the_handler_is_mounted() {
    let mut h = Harness::<Log>::render(VIEW);
    h.dispatch(Action::Open);
    let effect = h.expect_effect(fx::spawn_program("echo"));
    match effect {
        Effect::Spawn { stream, capture, .. } => {
            assert_eq!(stream, "log");
            assert_eq!(capture, Capture::Stdout);
        }
        other => panic!("expected spawn, got {other:?}"),
    }
    h.emit_stream(
        "log",
        StreamEvent::Line {
            text: "one".into(),
            stderr: false,
        },
    );
    h.emit_stream(
        "log",
        StreamEvent::Line {
            text: "two".into(),
            stderr: false,
        },
    );
    h.close_stream("log", StreamClose::Process(ProcessStatus::Exited(0)));
    h.query(by::test_id("log")).assert_text("one\ntwo\nclosed");
}
