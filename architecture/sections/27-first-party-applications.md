---
title: "27. First-Party Applications"
section: "27"
aliases:
  - "§27"
  - "27"
  - "27. First-Party Applications"
tags:
  - architecture
  - section/27
type: architecture
---

# 27. First-Party Applications

## In this vault

- [[27.1-aws-sso-session-manager]]

## In code

- `applications/` — first-party Libra apps built for real daily use
- `examples/` — framework demonstrations (unchanged; see [[21-crate-layout|§21]])

---

`examples/` exists to exercise the framework. Each one is chosen to prove a
capability: `counter` proves the loop, `dashboard` proves the data plane,
`notes` proves text entry, `todos` proves item fields, `gallery` proves the
catalog. Their job is coverage, and it is fine for them to be small.

`applications/` is for the other direction — **software someone actually uses
to get work done**, which happens to be written in Libra. The distinction
matters because the two answer to different pressure. An example may stop at
the feature it was built to demonstrate. An application cannot: it meets a real
operator, a real OS, a real external service that changes under it, and it is
judged on whether the task got done.

That pressure is the point. A framework validated only by its own examples
drifts toward what is convenient to demonstrate. First-party applications are
the forcing function that finds the missing effect, the missing component, the
missing clock — and they find them as concrete, blocking gaps rather than as
speculation. [[27.1-aws-sso-session-manager|§27.1]] is the first, and it
immediately surfaced two: no way to run a subprocess
([[12.6-the-process-effect|§12.6]]) and no wall-clock time in a payload.

## Rules

An application in `applications/` is a normal Libra app with no special
dispensations. Specifically:

1. **Same contract, no exceptions.** One `State`, closed actions, effects as
   data, deterministic paint. The determinism checklist
   ([[appendix-a-determinism-checklist|Appendix A]]) applies exactly as it does
   to a framework crate. "It's just a tool for me" is not a reason to shell out
   from `update`.
2. **Gaps become framework work, not workarounds.** When an application needs
   something the framework lacks, the answer is a governed change to the
   framework — a §12.1 vocabulary extension, a catalog component, a grammar
   feature through both engines. It is never a private escape hatch inside the
   app. If the gap is too large to close now, the application waits; that is
   information, not an obstacle.
3. **Full test ladder.** `tests/unit.rs`, `components.rs`, `integration.rs`,
   `snapshots.rs` per [[15-testing|§15]] — one file per kind, same as examples.
   An application's flows run against intercepted effects and fixture results;
   no test touches the real external service.
4. **Fixtures, never real credentials or identities.** Snapshots and test
   fixtures use invented account numbers, ARNs, hostnames, and users. A
   committed PNG is a cross-machine determinism proof; it must not also be a
   disclosure.
5. **`applications/*` are workspace members** and follow [[21-crate-layout|§21]]
   dependency rules — every dep declared once in root `[workspace.dependencies]`.
6. **`applications/` is not a dumping ground for scripts.** A shell script that
   an application supersedes may live beside it during migration, but it stops
   being the thing anyone runs once the app ships.

## Relationship to §5.9

Applications reach the size where module layout matters sooner than examples do.
The split is the same one: a single file until `lib.rs` stops fitting in one
read, then exactly `model.rs` / `state.rs` / `actions.rs` / `lib.rs`-as-wiring
per [[05.9-module-layout|§5.9]], never changing a test import in the process.
`examples/notes` remains the reference shape.

---

← [[Architecture Home]] · [[Map of Content]]
