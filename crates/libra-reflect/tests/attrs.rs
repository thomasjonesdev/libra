//! `#[libra::state]` / `#[libra::actions]` exercised against the real
//! libra-core traits (dev-dep cycle, permitted by cargo). Imported here
//! via their real path (`libra_reflect::{state, actions}`) since the
//! `libra` facade crate that makes `#[libra::state]` literally resolve
//! doesn't exist yet (PLAN-ERGONOMICS.md E1) — same macros, same
//! expansion, different import spelling.

use libra_core::{
    Effect, FrameInfo, HttpRequest, HttpResult, Instant, IntoEffects, ReflectAction, ReflectState,
};
use libra_reflect::{actions, state};

#[state]
struct CounterState {
    count: i64,
    #[libra(skip)]
    #[allow(dead_code)]
    quote: Option<String>,
    #[libra(skip)]
    last_frame_ms: f64,
}

#[actions]
impl CounterState {
    fn increment(&mut self) {
        self.count += 1;
    }

    fn decrement(&mut self) {
        self.count -= 1;
    }

    // Multi-word method name: proves snake_case -> PascalCase variant
    // -> camelCase public name all the way through.
    fn reset_count(&mut self) {
        self.count = 0;
    }

    // Single-String-arg: a handler dispatchable with a control payload
    // (§6.2's onChange={filterChanged(value)}).
    fn filter_changed(&mut self, value: String) {
        self.quote = Some(value);
    }

    // Fetches something; returns an effect.
    fn fetch(&mut self) -> Effect<Action> {
        Effect::http(HttpRequest::get("https://api.example.com/session"), Action::Fetched)
    }

    // Continuation: an effect-result payload (not String) — must not
    // be nameable from markup (§5.2's dispatchability rule).
    fn fetched(&mut self, result: HttpResult) {
        if let HttpResult::Response { body, .. } = result {
            self.quote = Some(String::from_utf8_lossy(&body).into_owned());
        }
    }

    // Single-FrameInfo-arg: §12.5's <Frames onTick={frame}> source
    // element — a handler, not a continuation, despite taking a
    // non-String payload.
    fn frame(&mut self, info: FrameInfo) {
        self.last_frame_ms = info.t_ms;
    }

    // Single-Instant-arg: §12.5's <Interval every={ms} onTick={poll}>
    // source element — same shape as `frame`, different payload type.
    fn poll(&mut self, instant: Instant) {
        self.last_frame_ms = instant.t_ms;
    }
}

#[test]
fn state_derives_are_bundled_and_reflect_state_works() {
    let s = CounterState { count: 3, quote: None, last_frame_ms: 0.0 };
    // Debug + Clone + PartialEq + Serialize/Deserialize all present:
    let s2 = s.clone();
    assert_eq!(s, s2);
    assert_eq!(format!("{s:?}").contains("CounterState"), true);
    let json = serde_json::to_string(&s).expect("serializes");
    let back: CounterState = serde_json::from_str(&json).expect("deserializes");
    assert_eq!(back.count, 3);

    // ReflectState: bindable field, skipped field excluded.
    assert_eq!(s.field("count").as_deref(), Some("3"));
    assert_eq!(s.field("quote"), None);
    assert_eq!(CounterState::field_names(), &["count"]);
}

#[test]
fn single_instant_arg_methods_are_handlers_for_interval_source_elements() {
    let instant = Instant::at(2500.0);
    assert_eq!(Action::action_with_instant("poll", instant), Some(Action::Poll(instant)));
    // Not nameable via the other two paths:
    assert_eq!(Action::action_by_name("poll"), None);
    assert_eq!(Action::action_with_str("poll", String::new()), None);
    // Nor the FrameInfo path — distinct payload type, distinct handler:
    assert_eq!(Action::action_with_frame_info("poll", FrameInfo::at(2500.0)), None);

    let mut s = CounterState { count: 0, quote: None, last_frame_ms: 0.0 };
    reduce(&mut s, Action::Poll(instant));
    assert_eq!(s.last_frame_ms, 2500.0);
}

#[state]
struct DashboardState {
    series: Vec<libra_core::SeriesHandle>,
}

#[test]
fn vec_fields_route_to_collection_not_field() {
    use libra_core::{SeriesHandle, SeriesId};
    let s = DashboardState {
        series: vec![
            SeriesHandle::new(SeriesId(0), 3000.0),
            SeriesHandle::new(SeriesId(1), 3000.0),
        ],
    };
    // A Vec field has no Display, so it must not appear in field()'s
    // path space at all — this is what makes it compile in the first
    // place (Vec<SeriesHandle>: !Display).
    assert_eq!(s.field("series"), None);
    assert!(DashboardState::field_names().is_empty());

    let items = s.collection("series").expect("series is an iterable path");
    assert_eq!(items.len(), 2);
    // Each element's own Display form (round-trips via FromStr, same
    // as a scalar SeriesHandle field already does).
    assert_eq!(items[0].parse::<SeriesHandle>().unwrap(), s.series[0]);
    assert_eq!(items[1].parse::<SeriesHandle>().unwrap(), s.series[1]);

    assert_eq!(s.collection("nonexistent"), None);
}

#[test]
fn structured_vec_elements_expose_fields_via_collection_rows() {
    use libra_core::{collection_row_opaque, CollectionRow, LibraReflect};

    #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, LibraReflect)]
    struct Row {
        id: u32,
        label: String,
    }
    impl std::fmt::Display for Row {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.id)
        }
    }

    #[state]
    struct ListState {
        rows: Vec<Row>,
    }

    let s = ListState {
        rows: vec![Row { id: 7, label: "Alpha".into() }],
    };
    let rows = s.collection_rows("rows").expect("rows is iterable");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].whole, "7");
    assert_eq!(
        rows[0].get("t.id", "t").as_deref(),
        Some("7")
    );
    assert_eq!(
        rows[0].get("t.label", "t").as_deref(),
        Some("Alpha")
    );
    // Opaque SeriesHandle-style path still works via collection_row_opaque:
    let opaque = collection_row_opaque(&"plain".to_string());
    assert_eq!(opaque, CollectionRow::opaque("plain"));
    assert!(opaque.fields.is_empty());
}

#[test]
fn zero_arg_methods_become_unit_variants_dispatchable_by_camelcase_name() {
    assert_eq!(Action::action_by_name("increment"), Some(Action::Increment));
    assert_eq!(Action::action_by_name("decrement"), Some(Action::Decrement));
    // snake_case method -> PascalCase variant -> camelCase public name:
    assert_eq!(Action::action_by_name("resetCount"), Some(Action::ResetCount));
    // The Rust identifier itself is not the public name:
    assert_eq!(Action::action_by_name("reset_count"), None);
    assert_eq!(Action::action_by_name("ResetCount"), None);
}

#[test]
fn single_string_arg_methods_are_handlers_dispatchable_with_a_control_value() {
    assert_eq!(
        Action::action_with_str("filterChanged", "hi".into()),
        Some(Action::FilterChanged("hi".into()))
    );
    // Not nameable via action_by_name (it takes a payload):
    assert_eq!(Action::action_by_name("filterChanged"), None);
}

#[test]
fn continuations_are_not_nameable_from_markup() {
    // `fetched` takes an HttpResult, not a control payload — neither
    // resolution path should construct it by name.
    assert_eq!(Action::action_by_name("fetched"), None);
    assert_eq!(Action::action_with_str("fetched", String::new()), None);
}

#[test]
fn single_frame_info_arg_methods_are_handlers_for_source_elements() {
    let info = FrameInfo::at(1500.0);
    assert_eq!(
        Action::action_with_frame_info("frame", info),
        Some(Action::Frame(info))
    );
    // Not nameable via the other two paths:
    assert_eq!(Action::action_by_name("frame"), None);
    assert_eq!(Action::action_with_str("frame", String::new()), None);

    let mut s = CounterState { count: 0, quote: None, last_frame_ms: 0.0 };
    reduce(&mut s, Action::Frame(info));
    assert_eq!(s.last_frame_ms, 1500.0);
}

#[test]
fn action_names_lists_every_variant_including_continuations() {
    let names = Action::action_names();
    for expect in ["increment", "decrement", "resetCount", "filterChanged", "fetch", "fetched"] {
        assert!(names.contains(&expect), "missing '{expect}' in {names:?}");
    }
}

#[test]
fn reduce_dispatches_zero_arg_and_str_arg_actions_and_calls_the_real_method() {
    let mut s = CounterState { count: 0, quote: None, last_frame_ms: 0.0 };
    let fx = reduce(&mut s, Action::Increment);
    assert_eq!(s.count, 1);
    assert!(fx.is_empty());

    reduce(&mut s, Action::Increment);
    reduce(&mut s, Action::Increment);
    assert_eq!(s.count, 3);

    reduce(&mut s, Action::ResetCount);
    assert_eq!(s.count, 0);

    reduce(&mut s, Action::FilterChanged("zen".into()));
    assert_eq!(s.quote.as_deref(), Some("zen"));
}

#[test]
fn reduce_surfaces_effects_returned_by_an_action_and_dispatches_the_continuation() {
    let mut s = CounterState { count: 0, quote: None, last_frame_ms: 0.0 };
    let fx = reduce(&mut s, Action::Fetch);
    assert_eq!(fx.len(), 1);
    let Effect::Http { req, on_done } = &fx[0] else { panic!("expected Http") };
    assert_eq!(req.url, "https://api.example.com/session");

    // The effect's own ctor constructs the continuation — exactly the
    // integration-test mechanism (§15.4) in miniature.
    let continuation = on_done(HttpResult::Response { status: 200, body: b"Mind the gap.".to_vec() });
    let more_fx = reduce(&mut s, continuation);
    assert!(more_fx.is_empty());
    assert_eq!(s.quote.as_deref(), Some("Mind the gap."));
}

#[test]
fn into_effects_lets_unit_and_option_and_vec_returns_all_compile() {
    // Compile-time proof that the four §5.1 return shapes all satisfy
    // IntoEffects<Action> — increment/decrement return (), fetch
    // returns Effect<Action> directly (exercised above). Here: Option
    // and Vec via the trait directly, same as the macro's generated
    // reduce arms would produce for a differently-shaped method.
    let none: Vec<Effect<Action>> = None::<Effect<Action>>.into_effects();
    assert!(none.is_empty());
    let one: Vec<Effect<Action>> =
        Some(Effect::http(HttpRequest::get("x"), Action::Fetched)).into_effects();
    assert_eq!(one.len(), 1);
}

/// §6.3's `#[project]` hook, in its own module (one `#[actions]` block
/// per module). The hook must: stay out of the Action enum, run after
/// every reduce — unit handlers, payload handlers, and continuations
/// alike — and back `__libra_project` for `app!`'s projected default.
mod projected {
    use libra_core::ReflectAction;
    use libra_reflect::{actions, state};

    #[state]
    #[derive(Default)]
    struct Todos {
        #[libra(skip)]
        items: Vec<(String, bool)>,
        summary: String,
    }

    #[actions]
    impl Todos {
        fn add(&mut self, text: String) {
            self.items.push((text, false));
        }

        fn complete_first(&mut self) {
            if let Some(t) = self.items.first_mut() {
                t.1 = true;
            }
        }

        // The projection hook: not an action, called by reduce itself.
        #[project]
        fn project(&mut self) {
            let done = self.items.iter().filter(|t| t.1).count();
            self.summary = format!("{done} of {} done", self.items.len());
        }
    }

    #[test]
    fn project_runs_after_every_action_without_being_called() {
        let mut s = Todos::default();
        reduce(&mut s, Action::Add("a".into()));
        assert_eq!(s.summary, "0 of 1 done");
        reduce(&mut s, Action::CompleteFirst);
        assert_eq!(s.summary, "1 of 1 done");
    }

    #[test]
    fn project_is_not_an_action() {
        assert!(Action::action_by_name("project").is_none(), "#[project] must not enter the enum");
        assert!(!Action::action_names().contains(&"project"));
    }

    #[test]
    fn libra_project_backs_the_projected_default() {
        let mut s = Todos::default();
        assert_eq!(s.summary, "", "raw Default is unprojected");
        s.__libra_project();
        assert_eq!(s.summary, "0 of 0 done", "app!'s no-init default projects");
    }
}

/// A block *without* `#[project]` still gets the no-op `__libra_project`
/// (what `app!`'s default init calls unconditionally).
mod unprojected {
    use libra_reflect::{actions, state};

    #[state]
    #[derive(Default)]
    struct Plain {
        count: i64,
    }

    #[actions]
    impl Plain {
        fn increment(&mut self) {
            self.count += 1;
        }
    }

    #[test]
    fn libra_project_is_a_noop_without_a_hook() {
        let mut s = Plain::default();
        s.__libra_project();
        assert_eq!(s, Plain::default());
        let _ = Action::Increment; // silence single-variant lint surface
    }
}
