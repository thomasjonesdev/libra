//! `#[libra::state]` / `#[libra::actions]` exercised against the real
//! libra-core traits (dev-dep cycle, permitted by cargo). Imported here
//! via their real path (`libra_reflect::{state, actions}`) since the
//! `libra` facade crate that makes `#[libra::state]` literally resolve
//! doesn't exist yet (PLAN-ERGONOMICS.md E1) — same macros, same
//! expansion, different import spelling.

use libra_core::{Effect, HttpRequest, HttpResult, IntoEffects, ReflectAction, ReflectState};
use libra_reflect::{actions, state};

#[state]
struct CounterState {
    count: i64,
    #[libra(skip)]
    #[allow(dead_code)]
    quote: Option<String>,
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
}

#[test]
fn state_derives_are_bundled_and_reflect_state_works() {
    let s = CounterState { count: 3, quote: None };
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
fn action_names_lists_every_variant_including_continuations() {
    let names = Action::action_names();
    for expect in ["increment", "decrement", "resetCount", "filterChanged", "fetch", "fetched"] {
        assert!(names.contains(&expect), "missing '{expect}' in {names:?}");
    }
}

#[test]
fn reduce_dispatches_zero_arg_and_str_arg_actions_and_calls_the_real_method() {
    let mut s = CounterState { count: 0, quote: None };
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
    let mut s = CounterState { count: 0, quote: None };
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
