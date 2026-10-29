//! Autopilot — affordance-level fuzzing over the real tree (§15.7, §22).
//!
//! §15.7 asks for action-sequence fuzzing: generate sequences, assert
//! invariants hold throughout. The autopilot does that one level up, driving
//! the *affordances the view actually offers* rather than the `Action` enum.
//! The difference matters, because the two find different bugs. Synthesized
//! actions reach states no user can reach and miss states every user reaches;
//! a run that presses what is on screen only visits reachable states, and
//! visits them in the orders a hurried user produces — including the ones that
//! arrive faster than the effects answering the previous step.
//!
//! Each step re-discovers the tree, picks an affordance with the seeded PRNG,
//! drives it through the same verbs a hand-written component test uses, then
//! drains effects in shuffled order (§12.3). Between steps it checks the
//! invariants the test declared and records the frame cost, so one run covers
//! §15.7's "never panics, invariants hold" and §22's frame budget at once.
//!
//! Everything is deterministic: same seed, same run. A failure prints the seed
//! and the step log, and re-running reproduces it exactly.

use libra_components::{ItemKind, Role};
use libra_core::App;

use crate::Harness;

/// Something on screen the autopilot can drive.
#[derive(Clone, Debug)]
pub struct Affordance {
    /// Index into the laid tree *as discovered this step* — the tree rebuilds
    /// after every interaction, so this is only valid until the next one.
    pub index: usize,
    pub kind: AffordanceKind,
    /// Human-readable identity for the step log and failure messages.
    pub label: String,
    /// The handler this would dispatch, when it has one — the hook `avoid`
    /// filters on.
    pub handler: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AffordanceKind {
    /// A pressable item: Button, ListItem, Tab, Chip, and friends.
    Press,
    /// An `Input`/`Search`/`Textarea` — the autopilot types into it.
    Type,
    /// Click into a text field and leave it, changing nothing.
    ///
    /// Distinct from [`AffordanceKind::Type`] because it reaches a different
    /// state: a focused field whose draft is whatever the app last put there.
    /// Clicking away then fires `onSubmit` over that untouched draft, which is
    /// how a stale draft turns into a spurious write — a path no run that only
    /// types can reach, and the one a real user takes constantly.
    Focus,
    /// A `<Scroll>` viewport, scrolled by a step.
    Scroll,
}

/// A §22 frame budget. Wall-clock, so it is machine-dependent by nature —
/// §22 enforces budgets "on a pinned reference machine class". Pick values
/// that catch a regression in kind (a 30× worst-frame blowup) rather than
/// values that track your laptop's thermal state.
#[derive(Clone, Copy, Debug, Default)]
pub struct Budget {
    pub p50_ms: Option<f64>,
    pub p95_ms: Option<f64>,
    pub max_ms: Option<f64>,
}

impl Budget {
    pub fn new() -> Budget {
        Budget::default()
    }
    pub fn p50_ms(mut self, ms: f64) -> Budget {
        self.p50_ms = Some(ms);
        self
    }
    pub fn p95_ms(mut self, ms: f64) -> Budget {
        self.p95_ms = Some(ms);
        self
    }
    /// The one that catches stalls. A p50 regression is a slow creep; a max
    /// regression is the frame the user watches the window freeze on.
    pub fn max_ms(mut self, ms: f64) -> Budget {
        self.max_ms = Some(ms);
        self
    }
}

/// What a completed run measured.
#[derive(Clone, Debug)]
pub struct Run {
    pub seed: u64,
    pub steps: usize,
    pub effects_delivered: usize,
    /// Per-step frame cost in ms (layout through rasterize), ascending.
    pub frame_ms: Vec<f64>,
    /// The interaction behind the worst frame. A budget failure is only
    /// actionable if it names what was slow.
    pub slowest: Option<(String, f64)>,
}

impl Run {
    pub fn p50_ms(&self) -> f64 {
        self.quantile(0.50)
    }
    pub fn p95_ms(&self) -> f64 {
        self.quantile(0.95)
    }
    pub fn max_ms(&self) -> f64 {
        self.frame_ms.last().copied().unwrap_or(0.0)
    }

    fn quantile(&self, q: f64) -> f64 {
        if self.frame_ms.is_empty() {
            return 0.0;
        }
        self.frame_ms[((self.frame_ms.len() - 1) as f64 * q) as usize]
    }
}

impl std::fmt::Display for Run {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "seed {} · {} steps · {} effects · frame p50 {:.2} ms  p95 {:.2} ms  max {:.2} ms",
            self.seed,
            self.steps,
            self.effects_delivered,
            self.p50_ms(),
            self.p95_ms(),
            self.max_ms()
        )?;
        if let Some((what, ms)) = &self.slowest {
            write!(f, " · worst {what} ({ms:.2} ms)")?;
        }
        Ok(())
    }
}

/// What the run just did, for invariants whose truth depends on the
/// interaction and not only on the state.
///
/// Focus is the common reason to need this. It is a deterministic input the
/// harness owns rather than app state (like scroll and hover), so a property
/// such as "this draft matches the selection *unless the user is editing it*"
/// is not expressible from `&A` alone.
#[derive(Clone, Copy, Debug)]
pub struct Step<'a> {
    pub index: usize,
    pub kind: AffordanceKind,
    pub label: &'a str,
    /// `testId` of the focused text field, if one has focus.
    pub focused: Option<&'a str>,
    /// Whether effects had settled when this check ran. False mid-burst.
    pub settled: bool,
}

type Invariant<A> = (String, Box<dyn Fn(&A) -> bool>);
type StepInvariant<A> = (String, Box<dyn Fn(&A, &Step<'_>) -> bool>);
type Metric<A> = (String, Box<dyn Fn(&A) -> usize>, usize);

/// Builder for one autopilot run. See the module docs.
pub struct Autopilot<'h, A: App> {
    harness: &'h mut Harness<A>,
    steps: usize,
    invariants: Vec<Invariant<A>>,
    always: Vec<Invariant<A>>,
    always_at: Vec<StepInvariant<A>>,
    bounded: Vec<Metric<A>>,
    budget: Option<Budget>,
    avoid: Vec<Box<dyn Fn(&Affordance) -> bool>>,
    typed_text: String,
    burst: usize,
    render: bool,
    verbose: bool,
}

impl<'h, A: App> Autopilot<'h, A> {
    pub(crate) fn new(harness: &'h mut Harness<A>, steps: usize) -> Autopilot<'h, A> {
        Autopilot {
            harness,
            steps,
            invariants: Vec::new(),
            always: Vec::new(),
            always_at: Vec::new(),
            bounded: Vec::new(),
            budget: None,
            avoid: Vec::new(),
            typed_text: "libra".into(),
            burst: 3,
            render: true,
            verbose: false,
        }
    }

    /// A property that must hold once the app has *settled* — effects drained,
    /// nothing outstanding. Use this for things that are allowed to be briefly
    /// untrue while work is in flight: "no I/O turn is left hanging", "no work
    /// left queued".
    pub fn invariant(mut self, name: &str, f: impl Fn(&A) -> bool + 'static) -> Autopilot<'h, A> {
        self.invariants.push((name.into(), Box::new(f)));
        self
    }

    /// A property that must hold after *every* step, including mid-burst with
    /// requests outstanding.
    ///
    /// This is where the interesting failures live. A field that describes
    /// another field — a draft of the selected note, a label for the current
    /// tab — is easy to update on the I/O completion that produced it and
    /// forget to update when the selection moves. That is invisible to a
    /// settled check, because the completion repairs it before anything looks;
    /// it is visible to the *next click*, which arrives first and acts on a
    /// value describing something the user is no longer looking at.
    pub fn always(mut self, name: &str, f: impl Fn(&A) -> bool + 'static) -> Autopilot<'h, A> {
        self.always.push((name.into(), Box::new(f)));
        self
    }

    /// [`Autopilot::always`] with the [`Step`] that just ran — for properties
    /// that hold only under some interactions, most often "unless this field
    /// has focus".
    pub fn always_at(
        mut self,
        name: &str,
        f: impl Fn(&A, &Step<'_>) -> bool + 'static,
    ) -> Autopilot<'h, A> {
        self.always_at.push((name.into(), Box::new(f)));
        self
    }

    /// A count that must never exceed `max` — undo stacks, open tabs,
    /// projected rows. This is the leak check that actually belongs in a
    /// deterministic test: process RSS depends on the allocator's mood, while
    /// "history never exceeds 200" is a property of the app.
    pub fn bounded(
        mut self,
        name: &str,
        max: usize,
        f: impl Fn(&A) -> usize + 'static,
    ) -> Autopilot<'h, A> {
        self.bounded.push((name.into(), Box::new(f), max));
        self
    }

    /// Assert §22 frame budgets at the end of the run.
    pub fn budget(mut self, budget: Budget) -> Autopilot<'h, A> {
        self.budget = Some(budget);
        self
    }

    /// Skip affordances this predicate accepts — the escape hatch for
    /// controls that would end the run or leave the app (quit, "open vault"
    /// sheets) rather than exercise it.
    pub fn avoid(mut self, f: impl Fn(&Affordance) -> bool + 'static) -> Autopilot<'h, A> {
        self.avoid.push(Box::new(f));
        self
    }

    /// Skip any affordance whose handler is named here.
    pub fn avoid_handlers<S: AsRef<str>>(
        self,
        names: impl IntoIterator<Item = S>,
    ) -> Autopilot<'h, A> {
        let names: Vec<String> = names.into_iter().map(|s| s.as_ref().to_string()).collect();
        self.avoid(move |a| {
            a.handler
                .as_deref()
                .is_some_and(|h| names.iter().any(|n| n == h))
        })
    }

    /// What to type when the autopilot picks a text field.
    pub fn typing(mut self, text: &str) -> Autopilot<'h, A> {
        self.typed_text = text.into();
        self
    }

    /// How many interactions may stack up before the run waits for effects to
    /// settle — the impatient-user knob (default 3).
    ///
    /// Zero means every step waits for its own I/O, which is the one thing a
    /// user in a hurry never does. Above zero, clicks arrive while earlier
    /// requests are still outstanding, so the app is asked to act on state a
    /// pending result is about to overwrite. That is where "it wedges if I
    /// click too fast" lives, and it is unreachable from a run that waits.
    pub fn burst(mut self, max: usize) -> Autopilot<'h, A> {
        self.burst = max;
        self
    }

    /// Skip rasterizing each frame. Faster, but gives up the frame budget —
    /// only worth it for a run that is purely about state invariants.
    pub fn without_render(mut self) -> Autopilot<'h, A> {
        self.render = false;
        self
    }

    /// Print each step. Use with `cargo test -- --nocapture` when a run fails
    /// and you want to see how it got there.
    pub fn verbose(mut self) -> Autopilot<'h, A> {
        self.verbose = true;
        self
    }

    /// Drive the app and return what the run measured. Panics on the first
    /// violated invariant, exceeded bound, or blown budget, naming the seed
    /// and step so the failure replays.
    pub fn run(self) -> Run {
        let Autopilot {
            harness,
            steps,
            invariants,
            always,
            always_at,
            bounded,
            budget,
            avoid,
            typed_text,
            burst,
            render,
            verbose,
        } = self;
        let seed = harness.seed_value();
        let mut frame_ms = Vec::with_capacity(steps);
        let mut delivered = harness.drain_effects();
        let mut taken = 0usize;
        // Interactions left in the current burst — steps taken without waiting
        // for the previous one's effects to land. See `burst`.
        let mut burst_left = 0usize;
        let mut slowest: Option<(String, f64)> = None;

        for step in 0..steps {
            let mut targets = discover(harness);
            targets.retain(|a| !avoid.iter().any(|f| f(a)));
            if targets.is_empty() {
                // Nothing left to drive. Not a failure — a modal with one
                // dismissed control, or an app that ran out of screen.
                break;
            }
            let pick = harness.pick(targets.len());
            let target = targets[pick].clone();
            if verbose {
                println!(
                    "[autopilot] step {step}: {:?} {}",
                    target.kind, target.label
                );
            }

            let started = std::time::Instant::now();
            drive(harness, &target, &typed_text);
            // Mid-burst nothing is answered, so the next interaction lands with
            // the previous one's request still outstanding. At the end of a
            // burst everything settles, because the invariants below describe
            // the app at rest, not mid-flight.
            let settling = burst_left == 0;
            if settling {
                delivered += harness.drain_effects();
            } else {
                burst_left -= 1;
            }
            if render {
                std::hint::black_box(harness.frame_png());
            }
            let ms = started.elapsed().as_secs_f64() * 1000.0;
            if slowest.as_ref().is_none_or(|(_, worst)| ms > *worst) {
                slowest = Some((format!("{:?} {}", target.kind, target.label), ms));
            }
            frame_ms.push(ms);
            taken += 1;

            let focused = harness.focused_test_id().map(str::to_string);
            let context = Step {
                index: step,
                kind: target.kind,
                label: &target.label,
                focused: focused.as_deref(),
                settled: settling,
            };
            let flight = if settling {
                ""
            } else {
                ", with effects still in flight"
            };
            let state = harness.state();
            for (name, holds) in &always {
                assert!(
                    holds(state),
                    "autopilot: '{name}' does not hold at step {step} \
                     after {:?} {}{flight} (seed {seed})",
                    target.kind,
                    target.label,
                );
            }
            for (name, holds) in &always_at {
                assert!(
                    holds(state, &context),
                    "autopilot: '{name}' does not hold at step {step} \
                     after {:?} {}{flight} (focus {:?}) (seed {seed})",
                    target.kind,
                    target.label,
                    context.focused,
                );
            }
            if !settling {
                continue;
            }
            for (name, holds) in &invariants {
                assert!(
                    holds(state),
                    "autopilot: invariant '{name}' broken at step {step} \
                     after {:?} {} (seed {seed})",
                    target.kind,
                    target.label,
                );
            }
            for (name, measure, max) in &bounded {
                let n = measure(state);
                assert!(
                    n <= *max,
                    "autopilot: '{name}' grew to {n} (max {max}) at step {step} \
                     after {:?} {} (seed {seed})",
                    target.kind,
                    target.label,
                );
            }
            burst_left = harness.pick(burst + 1);
        }

        // A run that ends mid-burst would otherwise never have its invariants
        // checked at rest.
        delivered += harness.drain_effects();
        let state = harness.state();
        for (name, holds) in invariants.iter().chain(&always) {
            assert!(
                holds(state),
                "autopilot: '{name}' broken once the run settled (seed {seed})"
            );
        }

        let mut sorted = frame_ms;
        sorted.sort_by(f64::total_cmp);
        let run = Run {
            seed,
            steps: taken,
            effects_delivered: delivered,
            frame_ms: sorted,
            slowest,
        };
        if let Some(budget) = budget {
            check_budget(&run, budget);
        }
        run
    }
}

fn check_budget(run: &Run, budget: Budget) {
    let checks = [
        ("p50", budget.p50_ms, run.p50_ms()),
        ("p95", budget.p95_ms, run.p95_ms()),
        ("max", budget.max_ms, run.max_ms()),
    ];
    for (label, limit, actual) in checks {
        if let Some(limit) = limit {
            assert!(
                actual <= limit,
                "autopilot: frame {label} was {actual:.2} ms, over the {limit:.2} ms \
                 budget (§22) — {run}"
            );
        }
    }
}

/// Everything on the current frame the autopilot knows how to drive.
///
/// Reads the same laid tree the a11y projection and every component test query
/// read, so an affordance is discoverable here exactly when it is reachable by
/// a real pointer. A control the autopilot cannot find is a control assistive
/// tech cannot find either.
pub fn discover<A: App>(harness: &Harness<A>) -> Vec<Affordance> {
    let laid = harness.laid();
    let mut out = Vec::new();
    for (index, item) in laid.items.iter().enumerate() {
        if item.rect.w <= 1.0 || item.rect.h <= 1.0 {
            continue;
        }
        let label = item
            .semantics
            .test_id
            .clone()
            .or_else(|| (!item.semantics.text.is_empty()).then(|| item.semantics.text.clone()))
            .unwrap_or_else(|| format!("#{index}"));
        // Anything driven by a click must survive hit testing at the point the
        // click lands: clip rects, modal barriers and overlapping siblings make
        // plenty of items unreachable, and driving one would assert something
        // no user can do. Typing is exempt — it addresses the field directly,
        // the way a Tab-focused field receives keys.
        let (cx, cy) = item.rect.center();
        let clickable = laid.hit(cx, cy).is_some();
        let kinds: &[AffordanceKind] = if item.semantics.role == Role::TextInput {
            // Two different interactions reaching different states — see
            // `AffordanceKind::Focus`.
            if clickable {
                &[AffordanceKind::Type, AffordanceKind::Focus]
            } else {
                &[AffordanceKind::Type]
            }
        } else if matches!(item.kind, ItemKind::Scroll) && item.semantics.test_id.is_some() {
            &[AffordanceKind::Scroll]
        } else if item.on_press.is_some() && clickable {
            &[AffordanceKind::Press]
        } else {
            continue;
        };
        let label = match &item.on_press_arg {
            Some(arg) => format!("{label} ({arg})"),
            None => label,
        };
        for &kind in kinds {
            out.push(Affordance {
                index,
                kind,
                label: label.clone(),
                handler: item.on_press.clone(),
            });
        }
    }
    out
}

fn drive<A: App>(harness: &mut Harness<A>, target: &Affordance, typed: &str) {
    match target.kind {
        // Clicking a text field places a caret and dispatches nothing (§10.1),
        // so this is the focus-only interaction.
        AffordanceKind::Press | AffordanceKind::Focus => {
            harness.click_item(target.index, &target.label)
        }
        AffordanceKind::Type => harness.type_into_item(target.index, typed),
        AffordanceKind::Scroll => {
            let id = harness.laid().items[target.index]
                .semantics
                .test_id
                .clone()
                .expect("discover only yields Scroll with a testId");
            harness.scroll(&id, 120.0);
        }
    }
}
