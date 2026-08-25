---
title: "PLAN-AWS-LOGIN"
aliases:
  - "PLAN-AWS-LOGIN"
  - "aws sso app plan"
  - "aws-login"
tags:
  - plans
  - architecture
type: plan
---

# AWS SSO session manager (and the process effect it needs)

**Status: v1 shipped (P1, P2, A1–A7 — 2026-07-27); A8–A10 planned behind
[[PLAN-NOTIFY]], [[PLAN-RESIDENCY]] and [[PLAN-STREAM-DELIVERY]].** Builds `applications/aws-login` — a Libra app that replaces
`applications/aws-login/refresh_aws_sso.sh`: onboarding once, one **Authenticate**
button after that, and a session kept alive on a six-hour cadence against an
eight-hour token.

**Read before writing code:** [[27.1-aws-sso-session-manager|§27.1]] (the
normative spec — state machine, command allowlist, scheduling rule, security),
[[12.6-the-process-effect|§12.6]] (the effect this depends on),
[[27-first-party-applications|§27]] (what `applications/` is),
[[appendix-a-determinism-checklist|Appendix A]].

## Why this is two plans in one

The app is small. The reason it does not exist yet is that **Libra cannot run a
subprocess.** The implemented effect vocabulary stops at Http / Kv\* / Timer /
Random / Ingest / SetTheme / ClipboardSet; `aws` is unreachable from an app that
honours [[05.3-purity-rules|§5.3]]. So P1 is a framework stage with its own
gates, and A1–A7 are the app on top of it. P1 lands with no reference to AWS at
all — it is a general capability that this app happens to be the first consumer
of, following the `ClipboardSet` template ([[PLAN-NATIVE-INTEGRATIONS]]).

Do not start A1 before P1 is green. There is no useful shim: the app *is* a
sequence of process descriptions.

## Stages

| Stage | Deliverable | Framework pull |
|---|---|---|
| **P1** | `Effect::Process` + `CancelProcess` per [[12.6-the-process-effect|§12.6]]: `CommandSpec` / `ProcessResult` / `ProcessStatus` in `libra-core` (+ the hand-written `PartialEq` arm), worker-pool executor in `libra-effects` (program allowlist, `PATH` resolution, filtered env, `timeout_ms` kill, cancel + zombie-drop), harness parity in `libra-test` (`fx::process()`, `fx::process_program(…)`, `resolve_process`). Unit tests for the allowlist/timeout/cancel paths. §12.6 flips to shipped. | all of it |
| **A1** | `applications/*` as workspace members ([[21-crate-layout|§21]]); `applications/aws-login` crate that builds and runs: `#[libra::state]` / `#[libra::actions]` / `app!`, one `.lx` view, KV-backed config load at init, "not configured" first frame + its snapshot | none |
| **A2** | Preflight: `aws --version` parsed for major 2 (blocking), `claude --version` (advisory), remediation copy, preflight gates every other mode | none |
| **A3** | Onboarding: the form, validation (12-digit account, https start URL, fragment warning, profile-name collision via `list-profiles`), the `aws configure set` sequence, KV write, **Copy command** escape hatch for `aws configure sso` | none |
| **A4** | Authenticate: `aws sso login --profile <p>` with a generous timeout → `aws sts get-caller-identity --profile <p>` → `Active`; stderr surfaced verbatim on failure; **Cancel** via `CancelProcess` | none |
| **A5** | Steady state + scheduler: `<Interval every={60000} onTick={tick}/>`, startup probe, probe cadence, the six-hour nudge, auto-refresh `Switch` (default on), "needs attention" state | none |
| **A6** | Config surface: `aws configure get` read-back + `list-profiles` → `config_rows` projection, **Show config**, **Re-run setup**, `export AWS_PROFILE=` to clipboard (script `--export` parity) | none |
| **A7** | Full ladder per [[27.1-aws-sso-session-manager|§27.1]] *Testing* + all seven snapshots + README with hashes; the bash script is marked superseded | none |
| **P2** *(shipped with P1)* | Wall-clock stamp on effect results (`ProcessResult.finished_at_unix_ms`), making [[05.3-purity-rules|§5.3]]'s `received_at` real. Enables an honest "session data is N hours old" after a sleep instead of waiting one probe interval. **Not** a correctness dependency — the probe already covers it | libra-core + libra-effects + harness |

### Beyond v1

Each of the three v1 holes is a *framework* gap with its own plan; the stage
below is only this app's consumption of it. Do not build any of the capabilities
inside `applications/aws-login` — the whole point of `applications/`
([[27-first-party-applications|§27]]) is that a real app's needs pull the
framework forward in a reusable shape.

| Stage | Deliverable | Blocked on |
|---|---|---|
| **A8** | Notify on "needs attention": one `Effect::Notify` when the probe finds a dead session or the six-hour nudge fires, `tag: "aws-session"` so an unattended overnight run produces **one** banner rather than a dozen, `on_activate` raising the window. The banner text names no profile, account, or ARN ([[PLAN-NOTIFY]] rule 4) | [[PLAN-NOTIFY]] NT1–NT4 |
| **A9** | Residency, which is what actually closes the "app must stay running" hole: tray status icon (`Ok` / `Warn` / `Error` from `AuthStatus`), menu of Authenticate / Show window / Quit, `CloseToTray`, single instance, and a launch-at-login `Switch` in the config panel reading real on-disk state via `LoginItemQuery` | [[PLAN-RESIDENCY]] R1–R5 |
| **A10** | Device-code login in-app, replacing the clipboard hand-off: `Effect::Spawn` of `aws sso login --use-device-code --no-browser`, `<Stream>` mounted in the same commit, the verification URL and user code parsed out of the streamed lines and shown as selectable text + **Copy code**; Cancel closes the stream and kills the child. **Verify the CLI streams over a pipe before designing the screen** — if it gates output on a TTY, cut the stage rather than reaching for a PTY | [[PLAN-STREAM-DELIVERY]] D1–D5 |

## Execution notes

1. **P1 first, and prove it without AWS.** Its integration test should run
   something boring and portable, not `aws`. The allowlist is per-application
   configuration; the effect itself is generic.
2. **A3 and A4 are where the script's behavior must be matched exactly.** Diff
   the emitted argv against `bootstrap_sso_config` and `run_refresh_for_profile`
   in `refresh_aws_sso.sh` line by line. The integration test that asserts the
   argv sequence is the equivalence proof, and it is cheap to write.
3. **A5 is the stage most likely to be got wrong.** Re-read
   [[27.1-aws-sso-session-manager|§27.1]] *Scheduling* before writing it. The
   failure mode — deadline arithmetic on session-relative `t_ms` reporting a
   healthy session after the machine slept — is silent, and it defeats the
   entire point of the app.
4. **Snapshots use invented identities.** Set them up in A1 so no real ARN ever
   reaches a fixture, rather than scrubbing later.
5. **The window is small.** This is a utility, not a dashboard; a single narrow
   column at a fixed size keeps the snapshot set cheap.

## Acceptance

- [ ] `Effect::Process` is describable, interceptable, resolvable with a fixture,
      and cancellable — with no `std::process::Command` outside `libra-effects`
- [ ] A cold machine reaches a working AWS session through the GUI alone: install
      check → onboarding → authenticate → verified identity
- [ ] Steady state is one button, and the session survives six hours unattended
      when the refresh token allows it
- [ ] A session that died while the machine slept is reported as dead within one
      probe interval — never as `Active`
- [ ] `Show config` reports what is on disk (via `aws configure get`), and
      `Re-run setup` returns to onboarding without losing the ability to cancel
- [ ] The emitted argv for onboarding and refresh matches `refresh_aws_sso.sh`,
      asserted in an integration test
- [ ] No credential, token, or identity reaches KV, a log, or a committed PNG
- [ ] `cargo test --workspace` green; every screen snapshot committed with its
      README hash

## Out of scope

- PTY allocation and interactive prompt-answering — still
  [[12.6-the-process-effect|§12.6]]'s non-goal, and the risk A10 must retire
  before it starts
- Generalising this into a credential broker for other services
- Assuming an OS can wake or schedule the app while it is not running. A9's
  answer is "be resident", not "be scheduled" ([[PLAN-NOTIFY]] *Out of scope*)
- Anything in [[27.1-aws-sso-session-manager|§27.1]] *Out of scope (v1)* that
  A8–A10 do not name

## In code

- `crates/libra-core/src/lib.rs`, `crates/libra-effects/src/lib.rs`,
  `crates/libra-test/src/lib.rs` — P1 + P2, shipped
- `applications/aws-login/` — A1–A7, shipped
- `applications/aws-login/refresh_aws_sso.sh` — behavioral reference
- *(planned)* A8 → [[PLAN-NOTIFY]] · A9 → [[PLAN-RESIDENCY]] ·
  A10 → [[PLAN-STREAM-DELIVERY]]

---

← [[Plans Index]] · [[27.1-aws-sso-session-manager|§27.1]] · [[12.6-the-process-effect|§12.6]] · [[PLAN-NOTIFY]] · [[PLAN-RESIDENCY]] · [[PLAN-STREAM-DELIVERY]]
