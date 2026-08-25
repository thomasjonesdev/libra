---
title: "PLAN-RESIDENCY"
aliases:
  - "PLAN-RESIDENCY"
  - "tray"
  - "menubar"
  - "launch at login"
tags:
  - plans
  - architecture
type: plan
---

# Background residency: tray/menubar, window policy, launch at login

**Status: planned.** Makes a Libra app able to *live* on a desktop rather than
be launched at: a tray/menubar presence, a window that can close without
quitting, one instance rather than five, and optional start-at-login. Claims the
"Menus / tray / hotkeys — separate small plans when needed" row of
[[PLAN-NATIVE-INTEGRATIONS]] and the `tray-icon`/`muda` rows of the
[[19-platform-integration|§19]] table.

**Read before writing code:** [[19-platform-integration|§19]] (native chrome vs
Libra-rendered content — the policy this plan lives or dies by),
[[19.2-keyboard-shortcuts-declarations-not-handlers|§19.2]] (the
declarations-not-handlers precedent, and the one place this plan departs from
it), [[20-packaging|§20]] + [[PLAN-PACKAGE]] (app identity),
[[12.5-declarative-subscriptions|§12.5]] (what must keep running while
windowless), [[appendix-a-determinism-checklist|Appendix A]].

## Why it exists

[[27.1-aws-sso-session-manager|§27.1]] shipped with an honest hole: *the app
must stay running for its own six-hour cadence to fire.* A session manager you
have to remember to launch is a session manager that does not manage anything.
The fix is not more scheduling logic — it is residency, and residency is four
distinct capabilities that are usually conflated:

| Capability | Without it |
|---|---|
| Tray/menubar presence | A resident app with no window is invisible and unquittable |
| Close ≠ quit | Closing the window kills the cadence (the shell calls `event_loop.exit()` on `CloseRequested` today) |
| Single instance | Autostart plus a user double-click = two trays, two schedulers, two logins |
| Launch at login | The user must remember, daily, forever |

Every one of them is generic. Nothing in this plan mentions AWS.

## The one interesting design question

A tray menu is native chrome, so it is outside the paint contract
([[19-platform-integration|§19]] policy) — no DrawCmds, no snapshot. But its
items still have to be **data**, and clicking one still has to become an
**Action**, or the one-way data flow has a hole in it.

[[19.2-keyboard-shortcuts-declarations-not-handlers|§19.2]] made shortcuts a
*static* table on purpose: "state-dependent enablement is the action's job, not
the table's." **The tray deliberately differs**, and the reason is worth stating
because it looks like an inconsistency: a shortcut has no visible
representation, so there is nothing about it that state could change. A tray
*is* a display surface — its icon shows session status and its top item reads
"Signed out" or "Active until 18:04". A static table cannot express that, and
pushing it into actions would mean the tray shows stale text.

So: **the tray is a second view surface — a pure projection of State, rebuilt
and diffed by the runtime on every commit, exactly like the laid tree.**

```rust
pub struct TrayDecl {
    pub tooltip: String,
    /// Closed set mapped to bundled assets — not an arbitrary path, so a
    /// tray icon can never be an environment-dependent input.
    pub icon: TrayIcon,          // Default | Ok | Warn | Error
    pub items: Vec<TrayItem>,    // Label | Action { text, action, enabled } | Check { … } | Separator
}

impl AwsLogin {
    fn tray(&self) -> TrayDecl { /* pure */ }
}

libra::app! { state: AwsLogin, /* … */ tray: AwsLogin::tray }
```

Action names reflect through `action_by_name`, validated **at startup** like
markup handlers and the shortcut table — a typo is a launch-time error naming
the bad item, not a dead menu entry discovered in six weeks.

**Considered and rejected: a `<TrayMenu>` element in `.lx`.** It is tempting
(source elements already prove that non-painting markup nodes are fine). But the
markup pipeline is `AST → ViewNode → taffy → LaidTree` — it is fundamentally
about *geometry*, and a native menu has none. Source elements get away with it
because they are inert leaf markers keyed by props; a menu has nesting, labels
and checked state, i.e. a whole parallel non-geometric tree inside a grammar
that exists to describe layout. A typed Rust projection keeps it data, keeps it
diffable, and keeps it testable without inventing that second tree. If a `.lx`
form is ever wanted it can compile *to* `TrayDecl`; nothing here forecloses it.

## Stages

| Stage | Deliverable | Gate |
|---|---|---|
| **R1** | `TrayDecl` / `TrayItem` / `TrayIcon` in `libra-core` (pure — no `tray-icon` types); `App::tray()` (default `None`) + `tray:` field in `app!` mirroring `shortcuts:`; startup validation of every action name; shell builds and **diffs** the decl per commit via `tray-icon` + `muda`; clicks dispatch through the effects sink like an AccessKit `Click` | a tray icon whose label tracks state, on macOS + Linux |
| **R2** | Harness parity: `h.tray()` returns the decl for assertions, `h.click_tray("authenticate")` dispatches (erroring on an unknown or disabled item). Tray behavior becomes component-testable with **no OS involved** | `cargo test -p libra-test`; a tray test in an example app |
| **R3** | Window policy: `Options::window_policy` (`QuitOnClose` — today's behavior, the default — vs `CloseToTray`); `Effect::WindowOp { Show, Hide, Raise, Quit }`; macOS `ActivationPolicy::Accessory` so a menubar app stays out of the Dock. **The load-bearing invariant: hiding the window must not unmount sources.** `<Interval>` threads and `subscriptions()` keep running while windowless; only painting stops | closing the window leaves the process and its intervals alive; `Quit` from the tray exits and runs the save hook |
| **R4** | Single instance: an OS-level guard (lock file in the app-data dir on unix, named mutex on Windows). A second launch does not start a shell — it signals the first to `Raise` and exits 0. Opt-in via `Options` | launching twice yields one window and one tray |
| **R5** | `Effect::LoginItem { enabled, on_done }` + `LoginItemQuery` (so a settings switch can render the *actual* state rather than a remembered one) + `LoginItemResult`. Per-OS: XDG autostart `.desktop` (Linux), Run key / Startup folder (Windows), LaunchAgent / `SMAppService` (macOS — needs a bundle, so gated on [[PLAN-PACKAGE]]). `auto-launch` is the candidate crate; verify its macOS path before committing to it | toggling the switch survives a reboot on Linux + Windows |
| **R6** | First consumer: `aws-login` residency ([[PLAN-AWS-LOGIN]] A9) — status icon, menu (Authenticate / Show window / Quit), `CloseToTray`, single instance, autostart switch in the config panel | its cadence fires unattended with no window open |

## Overlaps to resolve, not duplicate

- **`WindowOp`** is [[PLAN-NATIVE-INTEGRATIONS]] **N6**. R3 needs only
  `Show`/`Hide`/`Raise`/`Quit`; N6 adds title/fullscreen/resize. Build one
  effect variant in whichever plan lands first and have the other consume it —
  two `WindowOp`s would be a defect.
- **Menubar / context menus** are **N7** (`muda` for in-window chrome). R1
  already pulls `muda` in for the tray menu; N7 reuses that dependency and its
  action-name validation rather than re-deriving them.
- **Notification activation** ([[PLAN-NOTIFY]] NT2) wants to raise the window.
  That is R3's `WindowOp::Raise`. Sequence R3 before NT2, or NT2 ships without
  raise and gains it later.
- **App identity** ([[PLAN-PACKAGE]]) blocks R5 on macOS and NT3 on
  macOS/Windows. These are the first two plans that make packaging a
  prerequisite rather than a finishing step; [[PLAN-PACKAGE]] should record that.

## Determinism

Nothing here touches the paint path. The tray, its menu, and the Dock/taskbar
presence are OS chrome ([[19-platform-integration|§19]]); snapshots are
unaffected and no new bytes enter `libra-raster`. What *is* on the determinism
contract is the `TrayDecl` itself: a pure function of State, asserted as data in
tests, with a closed icon set so no filesystem path or theme lookup can make it
environment-dependent. Tray clicks and login-item results are ordinary actions
in the log, replayable like a button press.

## Acceptance

- [ ] A resident app with no window still ticks its `<Interval>` and still
      answers its tray menu — asserted on a real run, since no harness can prove it
- [ ] The tray decl is a pure projection of State, and every tray flow is
      testable through `h.click_tray(…)` with no OS
- [ ] A bad action name in a `TrayDecl` fails at startup, naming the item
- [ ] Closing the window under `CloseToTray` neither quits nor leaks; `Quit`
      runs the rehydration save hook (§18.2) exactly as `CloseRequested` does today
- [ ] A second launch raises the first instance instead of racing it
- [ ] The autostart switch reflects on-disk reality (via `LoginItemQuery`), not a
      value the app remembered
- [ ] `QuitOnClose` remains the default — no existing example changes behavior
- [ ] `cargo test --workspace` green; no snapshot re-validated by this plan

## Out of scope

- **Global hotkeys** (`global-hotkey` in the §19 table). Adjacent and small, but
  a different input path with its own conflict-resolution problem; a separate
  plan when an app needs one.
- Dock/taskbar badges and progress bars.
- Auto-update, still a [[24-non-goals|§24]] non-goal; R5 is start-at-login only.
- Running as a system service/daemon with no user session. A tray app is a
  *user-session* resident; headless scheduling is a different product.
- Rich tray UI (popover windows hanging off the menubar icon). A menu of items
  plus "Show window" is the whole surface here.

## In code

- *(planned)* `crates/libra-core/src/lib.rs` — `TrayDecl`, `App::tray`,
  `Effect::WindowOp` / `LoginItem` (R1, R3, R5)
- *(planned)* `crates/libra-reflect/src/app.rs` — `tray:` field, alongside
  `shortcuts:` (R1)
- *(planned)* `crates/libra-platform/src/lib.rs` — tray build/diff, window
  policy, `CloseRequested` branch (currently an unconditional
  `event_loop.exit()`), single-instance guard (R1, R3, R4)
- *(planned)* `crates/libra-test/src/lib.rs` — `h.tray()` / `click_tray` (R2)

---

← [[Plans Index]] · governing [[19-platform-integration|§19]] · [[19.2-keyboard-shortcuts-declarations-not-handlers|§19.2]] · [[20-packaging|§20]] · [[PLAN-NATIVE-INTEGRATIONS]] · [[PLAN-NOTIFY]] · [[PLAN-AWS-LOGIN]]
