---
title: Operations
type: spec
status: draft
created: 2026-08-24
tags: [gloss, engineering]
---

# Operations

BYOK and no backend means there is no infrastructure to operate. Operations here
means the things that keep a local app trustworthy on someone's real data — and
"someone's real data" is the entire product surface.

## Key management

- **macOS Keychain, always.** `security-framework` crate, one item per provider.
  There is no Libra Keychain effect yet. Do not put keys on a Process argv
  so the CLI can "just read them." See [[10-libra]].
- **Never a config file inside the vault.** People commit their vaults. A key in
  `.gloss/config.json` reaches GitHub within a week of the second user.
- Keys are never logged, never included in crash reports, never in patch
  provenance. Redact by type at the logging boundary, not by pattern-matching
  strings.
- Validate on entry with a minimal request so a bad key fails at setup rather
  than three minutes into a background pass.

## Sensitive-vault mode

A per-workspace flag that pins the provider to the local endpoint and refuses to
start any run that would leave the machine. Hard failure, not a warning.

This exists because my own vault contains work material, and export-controlled or
otherwise restricted notes cannot transit a third-party API. Without this flag I
can't dogfood on the vault I actually care about — which makes it a v1 feature,
not a compliance afterthought. Indicate it persistently in the UI, not in a
settings pane nobody reopens.

## Cost control

The user's money, so the constraints are theirs to see.

- **Per-feature spend, visible.** A ledger view: what Tab cost this week, what the
  background passes cost, what one inbox-amnesty run cost.
- **Budget ceilings** — daily and monthly, per workspace. On breach, background
  work stops and interactive work warns. Never silently keep spending.
- **Estimate before expensive runs.** "This will process 340 notes, roughly
  1.2M tokens, about $X." Confirm before starting.
- **Prompt caching is the main lever.** Structure prompts so the stable vault
  context is a cacheable prefix. Track and display cache hit rate — a falling
  hit rate is a bug, not a fact of life.
- **The local gate does the real work.** Under 5% of touched notes should ever
  reach a frontier model. See [[02-architecture#L1 — Index]].

## Rate limits and degradation

Rate limits are now the *user's* tier, not yours, and a Tier 1 key will hit walls
that testing never sees.

- Queue background work with exponential backoff and jitter
- Respect `retry-after`; never hot-loop a 429
- Background work degrades quietly — a queued pass that's waiting is not an error
  dialog. Interactive work (Tab, Cmd+K) surfaces the limit immediately, because
  the user is sitting there.
- Interactive requests preempt background ones on the same key

## Data safety and recovery

The failure mode that kills the product is corrupting a vault. Everything below
is load-bearing.

- **Git checkpoint per run.** If the vault isn't a repo, offer to initialize a
  shadow repo; if declined, disable proactive features rather than run unsafe.
- **`expect_hash` on every op** — see [[03-data-model#Patch and checkpoint format]].
- **Index corruption is recoverable by construction** — delete and rebuild. Test
  this path routinely; it will be used.
- **Never destructive by default.** Deletes are moves to `.gloss/trash/` with a
  30-day sweep.
- **Crash mid-apply must be safe.** Journal the patch before applying, mark
  complete after. On startup, resolve any incomplete entry — the ops are
  idempotent under retry by design.

## Telemetry

Opt-in, off by default, and say so plainly at first run. This audience
self-selected for local and no-vendor; default-on analytics is a betrayal of the
thing that got them here.

If enabled, and never including note content or paths:
- Crash reports with symbols
- Feature counters, patch accept/reject ratio, cache hit rate
- Index size buckets and performance timings

Accept/reject ratio is the single most valuable number in the product. It is the
only quality signal available on BYOK, and it should be visible to the user too —
it makes the agent's reliability legible instead of vibes.

## Support surface

Solo, so keep it cheap:

- **In-app diagnostics export** — versions, index stats, recent errors, redacted.
  One button, one file to attach.
- **Structured logs** to `~/Library/Logs/Gloss/`, rotated, note content excluded
- GitHub Issues as the only channel; a template that asks for the diagnostics file
- A one-page "how Gloss touches your files" doc — the answer to the first
  question every careful user will ask, and the audience is entirely careful
  users
