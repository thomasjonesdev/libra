---
title: "PLAN-NOTIFY"
aliases:
  - "PLAN-NOTIFY"
  - "Effect::Notify"
  - "desktop notifications"
tags:
  - plans
  - architecture
type: plan
---

# Desktop notifications (`Effect::Notify`)

**Status: planned.** Expands [[PLAN-NATIVE-INTEGRATIONS]] **N3** from a table
row into a stage plan, following that plan's own precedent for tray/menus
("separate small plans when needed"). It stays governed by
[[12.1-effect-vocabulary|§12.1]] and [[19-platform-integration|§19]]; N3 is now a
pointer here rather than a second description.

**Read before writing code:** [[12.1-effect-vocabulary|§12.1]] (the sketched
`Notify(Notification)` shape this refines), [[PLAN-NATIVE-INTEGRATIONS]] (design
rules 1–4 apply verbatim), [[19-platform-integration|§19]] (native chrome
policy), [[appendix-a-determinism-checklist|Appendix A]].

## Why it is more than a one-liner

`notify-rust` makes *showing* a notification trivial. Three things around it are
not, and each one is a design decision rather than a wiring task:

1. **A click has to become an Action.** A notification that cannot be acted on is
   a worse version of a log line. Routing an OS callback back into the one-way
   data flow is the same problem AccessKit `Click` already solved (§17: AT clicks
   enter through the effects sink as real actions) — reuse that path, do not
   invent a second one.
2. **Repeat notifications must replace, not stack.** The motivating consumer is
   a six-hourly "your AWS session needs attention" nudge
   ([[27.1-aws-sso-session-manager|§27.1]]). Left alone overnight, the naive
   version produces a dozen identical banners and trains the user to dismiss
   them. Replace-by-key is a requirement, not a polish item.
3. **It can legitimately fail, and must not be fatal.** No notification daemon
   on a headless Linux box, consent not granted on macOS, no bundle identity on
   Windows. An ops tool that panics or wedges because a banner did not appear is
   broken. Degradation is part of the contract.

## The shape

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    /// Replace-key. A second notify with the same tag *replaces* the
    /// first rather than stacking. `None` always shows a new banner.
    pub tag: Option<String>,
    pub title: String,
    pub body: String,
    pub urgency: Urgency,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Urgency { Low, Normal, Critical }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotifyResult {
    Shown,
    /// No mechanism on this host (headless, no daemon). Not an error the
    /// app should surface as a failure — it is an absent capability.
    Unsupported(String),
    /// The user or OS declined. Actionable: the app may offer to open
    /// system settings.
    PermissionDenied,
    Failed(String),
}

// In the Effect enum — a refinement of §12.1's `Notify(Notification)`:
Notify { note: Notification,
         /// Dispatched if the user activates (clicks) the notification.
         /// A plain action, not a ctor: there is no payload to carry.
         on_activate: Option<M>,
         on_done: Option<ActionCtor<NotifyResult, M>> }
```

**Shell-handled, not pool-handled.** Notification APIs are UI-affine on macOS
and Windows, and showing a banner does not block. So `Notify` follows
`SetTheme`/`ClipboardSet` — handled inline in the shell's dispatch, per
[[12.3-execution-model|§12.3]] — not the [[12.6-the-process-effect|§12.6]] worker
pool. In the headless harness it resolves to `Unsupported("headless")` unless a
test feeds otherwise.

**`on_activate` is an ordinary action.** It arrives through the effects sink,
possibly minutes later, possibly after the state that scheduled it has moved on.
So the action must be **idempotent and self-validating** — "clicking a stale
banner re-authenticates a session that is already fine" is a real bug class.
Write this into the plan's design rules, because it is invisible in the type.

## Platform reality

| OS | Mechanism | Constraint that leaks into other plans |
|---|---|---|
| Linux | `notify-rust` → D-Bus `org.freedesktop.Notifications` | Nothing; degrade to `Unsupported` with no daemon |
| macOS | `UNUserNotificationCenter` | Requires a **bundled, signed `.app`** and user consent. From `cargo run` it will not work — expected, not a bug |
| Windows | Toast (`WinRT`) | Requires a registered **AppUserModelID**; unregistered = silent failure |

Two of three need real app identity, which makes this the first plan with a hard
dependency on [[PLAN-PACKAGE]]. Sequence accordingly: NT1 targets Linux and
graceful degradation, NT3 lands macOS/Windows behind bundling. Do not let "it
doesn't work on my Mac in dev" turn into a redesign.

## Stages

| Stage | Deliverable | Gate |
|---|---|---|
| **NT1** | `Notification` / `Urgency` / `NotifyResult` + `Effect::Notify` in `libra-core` (+ hand-written `PartialEq` arm); shell handler via `notify-rust`; `Unsupported` on every host without a mechanism; §12.1 updated from the sketch to the shipped shape | `cargo test --workspace`; a banner appears on Linux |
| **NT2** | Activation routing: `on_activate` dispatched through the effects sink like an AccessKit `Click` (§17); raising/showing the window on activation is [[PLAN-RESIDENCY]] R3's `WindowOp`, referenced not duplicated | manual: click a banner, observe the action |
| **NT3** | `tag` replace semantics on all three platforms; macOS bundle + consent prompt; Windows AppUserModelID registration wired into `libra package` | repeat notify with one tag yields one banner, not N |
| **NT4** | Harness parity per [[PLAN-NATIVE-INTEGRATIONS]] rule 2: `fx::notify()` / `fx::notify_tag("session")`, `resolve_notify(effect, NotifyResult::Shown)`, `h.activate_notification(effect)` to drive `on_activate`. First consumer: `aws-login` "needs attention" ([[PLAN-AWS-LOGIN]] A8) | fixture-driven integration test; no test shows a real banner |

## Design rules

1. **Never fatal.** Every `NotifyResult` variant is a state the app can render;
   none of them may panic, block, or abort a flow. A failed banner does not fail
   the operation it was describing.
2. **Idempotent activation.** `on_activate` may arrive arbitrarily late, out of
   order ([[12.3-execution-model|§12.3]]), or after the app was restarted from a
   rehydrated snapshot. It must re-check state rather than assume it.
3. **Tag anything recurring.** Any notification emitted on a cadence carries a
   `tag`. Untagged is for one-shot, user-initiated results only.
4. **No secrets in a banner.** Notification text may be persisted by the OS,
   shown on a lock screen, and synced to other devices. Titles and bodies are
   subject to the same rule as [[12.6-the-process-effect|§12.6]] logging: the
   runtime cannot know what is sensitive, so apps must not put identities,
   ARNs, or tokens in them.
5. **Outside the paint contract.** Banners are OS chrome
   ([[19-platform-integration|§19]] policy), so they never affect snapshots. What
   *does* snapshot is the in-window state that motivated the notification.

## Out of scope

- Notification actions/buttons, replies, and grouped threads. `on_activate` is
  one binary "the user cared" signal; richer surfaces are a later refinement
  once an app needs one.
- Sounds, badges, and dock/taskbar counts — adjacent, but their own (small)
  concerns; badge counts belong with [[PLAN-RESIDENCY]] if anything.
- Scheduled/local notifications delivered while the app is *not* running. That
  is an OS-level scheduling capability, and for the aws-login case
  [[PLAN-RESIDENCY]] (stay resident) is the correct answer instead.
- Notification history / Do-Not-Disturb interrogation.

## In code

- *(planned)* `crates/libra-core/src/lib.rs` — NT1
- *(planned)* `crates/libra-platform/src/lib.rs` — NT1–NT3 (shell handler,
  alongside `SetTheme` / `ClipboardSet`)
- *(planned)* `crates/libra-test/src/lib.rs` — NT4
- `crates/libra-platform/src/a11y.rs` — the AT-click-through-the-sink precedent
  NT2 copies

---

← [[Plans Index]] · [[PLAN-NATIVE-INTEGRATIONS]] (N3) · [[12.1-effect-vocabulary|§12.1]] · [[19-platform-integration|§19]] · [[PLAN-RESIDENCY]]
