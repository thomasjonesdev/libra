---
title: "PLAN-STREAM-DELIVERY"
aliases:
  - "PLAN-STREAM-DELIVERY"
  - "Effect::Spawn"
  - "stream delivery"
tags:
  - plans
  - architecture
type: plan
---

# Streaming effect results: `Effect::Spawn` + `<Stream>` delivery

**Status: D1–D6 shipped.** Finishes the half of [[PLAN-SOURCES]] that was deferred:
`<Stream id={…} onEvent={…}/>` parses, lays out, mounts and answers
`by::source("stream")` today, but **nothing is ever delivered to it.** This plan
gives streams a producer (`Effect::Spawn`) and a delivery path, and makes
"incremental output from a long-running thing" a first-class, testable shape.

**Read before writing code:** [[12.5-declarative-subscriptions|§12.5]] (mount =
subscribe), [[12.6-the-process-effect|§12.6]] (the run-to-completion sibling and
its security posture, all of which applies unchanged),
[[12.3-execution-model|§12.3]] (ordering guarantees — this plan *strengthens*
one), [[12.1-effect-vocabulary|§12.1]], [[appendix-a-determinism-checklist|Appendix A]].

## Why this exists

`Effect::Process` ([[12.6-the-process-effect|§12.6]]) returns one
`ProcessResult` when the child exits. That is the wrong shape whenever the
*interesting output arrives before the process finishes*. The motivating case is
concrete: `aws sso login --use-device-code --no-browser` prints a verification
URL and a user code to stdout and then blocks for up to a few minutes waiting
for the user to visit it. With `Process`, the app sees the code **after** the
login it was needed for has already timed out. This is why
[[27.1-aws-sso-session-manager|§27.1]] shipped v1 with a clipboard hand-off
instead of an in-app device-code screen.

Note carefully what is *not* required: a PTY. The device-code flow writes to a
pipe and needs no terminal, so it stays inside
[[12.6-the-process-effect|§12.6]]'s "drive the non-interactive equivalent" rule.
**D6 must verify this against the real CLI before the screen is designed** — if
that binary turns out to gate its output on `isatty`, the stage becomes a PTY
question and should be cut rather than smuggled in.

The secondary payoff is general: log tailing, `cargo build` progress, `kubectl
logs`, and eventually the `WsConnect`/`WsSend` pair in
[[12.1-effect-vocabulary|§12.1]] all want exactly this vocabulary.

## The shape

One **string-keyed** stream namespace, so that the markup form and the effect
form name the same thing. The app chooses the name; `<Stream id={…}>` already
takes a string binding.

```rust
/// One event from an open stream. Ordinary action payloads — replay,
/// time travel and sequence fuzzing see them like every other action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEvent {
    /// One complete line, terminator stripped. The default: pipes from
    /// CLIs are line-oriented and apps should not re-implement splitting.
    Line(String),
    /// Raw bytes, for a source that is not line-oriented.
    Chunk(Vec<u8>),
    /// The runtime dropped `n` events because no handler was mounted and
    /// the buffer was full. Explicit, because silent loss of a device
    /// code is exactly the bug this plan must not ship.
    Dropped(u32),
    /// Source ended. Terminal — no further events for this id.
    Closed(StreamClose),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamClose {
    /// Spawn-backed: the child exited. Same status vocabulary as §12.6.
    Process(ProcessStatus),
    Cancelled,
    Failed(String),
}

// In the Effect enum:
Spawn       { stream: String, command: CommandSpec, capture: Capture },
CloseStream { stream: String },

/// Which pipes feed the stream. stderr is opt-in and *tagged* rather than
/// interleaved blindly, because "was this line an error?" is information
/// the app usually needs and can never recover afterwards.
pub enum Capture { Stdout, Stderr, Both }
```

`Spawn` reuses `CommandSpec` verbatim, which means it inherits every normative
rule in [[12.6-the-process-effect|§12.6]] for free: argv never a shell line,
allowlisted program, filtered environment, output never logged by the runtime.
**Do not introduce a second command type.** `timeout_ms` still applies and still
produces `Closed(Process(TimedOut))`.

## The two decisions worth arguing about

**1. Buffer until a handler mounts.** The shell's commit order is `reduce` →
execute effects → `relayout` (which collects mounted sources). So an app that
flips `logging_in = true` and returns `Effect::Spawn` in the same action has its
child running *before* the `<Stream>` inside `<if test={logging_in}>` exists. A
naive "drop events with no handler" rule loses the first lines — including the
device code. Therefore: the runtime holds a **bounded per-stream ring** (256
events) from spawn onward, and on the first commit where a handler is mounted it
replays the buffer in order before live delivery. Overflow with no handler
mounted coalesces into one `Dropped(n)`. Mount-after-the-fact is thus safe, and
loss is always *reported*, never silent.

**2. Ordering already covers this, but only implicitly.**
[[12.3-execution-model|§12.3]] promises that "actions from one effect arrive in
causal order" while guaranteeing nothing *across* effects. `Spawn` is the first
effect that emits **many** actions, so that clause stops being a formality: a log
with shuffled lines is not a log. D1 makes it explicit for multi-event effects
rather than leaving tests to depend on an unstated reading. Cross-stream ordering
stays unguaranteed, and the fuzz harness ([[15.7-fuzzing-property-tests|§15.7]])
should exploit that as it does today.

Unmount cancels, per [[12.5-declarative-subscriptions|§12.5]]: when the last
`<Stream>` for an id disappears (an `<if>` flip, a hot reload, a screen change),
the stream closes and a Spawn-backed child is **killed**. Otherwise a cancelled
login leaks a process that still holds the SSO poll open. `CloseStream` is the
explicit form of the same thing.

## Stages

| Stage | Deliverable | Gate |
|---|---|---|
| **D1** | `StreamEvent` / `StreamClose` / `Capture` + `Effect::Spawn` / `CloseStream` in `libra-core` (+ the hand-written `PartialEq` arms — [[12.1-effect-vocabulary|§12.1]] gotcha); §12.3's per-effect ordering clause made explicit for multi-event effects; §12.6's "out of scope" line retired | compiles, vocabulary note updated |
| **D2** | Executor in `libra-effects`: stream registry keyed by name, dedicated thread per stream (a blocking pipe read must not hold a pool worker — the Timer precedent), line splitting with a max line length, `Capture` tagging, bounded ring + `Dropped(n)`, child reap on close, kill on `CloseStream`. Unit tests for buffer replay, overflow, and kill-on-close | `cargo test -p libra-effects` |
| **D3** | Delivery in `libra-platform`: `markup_streams` (already collected per commit) becomes a real differ — newly-mounted ids drain their buffer then receive live events as dispatched actions; unmounted ids close. Hidden/​windowless shells still deliver (see [[PLAN-RESIDENCY]]) | a manual example streams `Both` from a boring portable program |
| **D4** | Harness parity per [[PLAN-NATIVE-INTEGRATIONS]] rule 2: `fx::spawn()` / `fx::spawn_program("aws")`, `h.emit_stream(id, StreamEvent::…)` (asserts a handler is mounted, then dispatches), `h.close_stream(id, …)`. Integration test proving buffered-then-live ordering | `cargo test -p libra-test` |
| **D5** | **Shipped.** `Sub::Stream` unions with markup `<Stream>`. Markup keeps the handler when both name an id | `union_stream_handlers` test; no grammar change |
| **D6** | **Shipped.** aws-login device-code screen. The child is `Effect::Spawn` with `--use-device-code --no-browser`. Lines are parsed for a URL and a user code. There is no PTY. A CLI that withholds text until a TTY is attached leaves the code empty and the close path reports that sign-in did not finish | fixture test; no real `aws` |

## Acceptance

- [ ] A stream spawned in the same action that mounts its `<Stream>` loses no
      events — asserted, not assumed
- [ ] Dropped events are always reported as `Dropped(n)`; the runtime never
      silently discards output
- [ ] Events within one stream arrive in source order, and
      [[12.3-execution-model|§12.3]] says so in as many words for multi-event
      effects; cross-stream ordering is still not promised
- [ ] Unmounting the `<Stream>` kills a Spawn-backed child — no leaked process
      after a cancelled flow, verified on a real run
- [ ] Every stream test runs on hand-fed fixtures; no test spawns a real
      long-running binary
- [ ] Snapshots stay byte-exact — streamed bytes reach pixels only through State
      set by a result action ([[appendix-a-determinism-checklist|Appendix A]])
- [ ] `cargo test --workspace` green

## Out of scope

- **PTY allocation** and interactive prompt-answering — still
  [[12.6-the-process-effect|§12.6]]'s non-goal, and the reason D6 has a
  verification step rather than an assumption.
- `stdin` writes to a running stream. Deliberately deferred: it turns `Spawn`
  into a bidirectional channel and deserves its own design pass.
- `WsConnect` / `WsSend`. The vocabulary here is built so websockets slot onto
  the same rails, but the transport is a separate plan.
- Process groups, supervision, restart policies.
- Rendering streamed output efficiently. A device-code screen shows two lines; a
  real log viewer should feed `LogView` through the [[14-data-plane|§14]] data
  plane, not accumulate a `Vec<String>` in State.

## In code

- `crates/libra-core/src/lib.rs` — D1
- `crates/libra-effects/src/lib.rs` — D2 (`stream.rs`)
- `crates/libra-platform/src/lib.rs` — D3 (`markup_streams` drains the ring)
- `crates/libra-test/src/lib.rs` — D4
- `crates/libra-components/src/catalog/source.rs` — `<Stream>` mount node, shipped

---

← [[Plans Index]] · governing [[12.5-declarative-subscriptions|§12.5]] · [[12.6-the-process-effect|§12.6]] · [[PLAN-SOURCES]] · [[PLAN-AWS-LOGIN]]
