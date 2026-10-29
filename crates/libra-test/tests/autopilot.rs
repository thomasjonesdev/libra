//! Guardrails for the autopilot itself (§15.7).
//!
//! The autopilot is a test-writing tool, so its own failure mode is the
//! dangerous kind: a run that quietly explores less than it claims, or reports
//! a failure nobody can reproduce. These pin the two properties the whole
//! thing rests on — a run is a pure function of its seed, and effects really
//! are answered out of emission order — plus the diagnostics that make a
//! failure actionable.
//!
//! The app under test is hand-written reflection (the §21 escape hatch) rather
//! than a real example app, so `libra-test` stays free of a dependency on the
//! crates that depend on it.

use libra_core::{
    App, CommandSpec, Effect, ProcessResult, ProcessStatus, ReflectAction, ReflectState,
};
use libra_test::{by, Budget, Harness, Reply};

const VIEW: &str = r#"
<Column gap={8} padding={12}>
  <Text testId="count">{count}</Text>
  <Text testId="loaded">{loaded}</Text>
  <Button onPress={bump} testId="bump">Bump</Button>
  <Button onPress={load} testId="load">Load</Button>
</Column>
"#;

#[derive(Default, Clone)]
struct Probe {
    count: u32,
    loaded: String,
    /// Every `Load` press appends here, so a run's history is comparable
    /// across seeds without depending on frame timing.
    log: Vec<String>,
}

#[derive(Clone, Debug)]
enum Msg {
    Bump,
    Load,
    Loaded(ProcessResult),
}

impl ReflectState for Probe {
    fn field(&self, path: &str) -> Option<String> {
        match path {
            "count" => Some(self.count.to_string()),
            "loaded" => Some(self.loaded.clone()),
            _ => None,
        }
    }
    fn field_names() -> &'static [&'static str] {
        &["count", "loaded"]
    }
}

impl ReflectAction for Msg {
    fn action_by_name(name: &str) -> Option<Msg> {
        match name {
            "bump" => Some(Msg::Bump),
            "load" => Some(Msg::Load),
            _ => None,
        }
    }
    fn action_names() -> &'static [&'static str] {
        &["bump", "load"]
    }
}

impl App for Probe {
    type Msg = Msg;

    fn init() -> (Probe, Vec<Effect<Msg>>) {
        (Probe::default(), Vec::new())
    }

    fn update(&mut self, msg: Msg) -> Vec<Effect<Msg>> {
        match msg {
            Msg::Bump => {
                self.count += 1;
                Vec::new()
            }
            Msg::Load => vec![Effect::process(
                CommandSpec::new("probe").args(["read", &self.count.to_string()]),
                Msg::Loaded,
            )],
            Msg::Loaded(result) => {
                let out = String::from_utf8_lossy(&result.stdout).into_owned();
                self.loaded = out.clone();
                self.log.push(out.clone());
                // A result that asks for another read — the shape of a real
                // effect cycle (a load whose completion re-triggers a load).
                if out == "again" {
                    return vec![Effect::process(
                        CommandSpec::new("probe").args(["read", "again"]),
                        Msg::Loaded,
                    )];
                }
                Vec::new()
            }
        }
    }
}

fn ok(stdout: &str) -> ProcessResult {
    ProcessResult {
        status: ProcessStatus::Exited(0),
        stdout: stdout.as_bytes().to_vec(),
        stderr: vec![],
        finished_at_unix_ms: 1,
    }
}

/// Answers every process with the arguments it was called with, so the log
/// records the exact sequence a run produced.
fn probe(seed: u64) -> Harness<Probe> {
    Harness::<Probe>::with_viewport(VIEW, 320, 200)
        .seed(seed)
        .responder(|effect| match effect {
            Effect::Process { command, .. } => Reply::Process(ok(&command.args.join(" "))),
            _ => Reply::Drop,
        })
}

#[test]
fn a_run_is_a_pure_function_of_its_seed() {
    // The whole reporting story depends on this: a failure prints a seed, and
    // the seed has to be enough to get the failure back.
    let mut first = probe(7);
    first.autopilot(120).without_render().run();
    let mut second = probe(7);
    second.autopilot(120).without_render().run();

    assert_eq!(first.state().count, second.state().count);
    assert_eq!(first.state().log, second.state().log);
    assert!(
        !first.state().log.is_empty(),
        "the run never pressed Load — this test would prove nothing"
    );
}

#[test]
fn a_different_seed_takes_a_different_path() {
    let mut a = probe(7);
    a.autopilot(120).without_render().run();
    let mut b = probe(99);
    b.autopilot(120).without_render().run();
    assert_ne!(
        (a.state().count, a.state().log.len()),
        (b.state().count, b.state().log.len()),
        "two seeds produced the same run — the seed is not reaching the choices"
    );
}

#[test]
fn effects_are_answered_out_of_emission_order() {
    // §12.3 leaves cross-effect ordering unguaranteed, so the drain shuffles.
    // Emission order is the one order production will never promise.
    let mut h = Harness::<Probe>::with_viewport(VIEW, 320, 200)
        .seed(3)
        .responder(|effect| match effect {
            Effect::Process { command, .. } => Reply::Process(ok(&command.args[1])),
            _ => Reply::Drop,
        });
    for _ in 0..8 {
        h.dispatch(Msg::Bump);
        h.dispatch(Msg::Load);
    }
    let delivered = h.drain_effects();
    assert_eq!(delivered, 8);
    let order: Vec<&str> = h.state().log.iter().map(String::as_str).collect();
    let emitted: Vec<String> = (1..=8).map(|n| n.to_string()).collect();
    assert_ne!(
        order, emitted,
        "the drain answered in emission order — the §12.3 shuffle is not happening"
    );
    let mut sorted = order.clone();
    sorted.sort_by_key(|s| s.parse::<u32>().expect("numeric"));
    assert_eq!(
        sorted, emitted,
        "the shuffle dropped or duplicated an effect"
    );
}

#[test]
#[should_panic(expected = "effect cycle")]
fn a_nonterminating_effect_chain_is_reported_rather_than_hung() {
    // The responder answers every read with "again", and the app responds to
    // "again" by reading again. Without the round cap this spins until the
    // test harness times out, which reports nothing anyone can act on.
    let mut h = Harness::<Probe>::with_viewport(VIEW, 320, 200).responder(|effect| match effect {
        Effect::Process { .. } => Reply::Process(ok("again")),
        _ => Reply::Drop,
    });
    h.dispatch(Msg::Load);
    h.drain_effects();
}

#[test]
fn a_terminating_chain_settles() {
    // The same machinery must not cry cycle over an ordinary multi-step flow.
    let mut h = probe(1);
    h.dispatch(Msg::Load);
    assert_eq!(h.drain_effects(), 1);
    h.expect_no_effects();
}

#[test]
fn discovery_finds_the_pressable_items() {
    let h = probe(1);
    let found = libra_test::autopilot::discover(&h);
    let mut labels: Vec<&str> = found.iter().map(|a| a.label.as_str()).collect();
    labels.sort_unstable();
    assert_eq!(labels, vec!["bump", "load"], "found {found:?}");
}

#[test]
#[should_panic(expected = "invariant 'count stays small'")]
fn a_broken_invariant_names_itself_and_the_seed() {
    let mut h = probe(11);
    h.autopilot(200)
        .without_render()
        .invariant("count stays small", |s: &Probe| s.count < 3)
        .run();
}

#[test]
#[should_panic(expected = "'log' grew to")]
fn an_exceeded_bound_reports_the_measurement() {
    let mut h = probe(11);
    h.autopilot(200)
        .without_render()
        .bounded("log", 2, |s: &Probe| s.log.len())
        .run();
}

#[test]
#[should_panic(expected = "over the 0.00 ms budget")]
fn a_blown_budget_cites_the_section() {
    let mut h = probe(5);
    h.autopilot(20).budget(Budget::new().max_ms(0.0)).run();
}

#[test]
fn avoided_handlers_are_never_driven() {
    let mut h = probe(4);
    h.autopilot(150)
        .without_render()
        .avoid_handlers(["load"])
        .run();
    assert!(
        h.state().log.is_empty(),
        "the autopilot drove an avoided handler"
    );
    assert!(h.state().count > 0, "it should still have pressed Bump");
}

#[test]
fn the_run_report_names_the_slowest_interaction() {
    let mut h = probe(2);
    let run = h.autopilot(30).run();
    let (what, ms) = run.slowest.expect("a run with steps has a slowest one");
    assert!(
        what.contains("bump") || what.contains("load"),
        "slowest should name an affordance, got {what:?}"
    );
    assert!(ms >= 0.0);
    assert!(run.frame_ms.windows(2).all(|w| w[0] <= w[1]), "sorted");
}

/// A hand-written harness verb check: clicking still routes through real hit
/// testing after the index refactor the autopilot needed.
#[test]
fn click_by_selector_still_dispatches() {
    let mut h = probe(1);
    h.click(by::test_id("bump"));
    h.query(by::test_id("count")).assert_text("1");
}
