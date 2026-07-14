//! The derive exercised against the real libra-core traits (dev-dep
//! cycle, permitted by cargo).

use libra_core::{FrameInfo, ReflectAction, ReflectState};
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

#[test]
fn unit_variants_construct_and_payload_variants_do_not() {
    assert_eq!(Action::action_by_name("Increment"), Some(Action::Increment));
    assert_eq!(Action::action_by_name("Reset"), Some(Action::Reset));
    // Payload variants are effect results, not handlers:
    assert_eq!(Action::action_by_name("Loaded"), None);
    assert_eq!(Action::action_by_name("Moved"), None);
    assert_eq!(Action::action_by_name("Incremnt"), None);
    // …but every variant is visible to diagnostics:
    assert_eq!(
        Action::action_names(),
        &["Increment", "Reset", "Loaded", "Moved", "TitleChanged", "Ticked"]
    );
}

#[test]
fn string_payload_variants_construct_with_a_control_value() {
    assert_eq!(
        Action::action_with_str("TitleChanged", "hi".into()),
        Some(Action::TitleChanged("hi".into()))
    );
    // Unit and non-String-payload variants do not:
    assert_eq!(Action::action_with_str("Increment", "x".into()), None);
    assert_eq!(Action::action_with_str("Loaded", "x".into()), None);
    assert_eq!(Action::action_with_str("Moved", "x".into()), None);
    assert_eq!(Action::action_with_str("Ticked", "x".into()), None);
}

#[test]
fn frame_info_payload_variants_construct_for_source_element_ticks() {
    let info = FrameInfo::at(2500.0);
    assert_eq!(Action::action_with_frame_info("Ticked", info), Some(Action::Ticked(info)));
    // Unit and non-FrameInfo-payload variants do not:
    assert_eq!(Action::action_with_frame_info("Increment", info), None);
    assert_eq!(Action::action_with_frame_info("TitleChanged", info), None);
}
