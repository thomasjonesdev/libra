//! The derive exercised against the real libra-core traits (dev-dep
//! cycle, permitted by cargo).

use libra_core::{ReflectModel, ReflectMsg};
use libra_reflect::LibraReflect;

#[derive(LibraReflect)]
struct Model {
    count: i64,
    label: String,
    ratio: f32,
    #[libra(skip)]
    #[allow(dead_code)]
    internal: Vec<u8>, // not Display — skipped, so this still compiles
}

#[derive(Debug, Clone, PartialEq, LibraReflect)]
#[allow(dead_code)] // payload variants exist to prove they don't construct by name
enum Msg {
    Increment,
    Reset,
    Loaded(Option<Vec<u8>>),
    Moved { x: f32 },
    TitleChanged(String),
}

#[test]
fn display_fields_bind_and_skipped_fields_do_not() {
    let m = Model { count: 3, label: "hi".into(), ratio: 0.5, internal: vec![1] };
    assert_eq!(m.field("count").as_deref(), Some("3"));
    assert_eq!(m.field("label").as_deref(), Some("hi"));
    assert_eq!(m.field("ratio").as_deref(), Some("0.5"));
    assert_eq!(m.field("internal"), None); // skipped
    assert_eq!(m.field("cuont"), None); // typo → load-time interp error
    assert_eq!(Model::field_names(), &["count", "label", "ratio"]);
}

#[test]
fn unit_variants_construct_and_payload_variants_do_not() {
    assert_eq!(Msg::msg_by_name("Increment"), Some(Msg::Increment));
    assert_eq!(Msg::msg_by_name("Reset"), Some(Msg::Reset));
    // Payload variants are effect results, not handlers:
    assert_eq!(Msg::msg_by_name("Loaded"), None);
    assert_eq!(Msg::msg_by_name("Moved"), None);
    assert_eq!(Msg::msg_by_name("Incremnt"), None);
    // …but every variant is visible to diagnostics:
    assert_eq!(Msg::msg_names(), &["Increment", "Reset", "Loaded", "Moved", "TitleChanged"]);
}

#[test]
fn string_payload_variants_construct_with_a_control_value() {
    assert_eq!(
        Msg::msg_with_str("TitleChanged", "hi".into()),
        Some(Msg::TitleChanged("hi".into()))
    );
    // Unit and non-String-payload variants do not:
    assert_eq!(Msg::msg_with_str("Increment", "x".into()), None);
    assert_eq!(Msg::msg_with_str("Loaded", "x".into()), None);
    assert_eq!(Msg::msg_with_str("Moved", "x".into()), None);
}
