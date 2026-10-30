//! File sheets and window chrome (PLAN-NATIVE-INTEGRATIONS).
//! No OS sheet is shown and no window is resized.

use libra_core::{
    App, Effect, FilePick, ReflectAction, ReflectState, WindowOp,
};
use libra_test::{fx, Harness, Reply};

const VIEW: &str = r#"<Column><Text testId="status">{status}</Text></Column>"#;

#[derive(Debug, Clone, PartialEq)]
struct Model {
    status: String,
}

#[derive(Debug, Clone, PartialEq)]
enum Msg {
    Open,
    Save,
    Retitle,
    Grow,
    Full,
    BadSize,
    Picked(FilePick),
}

impl ReflectState for Model {
    fn field(&self, path: &str) -> Option<String> {
        (path == "status").then(|| self.status.clone())
    }
    fn field_names() -> &'static [&'static str] {
        &["status"]
    }
}

impl ReflectAction for Msg {
    fn action_by_name(_: &str) -> Option<Self> {
        None
    }
    fn action_names() -> &'static [&'static str] {
        &[]
    }
}

impl App for Model {
    type Msg = Msg;
    fn init() -> (Self, Vec<Effect<Msg>>) {
        (
            Model {
                status: String::new(),
            },
            vec![Effect::window(WindowOp::Title("Notes".into()))],
        )
    }
    fn update(&mut self, msg: Msg) -> Vec<Effect<Msg>> {
        match msg {
            Msg::Open => vec![Effect::pick_file_titled("Open note", Msg::Picked)],
            Msg::Save => vec![Effect::save_file_as("note.md", Msg::Picked)],
            Msg::Retitle => vec![Effect::window(WindowOp::Title("  Renamed  ".into()))],
            Msg::Grow => vec![Effect::window(WindowOp::Resize {
                width: 1024.0,
                height: 700.0,
            })],
            Msg::Full => vec![Effect::window(WindowOp::Fullscreen(true))],
            Msg::BadSize => vec![Effect::window(WindowOp::Resize {
                width: 0.0,
                height: 10.0,
            })],
            Msg::Picked(FilePick::Picked(path)) => {
                self.status = path;
                vec![]
            }
            Msg::Picked(FilePick::Cancelled) => {
                self.status = "cancelled".into();
                vec![]
            }
            Msg::Picked(other) => {
                self.status = format!("{other:?}");
                vec![]
            }
        }
    }
}

#[test]
fn init_title_is_recorded_and_later_ops_update_chrome() {
    let mut h = Harness::<Model>::with_viewport(VIEW, 800, 600);
    assert_eq!(h.chrome().title, "Notes");
    assert_eq!((h.chrome().width, h.chrome().height), (800.0, 600.0));
    h.expect_effect(fx::window_op());

    h.dispatch(Msg::Retitle);
    h.expect_effect(fx::window_op());
    assert_eq!(h.chrome().title, "Renamed");

    h.dispatch(Msg::Grow);
    h.dispatch(Msg::Full);
    h.expect_effect(fx::window_op());
    h.expect_effect(fx::window_op());
    assert_eq!((h.chrome().width, h.chrome().height), (1024.0, 700.0));
    assert!(h.chrome().fullscreen);

    h.dispatch(Msg::BadSize);
    h.expect_effect(fx::window_op());
    assert_eq!((h.chrome().width, h.chrome().height), (1024.0, 700.0));
}

#[test]
fn file_sheets_are_intercepted() {
    let mut h = Harness::<Model>::with_viewport(VIEW, 400, 300);
    h.expect_effect(fx::window_op());

    h.dispatch(Msg::Open);
    let open = h.expect_effect(fx::pick_file());
    match &open {
        Effect::PickFile { title, .. } => assert_eq!(title, "Open note"),
        other => panic!("expected PickFile, got {other:?}"),
    }
    h.resolve_file_pick(open, FilePick::Picked("/tmp/note.md".into()));
    assert_eq!(h.state().status, "/tmp/note.md");

    h.dispatch(Msg::Save);
    let save = h.expect_effect(fx::save_file());
    match &save {
        Effect::SaveFile { name, .. } => assert_eq!(name.as_deref(), Some("note.md")),
        other => panic!("expected SaveFile, got {other:?}"),
    }
    h.resolve_file_pick(save, FilePick::Cancelled);
    assert_eq!(h.state().status, "cancelled");
    h.expect_no_effects();
}

#[test]
fn a_responder_answers_a_file_sheet() {
    let mut h = Harness::<Model>::with_viewport(VIEW, 400, 300).responder(|effect| match effect {
        Effect::WindowOp { .. } => Reply::Drop,
        Effect::PickFile { .. } => Reply::FilePick(FilePick::Picked("/vault/a.md".into())),
        _ => Reply::Drop,
    });
    h.drain_effects();
    h.dispatch(Msg::Open);
    h.drain_effects();
    assert_eq!(h.state().status, "/vault/a.md");
    h.expect_no_effects();
}
