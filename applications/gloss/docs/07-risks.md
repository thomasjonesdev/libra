---
title: Risks
type: plan
status: draft
created: 2026-08-24
tags: [gloss, planning]
---

# Risk register

Ordered by how much they should change the plan, not by likelihood.

## R-01 · Vault corruption destroys trust permanently

An agent that silently rewrites a note and gets it wrong once is finished. There
is no recovery from "it ate my notes" — the audience is precisely the people who
chose plain files to avoid that.

**Mitigation:** propose-only; `expect_hash` on every op; git checkpoint per run;
trash instead of delete; property tests as a release gate ([[05-delivery#Testing strategy]]).
**Trigger to stop:** any data-loss bug found post-release halts feature work
until the class of bug is closed by a property test.

## R-02 · No verifier means quality is unfalsifiable

Cursor had a compiler. Gloss has taste. Without a measurement, "the agent got
better" is a feeling, and six months of work can go into a regression.

**Mitigation:** the T4 eval harness with a labelled corpus, built during M1 and
treated as production code. Accept/reject ratio tracked from M3 onward.
**Trigger:** if the retroactive link pass in M3 has more rejections than
acceptances, the ranker is wrong — fix M1 rather than proceeding to M4.

## R-03 · Notification fatigue kills the app in week two

Proactivity is the product and also the largest product risk. A stream of
suggestions is indistinguishable from spam.

**Mitigation:** an explicit interruption budget. Batch into a daily digest by
default; never interrupt mid-writing; unsolicited suggestions capped per day and
user-configurable to zero. Escalation earns its way up, starting silent.

## R-04 · The category is crowded

Vault-aware chat and agentic edits are table stakes — several Obsidian plugins do
them, one past a million downloads. Entering on those features is entering last.

**Mitigation:** lead with capture and the code-guide wedge, which nobody serves.
Bundle coherence is the differentiator, and it's only defensible standalone.
**Watch:** if Obsidian ships first-party agentic editing, the plugin-competitor
framing collapses and the wedge becomes the whole product. Plan accordingly.

## R-05 · Willingness to pay

PKM has monetized badly for a decade and this audience self-selected for free and
local. BYOK removes the infrastructure cost but not the pricing problem.

**Mitigation:** the wedge points at engineers with a tools budget, who pay for
documentation that doesn't rot. Deferred deliberately — see the reassessment
gate in [[04-roadmap#Reassessment gate]].

## R-06 · Audio capture is a swamp

Core Audio process taps are gated behind screen-recording permission, the APIs
are recent, and the failure modes are per-device and hard to reproduce.

**Mitigation:** start R2 during M0, isolate in a Swift sidecar so failure can't
take the app down, mic-only as the shippable fallback. It is off the critical
path by design.

## R-07 · Solo capacity against a demanding day job

The most likely failure is simply stalling around M3, with L0/L1 built and
nothing anyone can use.

**Mitigation:** M2 ships something used daily before the hard UI work starts, so
a stall still leaves a working tool. Every milestone exits on "I use it," not "I
finished it."

## R-08 · Scope creep into sync

Standalone inherits multi-device the moment a second machine appears, and sync is
a year people routinely underestimate.

**Mitigation:** explicit non-goal. Defer to iCloud Drive, Syncthing, git, or
Obsidian Sync. The index is derived and rebuildable, so it must never be synced —
make it impossible to place inside the vault.

## R-09 · Name collision

"Gloss" is a common English word and a common product name. Unlike a coined
term, search results and trademark space are contested by default.

**Mitigation:** clear it before the first public commit — GitHub, crates.io,
npm, USPTO class 9, and the domain. Renaming after launch costs more than
renaming now: repo, bundle ID, Keychain service name, and MCP server identifier
all carry it.

## R-10 · Sensitive material on third-party APIs

Dogfooding on a vault containing work notes means an API call could be an
export-control problem, not just a privacy preference.

**Mitigation:** sensitive-vault mode as a v1 feature with hard failure, not a
warning — see [[06-operations#Sensitive-vault mode]].

## R-11 · Libra gaps get treated as Gloss features

The first draft assumed a webview editor. We are on Libra. Soft wrap,
`File*`, Keychain, streaming completions, and Tab paint are framework
changes. They have both-engines + Appendix A cost. The failure mode is
spending a month on catalog chrome, or inventing a Process workaround
and calling M4 done, while M2 still is not in my daily loop.

**Mitigation:** [[11-coherence]] is what we are building. [[10-libra]] is
the split. M2 does not wait on Libra. M4 exit names soft wrap and
`File*`, not graph viz. Tab is paint, not a §24 amendment.
**Trigger:** a PR that adds a catalog node or Effect "for Gloss" while
`gloss-mcp` is unusable.
