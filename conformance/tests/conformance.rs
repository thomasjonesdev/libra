//! §6.4's no-drift guarantee, asserted: the interpreter and the
//! compiled engine produce identical ViewTrees — structurally, node
//! for node — for the same fixture across a spread of state values.

use conformance::{generated, Item, TestAction, TestState, User};
use libra_core::{FrameInfo, ReflectAction, ReflectState, SeriesHandle, SeriesId};

const KITCHEN_SINK: &str = include_str!("../fixtures/kitchen_sink.lx");
const FOR_LOOP: &str = include_str!("../fixtures/for_loop.lx");
const FOR_ACTIONS: &str = include_str!("../fixtures/for_actions.lx");
const FOR_ITEM_FIELDS: &str = include_str!("../fixtures/for_item_fields.lx");
const IF_ELSE: &str = include_str!("../fixtures/if_else.lx");
const FRAMES: &str = include_str!("../fixtures/frames.lx");
const INTERVAL_STREAM: &str = include_str!("../fixtures/interval_stream.lx");
const ACCESSORS: &str = include_str!("../fixtures/accessors.lx");
const CHROME: &str = include_str!("../fixtures/chrome.lx");
const LIST_TEXTAREA: &str = include_str!("../fixtures/list_textarea.lx");
const LIST_ITEM_INSET: &str = include_str!("../fixtures/list_item_inset.lx");
const DIALOG: &str = include_str!("../fixtures/dialog.lx");
const SPLIT: &str = include_str!("../fixtures/split.lx");
const MENU: &str = include_str!("../fixtures/menu.lx");
const SURFACES: &str = include_str!("../fixtures/surfaces.lx");
const TOGGLES: &str = include_str!("../fixtures/toggles.lx");
const SLIDER_TOOLTIP: &str = include_str!("../fixtures/slider_tooltip.lx");
const NAV: &str = include_str!("../fixtures/nav.lx");
const BADGE: &str = include_str!("../fixtures/badge.lx");
const HEADING_PROGRESS: &str = include_str!("../fixtures/heading_progress.lx");
const SCROLL: &str = include_str!("../fixtures/scroll.lx");
const MARKDOWN: &str = include_str!("../fixtures/markdown.lx");

fn interp_tree_src(src: &str, state: &TestState, registry: &libra_markup::ComponentRegistry) -> libra_components::ViewNode {
    let ast = libra_markup::parse(src).expect("fixture parses");
    let field = |p: &str| state.field(p);
    let has_action = |m: &str| TestAction::action_by_name(m).is_some();
    let has_str_action = |m: &str| TestAction::action_with_str(m, String::new()).is_some();
    let collection_rows = |p: &str| state.collection_rows(p);
    let has_frame_info_action =
        |m: &str| TestAction::action_with_frame_info(m, FrameInfo::at(0.0)).is_some();
    let has_instant_action =
        |m: &str| TestAction::action_with_instant(m, libra_core::Instant::at(0.0)).is_some();
    let scope = libra_interp::Scope {
        field: &field,
        has_action: &has_action,
        has_str_action: &has_str_action,
        collection_rows: &collection_rows,
        has_frame_info_action: &has_frame_info_action,
        has_instant_action: &has_instant_action,
        binding_ctx: "State",
    };
    libra_interp::resolve(&ast, &scope, registry).expect("fixture resolves")
}

fn fixture_registry() -> libra_markup::ComponentRegistry {
    libra_markup::ComponentRegistry::load_from_sources(&[
        (
            "metric_card.lx".into(),
            include_str!("../fixtures/metric_card.lx").into(),
        ),
        (
            "card_shell.lx".into(),
            include_str!("../fixtures/card_shell.lx").into(),
        ),
    ])
    .expect("fixture registry")
}

fn state_variants() -> Vec<TestState> {
    vec![
        TestState::default(),
        TestState { count: 42, name: "Ada Lovelace".into(), status: "live".into(), ratio: 0.0, ..TestState::default() },
        TestState { count: -7, name: "🎉 unicode näme".into(), status: "degraded · 3 warnings".into(), ratio: 1.0, ..TestState::default() },
        TestState {
            count: i64::MAX,
            name: "with \"quotes\" and \\ backslashes".into(),
            status: String::new(),
            s0: SeriesHandle::new(SeriesId(9), 500.0),
            ratio: 0.333,
            ..TestState::default()
        },
    ]
}

#[test]
fn interp_and_codegen_produce_identical_trees() {
    for (i, state) in state_variants().into_iter().enumerate() {
        let interp = interp_tree_src(KITCHEN_SINK, &state, &fixture_registry());
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
    let collection_rows = |_: &str| None;
    let has_frame_info_action = |_: &str| false;
    let has_instant_action = |_: &str| false;
    let scope = libra_interp::Scope {
        field: &field,
        has_action: &has_action,
        has_str_action: &has_str_action,
        collection_rows: &collection_rows,
        has_frame_info_action: &has_frame_info_action,
        has_instant_action: &has_instant_action,
        binding_ctx: "State",
    };
    let interp_err = libra_interp::resolve(&ast, &scope, &libra_markup::ComponentRegistry::new()).unwrap_err();
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
        let interp = interp_tree_src(FOR_LOOP, &state, &fixture_registry());
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
        let interp = interp_tree_src(FOR_ACTIONS, &state, &fixture_registry());
        let compiled = generated::build_for_actions(&state);
        assert_eq!(
            interp, compiled,
            "engine drift on for_actions fixture with {} series (variant #{i})",
            state.series.len()
        );
    }
}

/// §6.2 item fields — `{t.label}`, `key={t.id}`, `onPress={nameChanged(t.id)}`
/// inside a `<for>` over a structured `Vec<Item>`. Interp resolves via
/// `collection_rows`; codegen emits `t.label` / `t.id` field access.
/// Both engines must agree across empty / one / several items.
#[test]
fn for_item_fields_produce_identical_trees() {
    let variants = [
        Vec::<Item>::new(),
        vec![Item { id: "7".into(), label: "Solo".into(), active: false }],
        vec![
            Item { id: "1".into(), label: "Alpha".into(), active: true },
            Item { id: "2".into(), label: "Beta".into(), active: false },
            Item { id: "3".into(), label: "Gamma".into(), active: false },
        ],
    ];
    for (i, items) in variants.into_iter().enumerate() {
        let state = TestState { items, ..TestState::default() };
        let interp = interp_tree_src(FOR_ITEM_FIELDS, &state, &fixture_registry());
        let compiled = generated::build_for_item_fields(&state);
        assert_eq!(
            interp, compiled,
            "engine drift on for_item_fields fixture with {} items (variant #{i})",
            state.items.len()
        );
    }
}

/// §6.2 `<if test={running}>` — both engines must agree on then vs else
/// trees, including a nested `<Frames>` that only mounts when true.
#[test]
fn if_else_produces_identical_trees_for_both_branches() {
    use libra_components::ViewNode;
    for running in [true, false] {
        let state = TestState { running, ..TestState::default() };
        let interp = interp_tree_src(IF_ELSE, &state, &fixture_registry());
        let compiled = generated::build_if_else(&state);
        assert_eq!(
            interp, compiled,
            "engine drift on if_else fixture with running={running}"
        );
        let ViewNode::Column { children, .. } = &interp else { panic!("{interp:?}") };
        if running {
            assert!(
                children.iter().any(|c| matches!(c, ViewNode::SourceFrames { .. })),
                "running → Frames mounted: {interp:?}"
            );
        } else {
            assert!(
                children.iter().all(|c| !matches!(c, ViewNode::SourceFrames { .. })),
                "paused → Frames absent: {interp:?}"
            );
        }
    }
}

/// §12.5's `<Frames onTick={frame}>` — interp's runtime resolution
/// (via ReflectAction::action_with_frame_info) and codegen's typed
/// `fn(FrameInfo) -> Action` coercion must agree on the exact same
/// ViewNode::SourceFrames { on_tick } value.
#[test]
fn frames_source_element_produces_identical_trees() {
    let state = TestState::default();
    let interp = interp_tree_src(FRAMES, &state, &fixture_registry());
    let compiled = generated::build_frames(&state);
    assert_eq!(interp, compiled, "engine drift on frames fixture — one grammar, two engines (§6.4)");
    assert!(
        matches!(&interp, libra_components::ViewNode::Column { children, .. }
            if children.contains(&libra_components::ViewNode::SourceFrames { on_tick: "frame".into() })),
        "expected a mounted Frames source in the tree: {interp:?}"
    );
}

/// §12.5 Interval + Stream — Instant-payload ticks and String-payload
/// stream events; both engines must agree on the Source* nodes.
#[test]
fn interval_and_stream_produce_identical_trees() {
    use libra_components::ViewNode;
    let state = TestState::default();
    let interp = interp_tree_src(INTERVAL_STREAM, &state, &fixture_registry());
    let compiled = generated::build_interval_stream(&state);
    assert_eq!(
        interp, compiled,
        "engine drift on interval_stream fixture — one grammar, two engines (§6.4)"
    );
    let ViewNode::Column { children, .. } = &interp else { panic!("{interp:?}") };
    assert!(
        children.contains(&ViewNode::SourceInterval { every_ms: 1000, on_tick: "poll".into() }),
        "{interp:?}"
    );
    assert!(
        children.contains(&ViewNode::SourceStream {
            id: "telemetry".into(),
            on_event: "packet".into()
        }),
        "{interp:?}"
    );
}

/// PLAN-ACCESSORS — `{user.name}`, `{items.len()}`, `test={items.isEmpty}` /
/// `test={user.active}`; both engines must agree.
#[test]
fn accessors_produce_identical_trees() {
    use libra_components::ViewNode;

    let variants = [
        TestState::default(),
        TestState {
            items: vec![],
            name: String::new(),
            user: User {
                name: "Grace".into(),
                active: false,
            },
            ..TestState::default()
        },
        TestState {
            items: vec![Item {
                id: "9".into(),
                label: "Zed".into(),
                active: true,
            }],
            name: "x".into(),
            user: User {
                name: "Lin".into(),
                active: true,
            },
            ..TestState::default()
        },
    ];
    for (i, state) in variants.iter().enumerate() {
        let interp = interp_tree_src(ACCESSORS, state, &fixture_registry());
        let compiled = generated::build_accessors(state);
        assert_eq!(
            interp, compiled,
            "engine drift on accessors fixture (variant #{i})"
        );
        let ViewNode::Column { children, .. } = &interp else {
            panic!("{interp:?}")
        };
        assert!(
            children.iter().any(|c| {
                matches!(c, ViewNode::Text { content, .. } if content == &state.user.name)
            }),
            "{interp:?}"
        );
        let expect_empty = state.items.is_empty();
        let has_empty = children.iter().any(|c| {
            matches!(c, ViewNode::Text { test_id: Some(id), .. } if id == "empty-branch")
        });
        let has_full = children.iter().any(|c| {
            matches!(c, ViewNode::Text { test_id: Some(id), .. } if id == "full-branch")
        });
        assert_eq!(has_empty, expect_empty, "variant #{i}: {interp:?}");
        assert_eq!(has_full, !expect_empty, "variant #{i}: {interp:?}");
    }
}

/// PLAN-CATALOG wave 1 — `<Spacer>` (fixed / spring), `<Separator>`
/// (horizontal / vertical), and `width={px}` pane sizing on
/// Column/Row; both engines must agree node for node.
#[test]
fn chrome_produces_identical_trees() {
    use libra_components::ViewNode;

    for (i, state) in state_variants().into_iter().enumerate() {
        let interp = interp_tree_src(CHROME, &state, &fixture_registry());
        let compiled = generated::build_chrome(&state);
        assert_eq!(interp, compiled, "engine drift on chrome fixture (variant #{i})");
    }

    // Shape assertions on the resolved tree: the spring spacer, the
    // fixed-width sidebar, and both separator orientations survive
    // resolution as data.
    let interp = interp_tree_src(CHROME, &TestState::default(), &fixture_registry());
    let ViewNode::Column { children, .. } = &interp else { panic!("{interp:?}") };
    assert!(
        matches!(&children[1], ViewNode::Separator { vertical: false }),
        "{:?}",
        children[1]
    );
    let ViewNode::Row { children: panes, .. } = &children[2] else { panic!("{:?}", children[2]) };
    let ViewNode::Column { style, children: sidebar } = &panes[0] else { panic!("{:?}", panes[0]) };
    assert_eq!(style.width, Some(180.0));
    assert!(matches!(&sidebar[1], ViewNode::Spacer { size: None }), "{:?}", sidebar[1]);
    assert!(matches!(&panes[1], ViewNode::Separator { vertical: true }), "{:?}", panes[1]);
    let ViewNode::Column { children: content, .. } = &panes[2] else { panic!("{:?}", panes[2]) };
    assert!(matches!(&content[1], ViewNode::Spacer { size: Some(s) } if *s == 24.0), "{:?}", content[1]);
    // Weighted grow survives resolution in both panes.
    let ViewNode::Column { style: s15, .. } = &panes[2] else { unreachable!() };
    assert_eq!(s15.grow, 1.5);
    let ViewNode::Column { style: s25, .. } = &panes[3] else { panic!("{:?}", panes[3]) };
    assert_eq!(s25.grow, 2.5);
}

/// PLAN-CATALOG wave 2 — `<Badge tone="…">` status pills. Non-interactive
/// child-text element with an enum `tone` attr; both engines must resolve
/// the same `ViewNode::Badge { label, tone, test_id }` node for node.
#[test]
fn badge_produces_identical_trees() {
    use libra_components::{Tone, ViewNode};

    for (i, state) in state_variants().into_iter().enumerate() {
        let interp = interp_tree_src(BADGE, &state, &fixture_registry());
        let compiled = generated::build_badge(&state);
        assert_eq!(interp, compiled, "engine drift on badge fixture (variant #{i})");
    }

    let interp = interp_tree_src(BADGE, &TestState::default(), &fixture_registry());
    let ViewNode::Column { children, .. } = &interp else { panic!("{interp:?}") };
    // Default tone, then an explicit success badge carrying a testId.
    assert!(
        matches!(&children[0], ViewNode::Badge { tone: Tone::Neutral, label, .. } if label == "neutral"),
        "{:?}",
        children[0]
    );
    assert!(
        matches!(&children[1], ViewNode::Badge { tone: Tone::Success, test_id: Some(id), label }
            if id == "ok" && label == "Online"),
        "{:?}",
        children[1]
    );
}

/// PLAN-CATALOG wave 2 — `<Heading level={n}>` (n → type ramp, clamped 1..4)
/// and `<Progress value={f}>`. Both engines must resolve the same nodes.
#[test]
fn heading_progress_produce_identical_trees() {
    use libra_components::ViewNode;

    for (i, state) in state_variants().into_iter().enumerate() {
        let interp = interp_tree_src(HEADING_PROGRESS, &state, &fixture_registry());
        let compiled = generated::build_heading_progress(&state);
        assert_eq!(interp, compiled, "engine drift on heading_progress fixture (variant #{i})");
    }

    let state = TestState { ratio: 0.42, ..TestState::default() };
    let interp = interp_tree_src(HEADING_PROGRESS, &state, &fixture_registry());
    let ViewNode::Column { children, .. } = &interp else { panic!("{interp:?}") };
    assert!(
        matches!(&children[1], ViewNode::Heading { level: 2, test_id: Some(id), content }
            if id == "h2" && content == "Section"),
        "{:?}",
        children[1]
    );
    // Bare <Heading> defaults to level 1.
    assert!(matches!(&children[3], ViewNode::Heading { level: 1, .. }), "{:?}", children[3]);
    assert!(
        matches!(&children[5], ViewNode::Progress { value, test_id: Some(id) }
            if (*value - 0.6).abs() < 1e-6 && id == "p60"),
        "{:?}",
        children[5]
    );
    // The **bound** Progress reflects `state.ratio` — numeric binding (§26 #8):
    // interp parses the field's Display, codegen accesses it typed, same f32.
    assert!(
        matches!(&children[7], ViewNode::Progress { value, test_id: Some(id) }
            if *value == state.ratio && id == "bound"),
        "{:?}",
        children[7]
    );
}

/// PLAN-SCROLL — `<Scroll testId height?>` viewport (fixed and grown); both
/// engines must resolve the same `ViewNode::Scroll` node for node.
#[test]
fn scroll_produces_identical_trees() {
    use libra_components::ViewNode;

    for (i, state) in state_variants().into_iter().enumerate() {
        let interp = interp_tree_src(SCROLL, &state, &fixture_registry());
        let compiled = generated::build_scroll(&state);
        assert_eq!(interp, compiled, "engine drift on scroll fixture (variant #{i})");
    }

    let interp = interp_tree_src(SCROLL, &TestState::default(), &fixture_registry());
    let ViewNode::Column { children, .. } = &interp else { panic!("{interp:?}") };
    assert!(
        matches!(&children[0], ViewNode::Scroll { height: Some(h), test_id, children }
            if *h == 120.0 && test_id == "list" && children.len() == 2),
        "{:?}",
        children[0]
    );
    assert!(
        matches!(&children[1], ViewNode::Scroll { height: None, test_id, .. } if test_id == "grow"),
        "{:?}",
        children[1]
    );
}

/// PLAN-CATALOG `<ListItem>` + PLAN-TEXTAREA `<Textarea>` — loop-bound
/// text/bool attrs, static rows, and the controlled multi-line field;
/// both engines must agree.
#[test]
fn list_textarea_produces_identical_trees() {
    use libra_components::ViewNode;

    for (i, state) in state_variants().into_iter().enumerate() {
        let interp = interp_tree_src(LIST_TEXTAREA, &state, &fixture_registry());
        let compiled = generated::build_list_textarea(&state);
        assert_eq!(interp, compiled, "engine drift on list_textarea fixture (variant #{i})");
    }

    let state = TestState::default();
    let interp = interp_tree_src(LIST_TEXTAREA, &state, &fixture_registry());
    let ViewNode::Column { children, .. } = &interp else { panic!("{interp:?}") };
    // Two loop rows: `selected` mirrors each item's `active` bool, the
    // payload carries that row's id.
    let ViewNode::ListItem { title, selected, on_press_arg, meta, .. } = &children[0] else {
        panic!("{:?}", children[0])
    };
    assert_eq!((title.as_str(), *selected), ("Alpha", true));
    assert_eq!(on_press_arg.as_deref(), Some("1"));
    assert_eq!(meta, "1");
    let ViewNode::ListItem { selected, .. } = &children[1] else { panic!("{:?}", children[1]) };
    assert!(!selected, "Beta is inactive");
    // The static row: literal title, bound meta, flag-selected.
    let ViewNode::ListItem { title, meta, selected, test_id, .. } = &children[2] else {
        panic!("{:?}", children[2])
    };
    assert_eq!((title.as_str(), meta.as_str(), *selected), ("static row", "ok", true));
    assert_eq!(test_id.as_deref(), Some("static-row"));
    assert!(
        matches!(&children[3], ViewNode::Textarea { test_id, .. } if test_id == "body"),
        "{:?}",
        children[3]
    );
}

/// Display-only `<Markdown source>` — both engines keep the source node.
#[test]
fn markdown_produces_identical_trees() {
    use libra_components::ViewNode;

    for (i, state) in state_variants().into_iter().enumerate() {
        let interp = interp_tree_src(MARKDOWN, &state, &fixture_registry());
        let compiled = generated::build_markdown(&state);
        assert_eq!(interp, compiled, "engine drift on markdown fixture (variant #{i})");
    }

    let state = TestState { name: "# Hi\n\npara".into(), status: "Hi".into(), ..TestState::default() };
    let interp = interp_tree_src(MARKDOWN, &state, &fixture_registry());
    let ViewNode::Column { children, .. } = &interp else { panic!("{interp:?}") };
    assert!(
        matches!(
            &children[0],
            ViewNode::Markdown { source, skip_title, test_id: Some(id), .. }
                if source == "# Hi" && skip_title.is_empty() && id == "literal"
        ),
        "{:?}",
        children[0]
    );
    assert!(
        matches!(
            &children[1],
            ViewNode::Markdown { source, skip_title, test_id: Some(id), .. }
                if source == "# Hi\n\npara" && skip_title == "Hi" && id == "bound"
        ),
        "{:?}",
        children[1]
    );
}

/// `<ListItem inset>` — literal px or a bound f32; both engines agree.
#[test]
fn list_item_inset_produces_identical_trees() {
    use libra_components::ViewNode;

    for (i, state) in state_variants().into_iter().enumerate() {
        let interp = interp_tree_src(LIST_ITEM_INSET, &state, &fixture_registry());
        let compiled = generated::build_list_item_inset(&state);
        assert_eq!(interp, compiled, "engine drift on list_item_inset fixture (variant #{i})");
    }

    let state = TestState::default();
    let interp = interp_tree_src(LIST_ITEM_INSET, &state, &fixture_registry());
    let ViewNode::Column { children, .. } = &interp else { panic!("{interp:?}") };
    let ViewNode::ListItem { inset, test_id, .. } = &children[0] else {
        panic!("{:?}", children[0])
    };
    assert_eq!((*inset, test_id.as_deref()), (16.0, Some("literal-inset")));
    let ViewNode::ListItem { inset, test_id, .. } = &children[1] else {
        panic!("{:?}", children[1])
    };
    assert_eq!((*inset, test_id.as_deref()), (16.0, Some("bound-inset")));
}

/// PLAN-CATALOG `<Dialog>` (M6) — the in-tree modal: title as literal
/// and binding, optional onDismiss, `<if>`-mounted; both engines agree.
#[test]
fn dialog_produces_identical_trees() {
    use libra_components::ViewNode;

    for (i, state) in state_variants().into_iter().enumerate() {
        let interp = interp_tree_src(DIALOG, &state, &fixture_registry());
        let compiled = generated::build_dialog(&state);
        assert_eq!(interp, compiled, "engine drift on dialog fixture (variant #{i})");
    }

    // running=true mounts the bound-title dialog; the static one is
    // always there. running=false drops the first (the `<if>` rule).
    let on = interp_tree_src(DIALOG, &TestState::default(), &fixture_registry());
    let ViewNode::Column { children, .. } = &on else { panic!("{on:?}") };
    let dialogs: Vec<_> =
        children.iter().filter(|c| matches!(c, ViewNode::Dialog { .. })).collect();
    assert_eq!(dialogs.len(), 2);
    let ViewNode::Dialog { title, on_dismiss, test_id, children: kids } = dialogs[0] else {
        unreachable!()
    };
    assert_eq!(title, &TestState::default().name);
    assert_eq!(on_dismiss.as_deref(), Some("reset"));
    assert_eq!(test_id.as_deref(), Some("the-dialog"));
    assert_eq!(kids.len(), 3);
    let ViewNode::Dialog { title, on_dismiss, .. } = dialogs[1] else { unreachable!() };
    assert_eq!((title.as_str(), on_dismiss.is_none()), ("static title", true));

    let off = interp_tree_src(DIALOG, &TestState { running: false, ..TestState::default() }, &fixture_registry());
    let ViewNode::Column { children, .. } = &off else { panic!("{off:?}") };
    assert_eq!(
        children.iter().filter(|c| matches!(c, ViewNode::Dialog { .. })).count(),
        1,
        "unmounted <if> drops its dialog"
    );
}

/// PLAN-CATALOG `<Split>` (M5) — typed fraction (bound + literal),
/// String-payload onResize, two-pane arity; both engines agree.
#[test]
fn split_produces_identical_trees() {
    use libra_components::ViewNode;

    for (i, state) in state_variants().into_iter().enumerate() {
        let interp = interp_tree_src(SPLIT, &state, &fixture_registry());
        let compiled = generated::build_split(&state);
        assert_eq!(interp, compiled, "engine drift on split fixture (variant #{i})");
    }

    let state = TestState { ratio: 0.42, ..TestState::default() };
    let interp = interp_tree_src(SPLIT, &state, &fixture_registry());
    let ViewNode::Column { children, .. } = &interp else { panic!("{interp:?}") };
    let ViewNode::Split { fraction, on_resize, test_id, children: panes } = &children[0] else {
        panic!("{:?}", children[0])
    };
    assert!((fraction - 0.42).abs() < 1e-6, "bound fraction resolves typed");
    assert_eq!((on_resize.as_str(), test_id.as_str()), ("nameChanged", "main"));
    assert_eq!(panes.len(), 2);
    let ViewNode::Split { fraction, .. } = &children[1] else { panic!("{:?}", children[1]) };
    assert!((fraction - 0.3).abs() < 1e-6, "literal fraction");
}

/// PLAN-CATALOG `<ContextMenu>` + ListItem `onMenu` (M7) — the anchored
/// overlay, right-click handlers with item-scoped args, `<if>`-mounted;
/// both engines agree.
#[test]
fn menu_produces_identical_trees() {
    use libra_components::ViewNode;

    for (i, state) in state_variants().into_iter().enumerate() {
        let interp = interp_tree_src(MENU, &state, &fixture_registry());
        let compiled = generated::build_menu(&state);
        assert_eq!(interp, compiled, "engine drift on menu fixture (variant #{i})");
    }

    let interp = interp_tree_src(MENU, &TestState::default(), &fixture_registry());
    let ViewNode::Column { children, .. } = &interp else { panic!("{interp:?}") };
    let ViewNode::ListItem { on_menu, on_menu_arg, .. } = &children[0] else {
        panic!("{:?}", children[0])
    };
    assert_eq!(on_menu.as_deref(), Some("packet"));
    assert_eq!(on_menu_arg.as_deref(), Some("1"), "item-scoped arg is the row id");
    let ViewNode::ListItem { on_menu, .. } = &children[2] else { panic!("{:?}", children[2]) };
    assert!(on_menu.is_none(), "onMenu is optional");
    let ViewNode::ContextMenu { on_dismiss, test_id, children: items } = &children[3] else {
        panic!("{:?}", children[3])
    };
    assert_eq!((on_dismiss.as_deref(), test_id.as_str()), (Some("reset"), "row-menu"));
    assert_eq!(items.len(), 2);
}

/// PLAN-CATALOG parity wave — Alert/Card/Panel/Skeleton/Avatar/Drawer:
/// bound + literal attrs, `<if>`-mounted drawer; both engines agree.
#[test]
fn surfaces_produce_identical_trees() {
    use libra_components::ViewNode;

    for (i, state) in state_variants().into_iter().enumerate() {
        let interp = interp_tree_src(SURFACES, &state, &fixture_registry());
        let compiled = generated::build_surfaces(&state);
        assert_eq!(interp, compiled, "engine drift on surfaces fixture (variant #{i})");
    }

    let state = TestState::default();
    let interp = interp_tree_src(SURFACES, &state, &fixture_registry());
    let ViewNode::Column { children, .. } = &interp else { panic!("{interp:?}") };
    let ViewNode::Alert { title, body, .. } = &children[0] else { panic!("{:?}", children[0]) };
    assert_eq!((title.as_str(), body.as_str()), ("", "something broke"));
    assert!(matches!(&children[2], ViewNode::Card { .. }));
    assert!(matches!(&children[3], ViewNode::Panel { raised: false, .. }));
    assert!(matches!(&children[4], ViewNode::Panel { raised: true, .. }));
    assert!(matches!(&children[5], ViewNode::Skeleton { width: None, .. }));
    let ViewNode::Avatar { name, .. } = &children[7] else { panic!("{:?}", children[7]) };
    assert_eq!(name, "Ada");
    assert!(matches!(
        &children[9],
        ViewNode::Drawer { side: libra_components::DrawerSide::Right, .. }
    ));
}

/// PLAN-CATALOG parity wave — Switch/Checkbox/Radio: bool bindings
/// under the Display-"true" rule, `<for>` radio groups with item-scoped
/// payloads; both engines agree.
#[test]
fn toggles_produce_identical_trees() {
    use libra_components::ViewNode;

    for (i, state) in state_variants().into_iter().enumerate() {
        let interp = interp_tree_src(TOGGLES, &state, &fixture_registry());
        let compiled = generated::build_toggles(&state);
        assert_eq!(interp, compiled, "engine drift on toggles fixture (variant #{i})");
    }

    let state = TestState::default();
    let interp = interp_tree_src(TOGGLES, &state, &fixture_registry());
    let ViewNode::Column { children, .. } = &interp else { panic!("{interp:?}") };
    assert!(matches!(&children[0], ViewNode::Switch { checked: true, .. }), "running=true binds");
    assert!(matches!(&children[2], ViewNode::Checkbox { checked: true, .. }), "user.active binds");
    assert!(matches!(&children[3], ViewNode::Checkbox { checked: false, .. }));
    // The <for> radio group: Alpha active, Beta not; args carry row ids.
    let ViewNode::Radio { selected, on_select_arg, label, .. } = &children[4] else {
        panic!("{:?}", children[4])
    };
    assert_eq!((*selected, on_select_arg.as_deref(), label.as_str()), (true, Some("1"), "Alpha"));
    let ViewNode::Radio { selected, .. } = &children[5] else { panic!("{:?}", children[5]) };
    assert!(!selected);
}

/// PLAN-CATALOG — Slider (typed value, model-owned) + Tooltip (hover
/// label wrapper); both engines agree.
#[test]
fn slider_tooltip_produce_identical_trees() {
    use libra_components::ViewNode;

    for (i, state) in state_variants().into_iter().enumerate() {
        let interp = interp_tree_src(SLIDER_TOOLTIP, &state, &fixture_registry());
        let compiled = generated::build_slider_tooltip(&state);
        assert_eq!(interp, compiled, "engine drift on slider_tooltip fixture (variant #{i})");
    }

    let state = TestState { ratio: 0.7, ..TestState::default() };
    let interp = interp_tree_src(SLIDER_TOOLTIP, &state, &fixture_registry());
    let ViewNode::Column { children, .. } = &interp else { panic!("{interp:?}") };
    let ViewNode::Slider { value, test_id, .. } = &children[0] else { panic!("{:?}", children[0]) };
    assert!((value - 0.7).abs() < 1e-6);
    assert_eq!(test_id, "bound-slider");
    let ViewNode::Tooltip { text, children: kids } = &children[2] else { panic!("{:?}", children[2]) };
    assert_eq!(text, "ok");
    assert_eq!(kids.len(), 1);
}

/// PLAN-CATALOG nav wave — Breadcrumb/Pagination/Stepper/Timeline:
/// item-scoped Breadcrumb press, Pagination's typed current/total,
/// Stepper's index-derived status, Timeline's direct `done` attr; both
/// engines agree.
#[test]
fn nav_produces_identical_trees() {
    use libra_components::ViewNode;

    // count doubles as Pagination.current / Stepper.current across states.
    let variants = [
        TestState::default(),
        TestState { page: 1, name: "Ada".into(), status: "Shipped".into(), ..TestState::default() },
        TestState { page: 2, name: "Grace".into(), status: "Delivered".into(), ..TestState::default() },
    ];
    for (i, state) in variants.iter().enumerate() {
        let interp = interp_tree_src(NAV, state, &fixture_registry());
        let compiled = generated::build_nav(state);
        assert_eq!(interp, compiled, "engine drift on nav fixture (variant #{i})");
    }

    let state = TestState { page: 1, name: "Ada".into(), ..TestState::default() };
    let interp = interp_tree_src(NAV, &state, &fixture_registry());
    let ViewNode::Column { children, .. } = &interp else { panic!("{interp:?}") };

    let ViewNode::Breadcrumb { children: crumbs } = &children[0] else { panic!("{:?}", children[0]) };
    assert_eq!(crumbs.len(), 3);
    let ViewNode::BreadcrumbItem { label, current, on_press, .. } = &crumbs[1] else {
        panic!("{:?}", crumbs[1])
    };
    assert_eq!((label.as_str(), *current, on_press.as_deref()), ("Ada", false, Some("reset")));
    let ViewNode::BreadcrumbItem { current, on_press, .. } = &crumbs[2] else { panic!("{:?}", crumbs[2]) };
    assert!(*current && on_press.is_none(), "the trailing crumb is non-pressable");

    let ViewNode::Pagination { current, total, .. } = &children[1] else { panic!("{:?}", children[1]) };
    assert_eq!((*current, *total), (1, 10));

    let ViewNode::Stepper { current, children: steps } = &children[2] else { panic!("{:?}", children[2]) };
    assert_eq!(*current, 1);
    assert_eq!(steps.len(), 3);

    let ViewNode::Timeline { children: entries } = &children[3] else { panic!("{:?}", children[3]) };
    let ViewNode::TimelineItem { title, done, .. } = &entries[0] else { panic!("{:?}", entries[0]) };
    assert_eq!((title.as_str(), *done), ("Order placed", true));
    let ViewNode::TimelineItem { title, done, .. } = &entries[1] else { panic!("{:?}", entries[1]) };
    assert_eq!((title.as_str(), *done), ("ok", false));
}

const SEARCH: &str = include_str!("../fixtures/search.lx");
const CHIP: &str = include_str!("../fixtures/chip.lx");
const TABS: &str = include_str!("../fixtures/tabs.lx");
const TOAST: &str = include_str!("../fixtures/toast.lx");
const TOOLBAR: &str = include_str!("../fixtures/toolbar.lx");
const STATUSBAR: &str = include_str!("../fixtures/statusbar.lx");
const ICON: &str = include_str!("../fixtures/icon.lx");
const COMPOSED: &str = include_str!("../fixtures/composed.lx");
const COMPOSED_INLINED: &str = include_str!("../fixtures/composed_inlined.lx");
const SLOT_HOST: &str = include_str!("../fixtures/slot_host.lx");

/// PLAN-CATALOG wave 3 — `<Search>`: Input-shaped value/onChange/testId
/// plus optional unit `onSubmit`. Both engines must agree.
#[test]
fn search_produces_identical_trees() {
    use libra_components::ViewNode;

    for (i, state) in state_variants().into_iter().enumerate() {
        let interp = interp_tree_src(SEARCH, &state, &fixture_registry());
        let compiled = generated::build_search(&state);
        assert_eq!(interp, compiled, "engine drift on search fixture (variant #{i})");
    }

    let state = TestState { name: "Ada".into(), ..TestState::default() };
    let interp = interp_tree_src(SEARCH, &state, &fixture_registry());
    let ViewNode::Column { children, .. } = &interp else { panic!("{interp:?}") };
    assert!(
        matches!(
            &children[0],
            ViewNode::Search {
                value,
                on_change,
                on_submit: Some(sub),
                test_id,
                ..
            } if value == "Ada" && on_change == "nameChanged" && sub == "reset" && test_id == "bound-search"
        ),
        "{:?}",
        children[0]
    );
    assert!(
        matches!(&children[1], ViewNode::Search { on_submit: None, test_id, .. } if test_id == "live-search"),
        "{:?}",
        children[1]
    );
}

/// PLAN-CATALOG wave 5 — `<Chip>`: selected bool, optional onPress /
/// onRemove (item-scoped). Both engines must agree.
#[test]
fn chip_produces_identical_trees() {
    use libra_components::ViewNode;

    for (i, state) in state_variants().into_iter().enumerate() {
        let interp = interp_tree_src(CHIP, &state, &fixture_registry());
        let compiled = generated::build_chip(&state);
        assert_eq!(interp, compiled, "engine drift on chip fixture (variant #{i})");
    }

    let state = TestState::default();
    let interp = interp_tree_src(CHIP, &state, &fixture_registry());
    let ViewNode::Column { children, .. } = &interp else { panic!("{interp:?}") };
    assert!(
        matches!(
            &children[0],
            ViewNode::Chip {
                label,
                selected: true,
                on_press: Some(p),
                on_remove: None,
                test_id: Some(id),
                ..
            } if label == "Ada" && p == "reset" && id == "bound-chip"
        ),
        "{:?}",
        children[0]
    );
    assert!(
        matches!(&children[1], ViewNode::Chip { selected: true, label, .. } if label == "On"),
        "{:?}",
        children[1]
    );
    assert!(
        matches!(&children[3], ViewNode::Chip { on_remove: Some(r), test_id: Some(id), .. } if r == "reset" && id == "tag"),
        "{:?}",
        children[3]
    );
    // <for> first row: Alpha is active; press/remove args carry the id.
    let ViewNode::Chip { selected, on_press_arg, on_remove_arg, label, .. } = &children[4] else {
        panic!("{:?}", children[4])
    };
    assert_eq!(
        (*selected, on_press_arg.as_deref(), on_remove_arg.as_deref(), label.as_str()),
        (true, Some("1"), Some("1"), "Alpha")
    );
}

/// PLAN-CATALOG wave 7 — `<Tabs>` / `<Tab>`: selected bool, optional
/// onPress / onClose (item-scoped), `<for>` children. Both engines
/// must agree. Gloss note tabs use this exact shape.
#[test]
fn tabs_produce_identical_trees() {
    use libra_components::ViewNode;

    for (i, state) in state_variants().into_iter().enumerate() {
        let interp = interp_tree_src(TABS, &state, &fixture_registry());
        let compiled = generated::build_tabs(&state);
        assert_eq!(interp, compiled, "engine drift on tabs fixture (variant #{i})");
    }

    let state = TestState::default();
    let interp = interp_tree_src(TABS, &state, &fixture_registry());
    let ViewNode::Column { children, .. } = &interp else { panic!("{interp:?}") };
    let ViewNode::Tabs { children: tabs, test_id } = &children[0] else { panic!("{:?}", children[0]) };
    assert_eq!(test_id.as_deref(), Some("strip"));
    assert!(
        matches!(
            &tabs[0],
            ViewNode::Tab {
                label,
                selected: true,
                on_press: Some(p),
                on_close: None,
                test_id: Some(id),
                ..
            } if label == "Ada" && p == "reset" && id == "bound-tab"
        ),
        "{:?}",
        tabs[0]
    );
    assert!(
        matches!(&tabs[1], ViewNode::Tab { selected: true, label, .. } if label == "On"),
        "{:?}",
        tabs[1]
    );
    assert!(
        matches!(&tabs[3], ViewNode::Tab { on_close: Some(c), test_id: Some(id), .. } if c == "reset" && id == "scratch"),
        "{:?}",
        tabs[3]
    );
    let ViewNode::Tab { selected, on_press_arg, on_close_arg, label, .. } = &tabs[4] else {
        panic!("{:?}", tabs[4])
    };
    assert_eq!(
        (*selected, on_press_arg.as_deref(), on_close_arg.as_deref(), label.as_str()),
        (true, Some("1"), Some("1"), "Alpha")
    );
}

/// PLAN-CATALOG wave 7 — `<Toast>`: tone, optional title, body, optional
/// unit onDismiss. Both engines must agree.
#[test]
fn toast_produces_identical_trees() {
    use libra_components::{Tone, ViewNode};

    for (i, state) in state_variants().into_iter().enumerate() {
        let interp = interp_tree_src(TOAST, &state, &fixture_registry());
        let compiled = generated::build_toast(&state);
        assert_eq!(interp, compiled, "engine drift on toast fixture (variant #{i})");
    }

    let state = TestState::default();
    let interp = interp_tree_src(TOAST, &state, &fixture_registry());
    let ViewNode::Column { children, .. } = &interp else { panic!("{interp:?}") };
    assert!(
        matches!(
            &children[0],
            ViewNode::Toast {
                tone: Tone::Success,
                title,
                body,
                on_dismiss: Some(d),
                test_id: Some(id),
            } if title == "Ada" && body == "saved" && d == "reset" && id == "bound-toast"
        ),
        "{:?}",
        children[0]
    );
    assert!(
        matches!(
            &children[1],
            ViewNode::Toast { tone: Tone::Danger, title, on_dismiss: None, .. } if title == "Failed"
        ),
        "{:?}",
        children[1]
    );
    assert!(
        matches!(
            &children[2],
            ViewNode::Toast { title, body, on_dismiss: Some(d), test_id: Some(id), .. }
                if title.is_empty() && body == "Saved" && d == "increment" && id == "body-only"
        ),
        "{:?}",
        children[2]
    );
}

/// PLAN-CATALOG wave 7 — `<Toolbar>`: optional vertical, children.
/// Both engines must agree. Gloss rail / editor chrome use this.
#[test]
fn toolbar_produces_identical_trees() {
    use libra_components::ViewNode;

    for (i, state) in state_variants().into_iter().enumerate() {
        let interp = interp_tree_src(TOOLBAR, &state, &fixture_registry());
        let compiled = generated::build_toolbar(&state);
        assert_eq!(interp, compiled, "engine drift on toolbar fixture (variant #{i})");
    }

    let state = TestState::default();
    let interp = interp_tree_src(TOOLBAR, &state, &fixture_registry());
    let ViewNode::Column { children, .. } = &interp else { panic!("{interp:?}") };
    let ViewNode::Toolbar { vertical, children: items, test_id } = &children[0] else {
        panic!("{:?}", children[0])
    };
    assert_eq!((*vertical, test_id.as_deref(), items.len()), (false, Some("bar"), 2));
    let ViewNode::Toolbar { vertical, test_id, .. } = &children[1] else { panic!("{:?}", children[1]) };
    assert_eq!((*vertical, test_id.as_deref()), (true, Some("rail")));
}

/// PLAN-CATALOG wave 7 — `<StatusBar>`: footer row, children.
/// Both engines must agree. Gloss footer uses this.
#[test]
fn statusbar_produces_identical_trees() {
    use libra_components::ViewNode;

    for (i, state) in state_variants().into_iter().enumerate() {
        let interp = interp_tree_src(STATUSBAR, &state, &fixture_registry());
        let compiled = generated::build_statusbar(&state);
        assert_eq!(interp, compiled, "engine drift on statusbar fixture (variant #{i})");
    }

    let state = TestState::default();
    let interp = interp_tree_src(STATUSBAR, &state, &fixture_registry());
    let ViewNode::Column { children, .. } = &interp else { panic!("{interp:?}") };
    let ViewNode::StatusBar { children: items, test_id } = &children[0] else {
        panic!("{:?}", children[0])
    };
    assert_eq!(test_id.as_deref(), Some("bar"));
    assert_eq!(items.len(), 2);
    assert!(
        matches!(&items[0], ViewNode::Text { content, muted: true, test_id: Some(id), .. } if content == "ok" && id == "status"),
        "{:?}",
        items[0]
    );
}

/// PLAN-CATALOG — `<Icon>` closed set + `Button icon="…"`.
#[test]
fn icon_produces_identical_trees() {
    use libra_components::{IconName, ViewNode};

    for (i, state) in state_variants().into_iter().enumerate() {
        let interp = interp_tree_src(ICON, &state, &fixture_registry());
        let compiled = generated::build_icon(&state);
        assert_eq!(interp, compiled, "engine drift on icon fixture (variant #{i})");
    }

    let state = TestState::default();
    let interp = interp_tree_src(ICON, &state, &fixture_registry());
    let ViewNode::Column { children, .. } = &interp else { panic!("{interp:?}") };
    assert!(
        matches!(
            &children[0],
            ViewNode::Icon { name: IconName::Plus, muted: false, test_id: Some(id) } if id == "plus"
        ),
        "{:?}",
        children[0]
    );
    assert!(
        matches!(
            &children[1],
            ViewNode::Icon { name: IconName::Search, muted: true, test_id: Some(id) } if id == "search-icon"
        ),
        "{:?}",
        children[1]
    );
    assert!(
        matches!(
            &children[3],
            ViewNode::Button { icon: Some(IconName::Plus), label, test_id: Some(id), .. }
                if label.is_empty() && id == "icon-btn"
        ),
        "{:?}",
        children[3]
    );
    assert!(
        matches!(
            &children[4],
            ViewNode::Button { icon: Some(IconName::Search), label, .. } if label == "Find"
        ),
        "{:?}",
        children[4]
    );
}

#[test]
fn composed_produces_identical_trees() {
    let state = TestState::default();
    let interp = interp_tree_src(COMPOSED, &state, &fixture_registry());
    let compiled = generated::build_composed(&state);
    assert_eq!(interp, compiled, "composed fixture drift (§6.4 / §7.2)");
}

#[test]
fn composed_tree_equals_inlined() {
    let state = TestState::default();
    let composed = interp_tree_src(COMPOSED, &state, &fixture_registry());
    let inlined = interp_tree_src(COMPOSED_INLINED, &state, &fixture_registry());
    assert_eq!(composed, inlined, "composition must equal hand-inlined twin (PLAN-COMPOSE tenet 1)");
}

#[test]
fn slot_host_produces_identical_trees() {
    let state = TestState::default();
    let interp = interp_tree_src(SLOT_HOST, &state, &fixture_registry());
    let compiled = generated::build_slot_host(&state);
    assert_eq!(interp, compiled, "slot fixture drift (§7.2 C6)");
}
