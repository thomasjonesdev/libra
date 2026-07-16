//! The derive exercised against the real libra-core traits (dev-dep
//! cycle, permitted by cargo).

use libra_core::{FrameInfo, Instant, ReflectAction, ReflectState};
use libra_reflect::LibraReflect;

#[derive(LibraReflect)]
struct State {
    count: i64,
    label: String,
    ratio: f32,
    #[libra(skip)]
    #[allow(dead_code)]
    internal: Vec<u8>, // not Display — skipped, so this still compiles
}

#[derive(Debug, Clone, PartialEq, LibraReflect)]
#[allow(dead_code)] // payload variants exist to prove they don't construct by name
enum Action {
    Increment,
    Reset,
    Loaded(Option<Vec<u8>>),
    Moved { x: f32 },
    TitleChanged(String),
    Ticked(FrameInfo),
    Polled(Instant),
}

#[test]
fn display_fields_bind_and_skipped_fields_do_not() {
    let s = State { count: 3, label: "hi".into(), ratio: 0.5, internal: vec![1] };
    assert_eq!(s.field("count").as_deref(), Some("3"));
    assert_eq!(s.field("label").as_deref(), Some("hi"));
    assert_eq!(s.field("ratio").as_deref(), Some("0.5"));
    assert_eq!(s.field("internal"), None); // skipped
    assert_eq!(s.field("cuont"), None); // typo → load-time interp error
    assert_eq!(State::field_names(), &["count", "label", "ratio"]);
}

#[derive(Debug, Clone, PartialEq, LibraReflect)]
struct User {
    name: String,
    active: bool,
}

impl std::fmt::Display for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(LibraReflect)]
struct NestedState {
    user: User,
    items: Vec<String>,
    title: String,
}

#[test]
fn nested_paths_and_accessors_resolve() {
    let s = NestedState {
        user: User { name: "Ada".into(), active: true },
        items: vec!["a".into(), "b".into()],
        title: String::new(),
    };
    assert_eq!(s.field("user.name").as_deref(), Some("Ada"));
    assert_eq!(s.field("user.active").as_deref(), Some("true"));
    assert_eq!(s.field("items.len").as_deref(), Some("2"));
    assert_eq!(s.field("items.len()").as_deref(), Some("2"));
    assert_eq!(s.field("items.isEmpty").as_deref(), Some("false"));
    assert_eq!(s.field("title.isEmpty").as_deref(), Some("true"));
    assert_eq!(s.field("title.len()").as_deref(), Some("0"));
    assert_eq!(s.field("user.missing"), None);
}

#[test]
fn unit_variants_construct_and_payload_variants_do_not() {
    assert_eq!(Action::action_by_name("increment"), Some(Action::Increment));
    assert_eq!(Action::action_by_name("reset"), Some(Action::Reset));
    // Payload variants are effect results, not handlers:
    assert_eq!(Action::action_by_name("loaded"), None);
    assert_eq!(Action::action_by_name("moved"), None);
    assert_eq!(Action::action_by_name("incremnt"), None);
    // The pre-revision PascalCase form no longer resolves (§3.3 — one
    // canonical camelCase public name):
    assert_eq!(Action::action_by_name("Increment"), None);
    // …but every variant is visible to diagnostics, by its public name:
    assert_eq!(
        Action::action_names(),
        &["increment", "reset", "loaded", "moved", "titleChanged", "ticked", "polled"]
    );
}

#[test]
fn string_payload_variants_construct_with_a_control_value() {
    assert_eq!(
        Action::action_with_str("titleChanged", "hi".into()),
        Some(Action::TitleChanged("hi".into()))
    );
    // Unit and non-String-payload variants do not:
    assert_eq!(Action::action_with_str("increment", "x".into()), None);
    assert_eq!(Action::action_with_str("loaded", "x".into()), None);
    assert_eq!(Action::action_with_str("moved", "x".into()), None);
    assert_eq!(Action::action_with_str("ticked", "x".into()), None);
}

#[test]
fn frame_info_payload_variants_construct_for_source_element_ticks() {
    let info = FrameInfo::at(2500.0);
    assert_eq!(Action::action_with_frame_info("ticked", info), Some(Action::Ticked(info)));
    // Unit and non-FrameInfo-payload variants do not:
    assert_eq!(Action::action_with_frame_info("increment", info), None);
    assert_eq!(Action::action_with_frame_info("titleChanged", info), None);
}

#[test]
fn instant_payload_variants_construct_for_interval_source_element_ticks() {
    let instant = Instant::at(4200.0);
    assert_eq!(Action::action_with_instant("polled", instant), Some(Action::Polled(instant)));
    // Unit and non-Instant-payload variants do not:
    assert_eq!(Action::action_with_instant("increment", instant), None);
    assert_eq!(Action::action_with_instant("titleChanged", instant), None);
}
