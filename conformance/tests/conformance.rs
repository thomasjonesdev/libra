//! §6.4's no-drift guarantee, asserted: the interpreter and the
//! compiled engine produce identical ViewTrees — structurally, node
//! for node — for the same fixture across a spread of state values.

use conformance::{generated, TestAction, TestState};
use libra_core::{FrameInfo, ReflectAction, ReflectState, SeriesHandle, SeriesId};

const KITCHEN_SINK: &str = include_str!("../fixtures/kitchen_sink.lx");
const FOR_LOOP: &str = include_str!("../fixtures/for_loop.lx");
const FOR_ACTIONS: &str = include_str!("../fixtures/for_actions.lx");
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

/// §3.3's camelCase-only handler-name rule, asserted identically
/// across both engines (§6.4): `onPress={Reset}` (the pre-revision
/// PascalCase style) is rejected by interp and by codegen with the
/// same complaint and the same camelCase suggestion — not a fixture
/// under `fixtures/` (those must all compile cleanly for `build.rs`
/// to produce `generated`), but the same "one grammar, two engines"
/// guarantee applied to an *invalid* markup string.
#[test]
fn pascalcase_handler_name_is_rejected_identically_by_both_engines() {
    const SRC: &str = "<Column><Button onPress={Reset}>Reset</Button></Column>";

    let ast = libra_markup::parse(SRC).expect("parses");
    let field = |_: &str| None;
    let has_action = |m: &str| TestAction::action_by_name(m).is_some();
    let has_str_action = |_: &str| false;
    let collection = |_: &str| None;
    let has_frame_info_action = |_: &str| false;
    let scope = libra_interp::Scope {
        field: &field,
        has_action: &has_action,
        has_str_action: &has_str_action,
        collection: &collection,
        has_frame_info_action: &has_frame_info_action,
    };
    let interp_err = libra_interp::resolve(&ast, &scope).unwrap_err();
    assert!(interp_err.message.contains("must be camelCase"), "{}", interp_err.message);
    assert!(interp_err.message.contains("did you mean 'reset'"), "{}", interp_err.message);

    let codegen_err = libra_codegen::generate(
        &ast,
        &libra_codegen::Config {
            state: "crate::TestState".into(),
            action: "crate::TestAction".into(),
            fn_name: "build_rejected".into(),
            source: "rejected.lx".into(),
        },
    )
    .unwrap_err();
    assert!(codegen_err.message.contains("must be camelCase"), "{}", codegen_err.message);
    assert!(codegen_err.message.contains("did you mean 'reset'"), "{}", codegen_err.message);
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

/// §6.2's item-scoped handler `onPress={nameChanged(s)}` inside a
/// `<for>` — the payload is the loop binding's Display, resolved by
/// interp via `scope.field` and by codegen via `to_string(&s)`. Both
/// engines must agree on `on_press_arg` for every item, across item
/// counts (empty, one, several) so the loop-boundary can't hide drift.
#[test]
fn for_actions_item_scoped_handlers_produce_identical_trees() {
    let variants = [
        Vec::<SeriesHandle>::new(),
        vec![SeriesHandle::new(SeriesId(7), 1500.0)],
        vec![
            SeriesHandle::new(SeriesId(1), 1000.0),
            SeriesHandle::new(SeriesId(2), 2000.0),
            SeriesHandle::new(SeriesId(3), 3000.0),
        ],
    ];
    for (i, series) in variants.into_iter().enumerate() {
        let state = TestState { series, ..TestState::default() };
        let interp = interp_tree_src(FOR_ACTIONS, &state);
        let compiled = generated::build_for_actions(&state);
        assert_eq!(
            interp, compiled,
            "engine drift on for_actions fixture with {} series (variant #{i})",
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
            if children.contains(&libra_components::ViewNode::SourceFrames { on_tick: "frame".into() })),
        "expected a mounted Frames source in the tree: {interp:?}"
    );
}
