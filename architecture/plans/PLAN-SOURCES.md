---
title: "PLAN-SOURCES"
aliases:
  - "PLAN-SOURCES"
  - "§12.5 Interval Stream"
tags:
  - plans
  - architecture
type: plan
---

# Interval / Stream Source Elements Plan

**Status: complete.** Landed 2026-07-16. `Instant` + `action_with_instant`;
`<Interval every={ms} onTick={…}>` with shell recurring timers; `<Stream>`
grammar + mount/query (event delivery later). Conformance `interval_stream`
fixture; `cargo test --workspace` green.

**Read:** [[12.5-declarative-subscriptions-effects-as-mount-scoped-data|§12.5]], [[12.1-the-effect-vocabulary-closed-versioned|§12.1]], [[12.2-subscriptions|§12.2]], [[PLAN-IF]].

## Interval (full v1)

```lx
<Interval every={1000} onTick={poll} />
```

- `every`: numeric literal (ms) — binding deferred with accessors plan
- `onTick`: `Instant`-payload action (`t_ms: f64`)
- Shell: recurring timer; identity `(interval, every_ms, handler)`; unmount cancels
- Harness: `by::source("interval")`; tests dispatch Instant actions directly
- Conformance fixture

## Stream (grammar + mount v1)

```lx
<Stream id={feed} onEvent={packet} />
```

- Parses/lays out/queryable as `by::source("stream")`
- `id`: string binding; `onEvent`: String-payload action (event body as text for v1)
- Full Ws/Spawn delivery remains later (§12.1) — shell records mounted streams for the differ; event injection via harness `resolve` path when wired

## Acceptance

- [ ] Conformance: Interval (+ Stream nodes) interp≡codegen
- [ ] Shell ticks Interval while mounted; cancel on unmount / `<if>` flip
- [ ] `cargo test --workspace` green

---

← [[Architecture Home]] · [[Plans Index]] · governing [[12.5-declarative-subscriptions-effects-as-mount-scoped-data|§12.5]]
