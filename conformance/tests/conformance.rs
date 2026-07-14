//! §6.4's no-drift guarantee, asserted: the interpreter and the
//! compiled engine produce identical ViewTrees — structurally, node
//! for node — for the same fixture across a spread of model states.

use conformance::{generated, TestModel, TestMsg};
use libra_core::{ReflectModel, ReflectMsg, SeriesHandle, SeriesId};

const KITCHEN_SINK: &str = include_str!("../fixtures/kitchen_sink.lx");

fn interp_tree(model: &TestModel) -> libra_widgets::ViewNode {
    let ast = libra_markup::parse(KITCHEN_SINK).expect("fixture parses");
    let field = |p: &str| model.field(p);
    let has_msg = |m: &str| TestMsg::msg_by_name(m).is_some();
    let has_str_msg = |m: &str| TestMsg::msg_with_str(m, String::new()).is_some();
    let scope = libra_interp::Scope { field: &field, has_msg: &has_msg, has_str_msg: &has_str_msg };
    libra_interp::resolve(&ast, &scope).expect("fixture resolves")
}

fn model_states() -> Vec<TestModel> {
    vec![
        TestModel::default(),
        TestModel { count: 42, name: "Ada Lovelace".into(), status: "live".into(), ..TestModel::default() },
        TestModel { count: -7, name: "🎉 unicode näme".into(), status: "degraded · 3 warnings".into(), ..TestModel::default() },
        TestModel {
            count: i64::MAX,
            name: "with \"quotes\" and \\ backslashes".into(),
            status: String::new(),
            s0: SeriesHandle::new(SeriesId(9), 500.0),
        },
    ]
}

#[test]
fn interp_and_codegen_produce_identical_trees() {
    for (i, model) in model_states().into_iter().enumerate() {
        let interp = interp_tree(&model);
        let compiled = generated::build_kitchen_sink(&model);
        assert_eq!(
            interp, compiled,
            "engine drift on model state #{i} — one grammar, two engines, one truth (§6.4)"
        );
    }
}

/// The compiled engine is a pure function of the model — same input,
/// same tree, no interior state.
#[test]
fn compiled_view_is_pure() {
    let model = model_states().remove(1);
    assert_eq!(generated::build_kitchen_sink(&model), generated::build_kitchen_sink(&model));
}
