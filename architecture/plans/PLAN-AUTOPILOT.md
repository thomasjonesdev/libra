---
title: "PLAN-AUTOPILOT"
aliases:
  - "PLAN-AUTOPILOT"
tags:
  - plans
  - architecture
type: plan
---

# Autopilot — affordance-level fuzzing in the harness

**Status: shipped (A1–A4).** Implements the action-sequence fuzzing line of
[[15.7-fuzzing-property-tests|§15.7]] and the "frame-timing assertions in
snapshot-harness runs" line of [[22-performance-budgets|§22]]. It adds no new
crate and no new test *kind*: it is a set of verbs on the existing
[[15.3-component-tests|§15.3]] `Harness`, in the same spirit as the named
property tests that guard `libra-raster` and `libra-data`.

## Goal

A run that drives whatever the view offers, in orders a hurried user produces,
and fails with a seed you can replay.

The five [[15-testing|§15]] kinds all describe a sequence someone thought of.
That is their strength for regression and their blind spot for discovery: the
bugs that survive review are the ones nobody thought to script. §15.7 already
asks for fuzzing against invariants; the autopilot supplies the missing input
side, generating *interactions* rather than `Action` values.

Generating actions directly would be easier and worse. Synthesized actions
reach states no user can reach — which produces failures nobody has to fix —
and skip states every user reaches, because reaching them takes a particular
sequence of presses. Driving discovered affordances only ever visits reachable
states.

## Foundation (decided)

**Everything stays inside the determinism contract.** No real I/O
([[12.6-the-process-effect|§12.6]]), no window, no clock in any decision. A run
is a pure function of `(app, view, responder, seed)`.

Four pieces, all on `Harness`:

| Piece | What it adds |
|---|---|
| **Responder** | `.responder(fn)` answers *kinds* of effect with fixtures, so a run can take steps whose effects it cannot enumerate in advance. Results are still hand-fed data — nothing executes. |
| **Shuffled drain** | `drain_effects` answers the queue in a seeded shuffle, not emission order. [[12.3-execution-model|§12.3]] leaves ordering unguaranteed, so emission order is the one arrangement production will never promise. |
| **Autopilot** | Discovers affordances from the `LaidTree`, picks one with the seeded PRNG, drives it through the ordinary harness verbs, checks invariants, records the frame. |
| **Budgets** | Per-step frame cost with §22 assertions, and bounded-metric guards for growth. |

### Affordance discovery reads the semantics tree

Discovery walks the same laid tree the a11y projection and every component
query read ([[17-accessibility|§17]]), and hit-tests anything driven by a click.
So an affordance is discoverable exactly when it is reachable by a real
pointer — a control the autopilot cannot find is one assistive tech cannot find
either. Text fields yield **two** affordances, `Type` and `Focus`, because
clicking into a field and leaving it changes nothing yet reaches a different
state than typing does: it is the blur-submit path.

### Bursts — the part that finds the bugs

By default a run does not wait for its own I/O. It takes 0–3 further
interactions with requests still outstanding, then settles. This is the
difference between a user who waits for each spinner and a real one, and it is
where "it wedges if I click too fast" lives: the next action reads state that a
pending result was about to overwrite.

Bursts force the invariant split:

- `.invariant(…)` — must hold **at rest**, once effects have drained. For
  things allowed to be briefly untrue in flight ("no work left queued").
- `.always(…)` / `.always_at(…)` — must hold after **every** step, including
  mid-burst. This is where the interesting failures are. A field that describes
  another field is easy to update on the completion that produced it and forget
  to update when the selection moves; the completion repairs it before any
  settled check looks, but the *next click* arrives first and acts on it.

`.always_at` also receives the `Step` (which affordance ran, what has focus),
because focus is a deterministic input the harness owns rather than app state,
so "unless the user is editing this field" is not expressible from state alone.

### Budgets are wall-clock, and say so

§22 enforces budgets "on a pinned reference machine class". Frame assertions
are therefore opt-in per test and should be set to catch a regression *in kind*
— the clip-mask regression put single frames past a second — not a few ms of
drift. Growth guards are the opposite: `.bounded("history", 500, …)` is a
property of the app, deterministic on every machine, and is the leak check that
belongs in a committed test. Process RSS is not.

## Reporting

A failure names the invariant, the step, the affordance, whether effects were
in flight, and the seed. A run prints frame p50/p95/max and **the interaction
behind the worst frame** — a budget failure that does not say what was slow is
not actionable.

## Stages

| # | Stage | State |
|---|---|---|
| A1 | `respond.rs`: `Reply`, responder plumbing, seeded shuffle, effect-cycle cap | done |
| A2 | `autopilot.rs`: discovery, bursts, invariants, bounded metrics, budgets | done |
| A3 | Self-guardrails in `libra-test/tests/autopilot.rs` (seed replays a run; the drain really reorders; a cycle is reported not hung) | done |
| A4 | First consumer: `applications/gloss/tests/stress.rs` | done |
| A5 | Second consumer once another app has effect-heavy flows | not started |

## What it found on the way in

Both were live bugs in `applications/gloss`, and both are the "app hangs"
class:

1. **A stranded follow-up.** Three call sites queued work behind an autosave
   and returned whatever `flush_save` returned. `flush_save` declines on any of
   three conditions, and `after_io` is the only thing that drains the queue —
   so a declined save left a click queued forever with no I/O in flight to pick
   it up. Fixed by one `save_then` helper that falls through.
2. **A draft that lagged its selection.** Found only by an `always` invariant:
   the value repaired itself when the read completed, so no settled check could
   ever see the window the next click acted inside.

## Ground rules

- Runs stay headless and deterministic — [[15.8-ci-shape|§15.8]] means no
  Xvfb, no GPU, no real binaries.
- Real-I/O soak runs are `#[ignore]`d, never in the default suite. They exist
  for bugs *inside* the effects runtime, where a fixture cannot see the failure
  (a pipe that deadlocks). `libra-effects` tests its own I/O; app tests do not.
- Invariants describe the app, not the fixture. If a check needs the autopilot
  to type a magic string to stay true, it is the wrong check.

## In code

- `crates/libra-test/src/autopilot.rs` — discovery, run loop, budgets
- `crates/libra-test/src/respond.rs` — `Reply`, seeded PRNG, effect labels
- `crates/libra-test/tests/autopilot.rs` — the harness's own guardrails
- `applications/gloss/tests/stress.rs` — first consumer + the opt-in soak

← [[Plans Index]] · [[15.7-fuzzing-property-tests|§15.7]] · [[22-performance-budgets|§22]] · [[12.3-execution-model|§12.3]]
