//! Harness interception for OpenUrl, Dialog, and Share
//! (PLAN-NATIVE-INTEGRATIONS N1, N2, N4).
//! Nothing is opened, no sheet is shown, and the clipboard is not touched.

use libra_core::{
    App, DialogResult, DialogSpec, Effect, OpenResult, ReflectAction, ReflectState, ShareResult,
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
    Confirm,
    ShareNote,
    Opened(OpenResult),
    Answered(DialogResult),
    Shared(ShareResult),
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
        (Model { status: String::new() }, vec![])
    }
    fn update(&mut self, msg: Msg) -> Vec<Effect<Msg>> {
        match msg {
            Msg::Open => {
                vec![Effect::open_url_acked("https://example.com/a?b=1&c=2", Msg::Opened)]
            }
            Msg::Confirm => vec![Effect::dialog(
                DialogSpec::confirm("Quit", "Leave?"),
                Msg::Answered,
            )],
            Msg::ShareNote => vec![Effect::share_acked(
                "see this",
                Some("https://example.com".into()),
                Msg::Shared,
            )],
            Msg::Opened(OpenResult::Opened) => {
                self.status = "opened".into();
                vec![]
            }
            Msg::Opened(other) => {
                self.status = format!("{other:?}");
                vec![]
            }
            Msg::Answered(DialogResult::Accepted) => {
                self.status = "accepted".into();
                vec![]
            }
            Msg::Answered(DialogResult::Cancelled) => {
                self.status = "cancelled".into();
                vec![]
            }
            Msg::Answered(other) => {
                self.status = format!("{other:?}");
                vec![]
            }
            Msg::Shared(ShareResult::Copied) => {
                self.status = "copied".into();
                vec![]
            }
            Msg::Shared(other) => {
                self.status = format!("{other:?}");
                vec![]
            }
        }
    }
}

#[test]
fn open_dialog_and_share_are_intercepted_and_resolved() {
    let mut h = Harness::<Model>::render(VIEW);

    h.dispatch(Msg::Open);
    let effect = h.expect_effect(fx::open_url_to("https://example.com/a?b=1&c=2"));
    assert!(matches!(
        &effect,
        Effect::OpenUrl { url, on_done: Some(_) } if url == "https://example.com/a?b=1&c=2"
    ));
    h.resolve_open_url(effect, OpenResult::Opened);
    assert_eq!(h.state().status, "opened");

    h.dispatch(Msg::Confirm);
    let effect = h.expect_effect(fx::dialog());
    match &effect {
        Effect::Dialog { spec, .. } => {
            assert_eq!(spec.title, "Quit");
            assert_eq!(spec.message, "Leave?");
        }
        other => panic!("expected a dialog, got {other:?}"),
    }
    h.resolve_dialog(effect, DialogResult::Cancelled);
    assert_eq!(h.state().status, "cancelled");

    h.dispatch(Msg::ShareNote);
    let effect = h.expect_effect(fx::share());
    match &effect {
        Effect::Share { text, url, .. } => {
            assert_eq!(text, "see this");
            assert_eq!(url.as_deref(), Some("https://example.com"));
            assert_eq!(
                libra_core::share_payload(text, url.as_deref()).unwrap(),
                "see this\nhttps://example.com"
            );
        }
        other => panic!("expected a share, got {other:?}"),
    }
    h.resolve_share(effect, ShareResult::Copied);
    assert_eq!(h.state().status, "copied");
    h.expect_no_effects();
}

#[test]
fn a_responder_answers_the_three_sheets() {
    let mut h = Harness::<Model>::render(VIEW).responder(|effect| match effect {
        Effect::OpenUrl { .. } => Reply::OpenUrl(OpenResult::Opened),
        Effect::Dialog { .. } => Reply::Dialog(DialogResult::Accepted),
        Effect::Share { .. } => Reply::Share(ShareResult::Copied),
        _ => Reply::Drop,
    });
    h.dispatch(Msg::Open);
    h.dispatch(Msg::Confirm);
    h.dispatch(Msg::ShareNote);
    h.drain_effects();
    assert_eq!(h.state().status, "copied");
    h.expect_no_effects();
}
