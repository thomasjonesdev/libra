//! §6.4's no-drift guarantee, asserted: the interpreter and the
//! compiled engine produce identical ViewTrees — structurally, node
//! for node — for the same fixture across a spread of state values.

use conformance::{generated, TestAction, TestState};
use libra_core::{FrameInfo, ReflectAction, ReflectState, SeriesHandle, SeriesId};

const KITCHEN_SINK: &str = include_str!("../fixtures/kitchen_sink.lx");
const FOR_LOOP: &str = include_str!("../fixtures/for_loop.lx");
const FRAMES: &str = include_str!("../fixtures/frames.lx");

fn interp_tree_src(src: &str, state: &TestState) -> libra_components::ViewNode {
    let ast = libra_markup::parse(src).expect("fixture parses");
    let field = |p: &str| state.field(p);
    let has_action = |m: &str| TestAction::action_by_name(m).is_some();
    let has_str_action = |m: &str| TestAction::action_with_str(m, String::new()).is_some();
    let collection = |p: &str| state.collection(p);
    let has_frame_info_action =
        |m: &str| TestAction::action_with_frame_info(m, FrameInfo::at(0.0)).is_some();
    let scope = libra_interp::Scope {
        field: &field,
        has_action: &has_action,
        has_str_action: &has_str_action,
        collection: &collection,
        has_frame_info_action: &has_frame_info_action,
    };
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
            ..TestState::default()
        },
    ]
}

#[test]
fn interp_and_codegen_produce_identical_trees() {
    for (i, state) in state_variants().into_iter().enumerate() {
        let interp = interp_tree_src(KITCHEN_SINK, &state);
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

/// §6.2's `<for each={series} as="s" key={s}>` — the real E2 proof:
/// interp's runtime iteration and codegen's `.iter().flat_map(...)`
/// must produce byte-for-byte identical trees, across zero, one, and
/// several items (the loop-count boundary is exactly where engine
/// drift would show up first).
#[test]
fn for_loop_produces_identical_trees_across_series_counts() {
    let variants = [
        Vec::<SeriesHandle>::new(),
        vec![SeriesHandle::new(SeriesId(1), 1000.0)],
        vec![
            SeriesHandle::new(SeriesId(1), 1000.0),
            SeriesHandle::new(SeriesId(2), 2000.0),
            SeriesHandle::new(SeriesId(3), 3000.0),
        ],
    ];
    for (i, series) in variants.into_iter().enumerate() {
        let state = TestState { series, ..TestState::default() };
        let interp = interp_tree_src(FOR_LOOP, &state);
        let compiled = generated::build_for_loop(&state);
        assert_eq!(
            interp, compiled,
            "engine drift on for_loop fixture with {} series (variant #{i})",
            state.series.len()
        );
    }
}

/// §12.5's `<Frames onTick={frame}>` — interp's runtime resolution
/// (via ReflectAction::action_with_frame_info) and codegen's typed
/// `fn(FrameInfo) -> Action` coercion must agree on the exact same
/// ViewNode::SourceFrames { on_tick } value.
#[test]
fn frames_source_element_produces_identical_trees() {
    let state = TestState::default();
    let interp = interp_tree_src(FRAMES, &state);
    let compiled = generated::build_frames(&state);
    assert_eq!(interp, compiled, "engine drift on frames fixture — one grammar, two engines (§6.4)");
    assert!(
        matches!(&interp, libra_components::ViewNode::Column { children, .. }
            if children.contains(&libra_components::ViewNode::SourceFrames { on_tick: "Frame".into() })),
        "expected a mounted Frames source in the tree: {interp:?}"
    );
}
