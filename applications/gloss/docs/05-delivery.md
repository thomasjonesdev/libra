---
title: Delivery
type: spec
status: draft
created: 2026-08-24
tags: [gloss, engineering]
---

# Delivery

## Repo layout

Gloss lives in the Libra workspace under `applications/gloss`.

```
applications/gloss/
  crates/gloss-core/         vault, patch, parse — no UI types
  crates/gloss-cli/          list / cat / apply / undo
  src/ + views/gloss.lx      Libra editor shell
  vault-fixtures/            golden vaults
  docs/                      these notes
```

Later crates (`gloss-agent`, `gloss-llm`, `gloss-mcp`) stay in this tree when
they land. **`gloss-core` must not depend on `libra-platform`.** That is what
lets M2 ship without a UI.

## Toolchain

| | |
| --- | --- |
| Rust | stable, pinned via `rust-toolchain.toml` |
| Frontend | Libra `.lx` + `<Textarea>` |
| Lint/format | `clippy -D warnings`, `rustfmt` |
| Shell | `libra-platform` |
| Swift | SwiftPM, macOS 14.2 deployment target |
| Task runner | `just` |

Given the [[oxc-rolldown-adoption|oxc/Rolldown]] work already underway, use
oxlint here from the start — a greenfield repo is the cheapest possible place to
validate that toolchain before it goes near anything at work.

## Testing strategy

Five tiers, mirroring the [[libra-ui-framework|Libra]] architecture, because the
failure mode here is identical: silent, compounding corruption of state the user
can't easily inspect.

**T1 — Unit.** Parser, chunker, path resolution, ranker scoring.

**T2 — Property.** `proptest` on the patch layer, and this is the tier that
matters most:
- apply-then-inverse is the identity, for any op sequence
- op application is idempotent under retry
- `expect_hash` mismatch never mutates
- frontmatter round-trips with unknown keys intact
- no resolved path ever escapes a root

**T3 — Golden vault.** Fixture vaults in `vault-fixtures/`, committed. Run a full
index and assert on the resulting graph. Includes deliberately hostile cases:
unicode filenames, 10k-note vault, circular links, broken frontmatter, files
mutated mid-index.

**T4 — Agent eval harness.** Deterministic replay against recorded provider
responses (VCR-style cassettes), so agent behavior is testable without spending
money or tolerating nondeterminism. Scored on: patch validity, acceptance-rate
proxy on a labelled corpus, and cost per run. This tier is the difference
between "the agent feels better" and "the agent is better."

**T5 — E2E smoke.** Launch the app, open a fixture vault, accept a patch, undo
it, verify disk state. Small and slow; keep it to a handful of paths.

## CI

GitHub Actions, `macos-14` runners.

- **PR:** fmt, clippy, T1–T3, `cargo test -p gloss -p gloss-core -p gloss-cli`
- **Nightly:** T4 eval harness, full-vault performance benchmark with regression
  thresholds on cold-index and incremental-update time
- **Tag:** build, sign, notarize, release

Benchmark regressions fail the build. Index performance degrades gradually and
invisibly otherwise, and by the time you feel it you can't bisect it.

## Release

Universal binary (`aarch64` + `x86_64`), though Apple silicon is the only tested
target given MLX and Core Audio taps.

1. `libra package -p gloss`
2. `codesign` with Developer ID Application, hardened runtime enabled
3. `notarytool submit --wait`, then `stapler staple`
4. Sparkle for updates — appcast signed with EdDSA, hosted on GitHub Releases

**Entitlements needed:** microphone, audio input, and — for system-audio taps —
screen recording (Core Audio process taps are gated behind it). Request them
lazily, at the moment the feature is first used, with an in-app explanation
first. A cold TCC prompt on launch is how you lose a user in the first thirty
seconds.

**Apple Developer Program membership is a hard prerequisite** — $99/yr, and
enrollment can take days. Do it during M0, not the week you want to ship.

## Versioning

- App: semver, `0.x` until the review surface is stable
- **Index schema: its own integer version, with forward migrations.** The index
  will be rebuilt from files on mismatch, but a rebuild on a 20k-note vault is a
  two-minute stall — migrate where possible, rebuild only as fallback.
- **Prompt versions are pinned and recorded in patch provenance.** Changing a
  prompt changes behavior; you need to know which version produced a bad patch.
