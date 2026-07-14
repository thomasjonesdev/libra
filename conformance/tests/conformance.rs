//! §6.4's no-drift guarantee, asserted: the interpreter and the
//! compiled engine produce identical ViewTrees — structurally, node
//! for node — for the same fixture across a spread of state values.

use conformance::{generated, TestAction, TestState};
use libra_core::{ReflectAction, ReflectState, SeriesHandle, SeriesId};

const KITCHEN_SINK: &str = include_str!("../fixtures/kitchen_sink.lx");

fn interp_tree(state: &TestState) -> libra_components::ViewNode {
    let ast = libra_markup::parse(KITCHEN_SINK).expect("fixture parses");
    let field = |p: &str| state.field(p);
    let has_action = |m: &str| TestAction::action_by_name(m).is_some();
    let has_str_action = |m: &str| TestAction::action_with_str(m, String::new()).is_some();
    let scope = libra_interp::Scope { field: &field, has_action: &has_action, has_str_action: &has_str_action };
    libra_interp::resolve(&ast, &scope).expect("fixture resolves")
}

fn state_variants() -> Vec<TestState> {
    vec![
        TestState::default(),
        TestState { count: 42, name: "Ada Lovelace".into(), status: "live".into(), ..TestState::default() },
        TestState { count: -7, name: "🎉 unicode näme".into(), status: "degraded · 3 warnings".into(), ..TestState::default() },
        TestState {
            count: i64::MAX,
            name: "with \"quotes\" and \\ backslashes".into(),
            status: String::new(),
            s0: SeriesHandle::new(SeriesId(9), 500.0),
        },
    ]
}

#[test]
fn interp_and_codegen_produce_identical_trees() {
    for (i, state) in state_variants().into_iter().enumerate() {
        let interp = interp_tree(&state);
        let compiled = generated::build_kitchen_sink(&state);
        assert_eq!(
            interp, compiled,
            "engine drift on state variant #{i} — one grammar, two engines, one truth (§6.4)"
        );
    }
}

/// The compiled engine is a pure function of state — same input,
/// same tree, no interior state.
#[test]
fn compiled_view_is_pure() {
    let state = state_variants().remove(1);
    assert_eq!(generated::build_kitchen_sink(&state), generated::build_kitchen_sink(&state));
}
