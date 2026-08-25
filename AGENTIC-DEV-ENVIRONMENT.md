# The Agentic Development Environment

A reusable playbook for setting up a repository so that coding agents (and new
humans) can work on it correctly without being told the same things twice.

This document is **generic**. It describes the system in terms of roles and
contracts, not the specifics of any one project. Every filename here is a
convention you can rename; what matters is that each *job* has exactly one home.

---

## 1. The problem this solves

Agents fail on real codebases for three reasons, and none of them are about model
quality:

1. **No normative source.** The agent finds three documents that disagree and
   picks the wrong one — or worse, picks the code, which was the thing being
   questioned.
2. **No addressable knowledge.** "Read the docs" means reading 4,000 lines to
   answer one question. Context gets spent on retrieval instead of work.
3. **Documentation rot.** A doc that carries status ("we currently support X")
   goes stale in a week, and after that the agent is being actively misled.

The system below fixes all three with a structural answer: **one normative
knowledge base, split into addressable notes, with status confined to exactly one
place, and the important rules backed by machine-checkable guards.**

The payoff compounds. A well-built environment means an agent can be handed
"implement feature Y" with no other context and reliably: find the governing
design note, respect the invariants, follow the file layout, run the right tests,
and update the right status line when it lands.

---

## 2. The seven artifacts

Each artifact has one job. The discipline is in the *separation* — the moment two
files both explain the same thing, you have a drift source.

| Artifact | Job | Staleness risk |
|---|---|---|
| **Knowledge vault** (`architecture/` or `docs/design/`) | Normative design: how the system is *supposed* to work, one note per concept | Low — describes intent, changes deliberately |
| **Agent brief** (`<vault>/AGENTS.md`) | End-to-end on-ramp: the whole system in one read, links out for detail | Medium — holds the status summary on purpose |
| **Operational file** (root `AGENTS.md` / `CLAUDE.md`) | Commands, invariants, gotchas, layout rules — durable operational knowledge, **no status narrative** | Very low, by construction |
| **Generated indexes** (`SECTION-INDEX.md`, `CODE-INDEX.md`) | Resolve `§7.2` → file, and file → the notes that govern it. **Derived from note frontmatter, never hand-edited** | None — regenerated and CI-diffed |
| **Map of content** (`Map of Content.md`, `Home.md`) | Graph entry point and reading order; the outline itself is generated | Low |
| **Plans** (`plans/` + `Plans Index.md`) | Execution layer: staged work, decisions, acceptance criteria, status | High — but that's their purpose |
| **Guards** (CI checks, contract tests) | Make the non-negotiable rules machine-verifiable | None — they fail loudly |

### 2.1 The knowledge vault (normative)

A folder of markdown notes, numbered, one per section *and* subsection. Open it
as an Obsidian vault if you like graph views and backlinks; it stays plain
markdown either way, which is what agents actually consume.

Three properties make it work:

- **Addressable.** Every concept has a stable id (`§12.6`) that can be cited from
  code comments, commit messages, plans, and other notes. An agent reading
  `// see §12.6` in a source file knows exactly which note to open.
- **Granular.** One note per concept, sized so that reading it is cheap. If a
  note grows past what someone will actually read in one sitting, it wants to
  become a parent with children.
- **Normative.** The vault wins over the code. If code disagrees with the vault,
  the code is wrong — *or* the change deliberately updates the vault in the same
  commit. There is no third option, and saying so explicitly is what stops the
  "which do I trust" paralysis.

### 2.2 The agent brief

One note that explains the entire system in a single read: what it is, the core
bets, the vocabulary, the pipeline or main flow, the closed sets nobody may
casually extend, the module map, the testing ladder, what's built vs remaining,
and how to change things safely.

It is deliberately **the map, not the territory** — heavy on links, light on
detail. Target 150–250 lines. If it grows past that, detail has leaked in and
belongs in a section note.

This is also the one place allowed to summarize status, because there is exactly
one of it and updating it is a step on the landing checklist.

### 2.3 The operational file

The root-level file your agent tooling loads automatically (`AGENTS.md`,
`CLAUDE.md`, or both). It carries what you'd tell a competent new contributor on
day one *and would still be true a year later*:

- Commands: build, test, run, one-test-kind, regenerate, lint, package.
- Module/crate/package map with a one-line job description each.
- Invariants that any change must preserve.
- Hard-won gotchas — the bugs that cost someone a day. This is the highest-value
  content in the whole system and it exists nowhere else.
- Layout rules: where new files go, how tests are split, dependency policy.

And what it must **not** carry: status, roadmap, progress narrative, "currently
we support…". Those live in the brief and the plans. Write this rule into the
file itself, as a note to future editors, or it will grow back.

### 2.4 Plans (the execution layer)

Design notes say what the system *is*. Plans say what someone is *doing about
it*, staged, with exit criteria. A plan note carries:

- **Status** line at the top (planned / in progress with stage / complete + date).
- **Read before writing code** — links to the governing design notes. This single
  line does more for agent correctness than any amount of prose, because it
  converts "go understand the system" into three specific files.
- Scope table: in v1 vs deferred to later. Explicit deferral prevents both
  gold-plating and re-litigation.
- Numbered stages, each independently landable and testable.
- **Acceptance checklist** with checkboxes — the machine-checkable definition of
  done, ticked as stages land.
- Out of scope, explicitly.
- **In code** — a stage → file-path table, so a resumed plan doesn't require
  rediscovery.
- **When this lands** — which notes and status lines to update.

The Plans Index is a table of plan → status → governing sections. It is the
answer to "what is being worked on and what does it touch."

### 2.5 Guards

Prose rules get violated; checked rules don't. For each invariant, ask: what's
the cheapest thing that fails when this breaks? Common shapes:

- **Deny-list CI job.** A grep for names, imports, or patterns that must not
  reappear, with explicit allowances for the notes that document the history.
  Cheap, and it catches regressions that reviewers wave through.
- **Contract test.** Two implementations that must agree, a property that must
  hold, a byte-exact output that must not change — one test whose failure message
  names the rule and the note that explains it.
- **Fixture requirement.** "Changing X requires a fixture in `conformance/`" is
  enforceable by making the fixture suite the thing that fails.
- **Link/index check.** A script asserting every citation resolves and every note
  appears in the index. Prevents silent decay of the addressing scheme.

Guard failure messages should cite the note. `See §6.4 — grammar changes need
both engines` teaches; `assertion failed` doesn't.

---

## 3. Layout

```
repo/
├── AGENTS.md                     # operational file (agent tooling auto-loads)
├── CLAUDE.md                     # same content, or a one-line pointer
├── README.md                     # human/user-facing: what this is, how to run
├── docs/design/                  # ← the vault (name it whatever fits)
│   ├── README.md                 # how to use this folder
│   ├── AGENTS.md                 # THE BRIEF — start here
│   ├── Home.md                   # entry + reading order
│   ├── Map of Content.md         # graph-friendly outline (outline generated)
│   ├── SECTION-INDEX.md          # generated: §N.N → file
│   ├── CODE-INDEX.md             # generated: file → governing notes
│   ├── DRIFT.md                  # inbox for noticed contradictions
│   ├── Plans Index.md            # plan → status → sections
│   ├── sections/
│   │   ├── 01-vision.md
│   │   ├── 01.1-problem.md
│   │   ├── 01.2-the-bet.md
│   │   ├── 02-system-overview.md
│   │   └── …
│   ├── appendices/
│   │   ├── appendix-a-invariants-checklist.md
│   │   └── appendix-b-glossary.md
│   └── plans/
│       ├── PLAN-<FEATURE>.md
│       ├── MIGRATION-<NAME>.md
│       └── archive/<year>/       # completed plans
├── scripts/docs-check.py         # generates the indexes, validates the vault
├── .github/workflows/            # guards, including docs-check
└── src/ …                        # code, with §N.N citations in doc comments
```

Notes on the choices:

- **Nested `AGENTS.md`.** Several agent tools auto-load `AGENTS.md` files found
  anywhere in the tree, so putting the brief at `docs/design/AGENTS.md` gets it
  loaded for free while keeping it inside the vault where its links resolve.
- **Root duplication.** If your tooling reads `CLAUDE.md` and your teammate's
  reads `AGENTS.md`, keep one canonical file and make the other a pointer (or a
  symlink). Never maintain two copies of the same content.
- **Numbering with zero padding** (`01.1-`, not `1.1-`) so filesystem order
  matches logical order.

---

## 4. Setup, in order

The order matters: each step makes the next cheaper. Expect a day for a
greenfield project, two or three for a retrofit of an existing codebase.

### Step 1 — Write the brief first, badly

Before any folder structure, write the end-to-end brief in one sitting: what the
system is, the three-to-five bets everything else follows from, the vocabulary,
the main flow, the module map. Don't research; write what you already believe.

This is the highest-leverage hour in the whole process. It forces you to name
your bets, and the gaps you can't fill become your first section notes.

### Step 2 — Extract the vocabulary

List the 5–10 words your system uses for its core concepts, each with a one-line
definition. Put it in the brief and in the glossary appendix.

Do this early and enforce it. Every synonym pair in a codebase ("job"/"task",
"user"/"account") costs every reader and every agent a translation step forever.
If you already have synonym drift, that's your first `MIGRATION-` plan, and its
final stage is a deny-list guard so the old names can't return.

### Step 3 — Outline the sections, then create only the notes you can fill

Number the outline top to bottom: vision, prior art, mental model, overview, then
one section per major subsystem, then cross-cutting concerns (testing, DX,
packaging, performance), then roadmap, non-goals, open questions.

Create notes for the sections you can write substantively today. An empty
numbered note is worse than a missing one — an agent reads it, learns nothing,
and trusts the vault less. Reserve the number in the outline instead.

Two sections earn their place immediately even in a tiny project:

- **Non-goals.** The scope fence. Without it, agents helpfully build things you
  decided not to build.
- **Open questions.** Where undecided design lives, so an agent can tell "not yet
  decided" from "not yet documented" — a distinction that otherwise gets
  resolved by invention.

### Step 4 — Adopt the note template

Every section note gets the same shape. Consistency is what lets an agent skim
20 notes in the same pass.

```markdown
---
title: "12.6 Background Job Retries"
section: "12.6"
aliases: ["§12.6", "12.6", "Job retries"]
tags: [design, section/12]
type: design
---

# 12.6 Background Job Retries

## In this vault

- Parent: [[12-background-jobs]]
- Related: [[12.1-job-vocabulary|§12.1]] · [[15.4-integration-tests|§15.4]]
- Plan: [[PLAN-RETRIES]]

## In code

- `src/jobs/retry.rs` — backoff policy, attempt accounting
- `src/jobs/worker.rs` — where the policy is applied
- `tests/retries.rs` — the contract test

---

**Status: shipped (2026-03-14).**

<the normative content: the shape, the rules, why each restriction pays,
what is deliberately excluded>

---

← [[Home]] · [[Map of Content]] · parent [[12-background-jobs]]
```

Three parts do the real work:

- **Aliases** including `§N.N` — quick-open by citation, and search hits from a
  code comment land on the right note.
- **In this vault** — bidirectional links (parent up, children down, related
  across) so the graph is navigable rather than a flat pile.
- **In code** — the bridge between design and implementation. This is the section
  most often skipped and most often missed; it turns "where would this live?"
  into a lookup.

### Step 5 — Build the index and the map

`SECTION-INDEX.md` is a two-column table: `§N.N` → file path. Nothing else. This
is the resolver an agent uses when it hits a citation in code, and it's the one
file that must never be out of date — which is why a script should check it.

`Map of Content.md` / `Home.md` carry the full nested outline with links, plus a
suggested reading order for a first hour. The reading order is worth writing
explicitly; left to itself, a fresh agent reads in file order, which is rarely
the order that builds understanding fastest.

### Step 6 — Write the operational file

Now write the root file: commands, module map, invariants, gotchas, layout rules.
Point at the brief in the first paragraph, and state that the vault is normative
and status lives there. Include the meta-rule that this file carries no status
narrative, so it can't go stale.

Every gotcha you've personally been bitten by belongs here, with the *reason*.
"Don't simplify this comparison to a derive — pointer equality breaks across
module boundaries" saves the next agent an afternoon and prevents a plausible
"cleanup" PR.

### Step 7 — Start the plans layer

Write one plan for the work in flight, using the template below, and create the
Plans Index with its single row. It'll feel like overhead for one plan; it stops
feeling that way around the fourth, when "what did we decide about X and how far
did it get" becomes a question with a file for an answer.

```markdown
---
title: "PLAN-RETRIES"
aliases: ["PLAN-RETRIES"]
tags: [plans, design]
type: plan
---

# Retry policy for background jobs (§12.6)

**Status: in progress (R1–R2 shipped).** Implements [[12.6-background-job-retries|§12.6]].

**Read before writing code:** [[12.6-background-job-retries|§12.6]] (normative),
[[12.3-execution-model|§12.3]] (ordering guarantees),
[[appendix-a-invariants-checklist|Appendix A]].

## Scope

| In (v1) | Out (later) |
|---|---|
| Exponential backoff, fixed ceiling | Per-tenant policy overrides |
| Attempt count on the job record | Dead-letter UI |

## Stages

1. **R1** — policy type + unit tests
2. **R2** — worker applies policy; integration test with a failing job
3. **R3** — observability: attempt counters
4. **R4** — docs + §12.6 status line

## Acceptance

- [x] Backoff sequence is deterministic and unit-tested
- [x] A job failing 3× lands in the expected terminal state
- [ ] Attempt counters visible in the metrics endpoint
- [ ] Full test suite green at every stage boundary

## Out of scope

- Per-tenant overrides — see [[26-open-questions|§26]]

## In code

| Stage | Location |
|---|---|
| R1 | `src/jobs/retry.rs` (new), `tests/unit/retry.rs` |
| R2 | `src/jobs/worker.rs`, `tests/integration/retries.rs` |

## When this lands

1. [[12.6-background-job-retries|§12.6]] — status line → shipped.
2. [[AGENTS]] §8 — move retries from "remaining" to "done".
3. [[Plans Index]] — status column.

---

← [[Plans Index]] · governing [[12.6-background-job-retries|§12.6]]
```

### Step 8 — Add the invariants appendix and the guards

Write the checklist appendix: the short list of properties that any change must
preserve, phrased as questions a reviewer or agent can answer. Then, for each
line, add the cheapest possible automated check. Some will have none — that's
fine, but know which ones are prose-only, because those are where drift starts.

### Step 9 — Cite sections from code

Add `§N.N` citations to doc comments as you touch code. This is the loop
closing: code points at design, design points back at code (`In code`), and an
agent entering from either side reaches the other in one hop.

Retrofit opportunistically — every file you edit gains a citation — rather than
in a sweeping pass.

---

## 5. The operating rules

These are the rules that keep the system from decaying. They're worth stating
verbatim in your brief, because they're what an agent needs to *follow*, not just
know.

1. **One source of truth per fact.** Before writing a sentence, ask where this
   fact already lives. If it lives somewhere, link instead of restating.
2. **The vault is normative.** Code that disagrees is wrong, unless the same
   change deliberately updates the vault.
3. **Edit the note, never the mega-doc.** Resist the urge to regenerate one big
   document "for convenience" — it instantly becomes a second source of truth,
   and the one people paste into chats.
4. **Status lives in exactly one place** (the brief's built-vs-remaining section,
   plus per-plan status lines and per-note status lines). Never in the
   operational file, never in the README.
5. **Cite, don't summarize.** A link costs one line and never goes stale; a
   summary costs a paragraph and goes stale silently.
6. **Update only what moved.** Landing a change means touching the governing
   note, the status lines that actually changed, and the index if you added a
   note. Not a full vault audit. Stale *adjacent* prose is acceptable; stale
   *governing* prose is not. Saying this out loud matters — without it agents
   either rewrite half the vault or nothing at all.
7. **Don't build ahead of the roadmap.** New capability that isn't in a plan or a
   section is a proposal, not a task. Write it in open questions.
8. **Generate anything derivable.** Indexes, outlines, and status columns are
   projections of note frontmatter, not documents. Nothing that a script can
   compute may be maintained by hand — that's the difference between a system
   that stays true and one that decays at the speed of everyone's discipline.
9. **IDs are permanent.** A section number is a public identifier the moment it
   appears in a code comment. Add numbers, insert sub-numbers, retire numbers —
   never renumber, never reuse. Renumbering silently invalidates every citation
   in the codebase, and nothing catches it.
10. **Never silently absorb a contradiction.** If code and docs disagree and
    you're not fixing it now, append a dated line to the drift inbox. An
    unrecorded discovery dies with the session that made it.
11. **Bounded, not append-only.** Status sections, briefs, and indexes have size
    caps. Anything that only grows is a changelog wearing a badge, and nobody
    reads it — including the agent it was written for.

### The landing checklist

Put this in the brief, short enough that following it is automatic:

1. Does the governing note match the new behavior? (Or did you consciously decide
   the code follows an existing note?)
2. Did built-vs-remaining status shift? Update the brief and/or the roadmap note.
   Replace a row; don't append a sentence.
3. New note? Frontmatter complete, parent links wired — then regenerate indexes.
4. Run the relevant guards — scoped tests, the contract tests your change could
   plausibly break, and `docs-check`.

---

## 6. Failure modes

| Symptom | Cause | Fix |
|---|---|---|
| Agent contradicts a documented rule | Rule is buried in a note it never opened | Promote the rule to the brief's "closed sets" list, or make it a guard |
| Docs describe a system that no longer exists | Landing checklist isn't part of the change | Put the checklist in the plan's "When this lands" — same commit as the code |
| Agent asks questions the docs answer | No reading order, no on-ramp | Strengthen the brief; make it the first thing the operational file points at |
| Vault is large but agents still guess | Notes describe *what* without *where* | Add "In code" blocks; cite `§N.N` from source |
| Two notes disagree | The same fact written twice | Delete one, link to the other |
| Status is wrong everywhere | Status was written wherever it was convenient | Confine it: brief + plan statuses + note status lines only |
| Nobody writes notes | Note template is heavy, or the payoff is invisible | Trim the template; require a note only for decisions that constrain future work |
| Brief is now 800 lines | Detail leaked in | Move the detail into section notes and link |
| Retired vocabulary keeps returning | Prose-only rename | Deny-list guard in CI, with the history notes explicitly allowed |
| Status section is an unreadable append-only paragraph of shipped items and dates | Status was allowed to accumulate instead of being replaced | Fixed-shape table, rows replaced in place; shipped narrative belongs to plan status lines and git |
| The outline exists in three files and they disagree | Hand-maintained duplication of a derivable fact | Generate the outline and the index from frontmatter; CI-diff the result |
| `In code` paths point at files that moved | Nothing checks them | `docs-check` asserts every referenced path exists |
| 30 plans, most complete, and "what's in flight" takes five minutes to work out | No archive policy | Move completed plans to `plans/archive/`; index keeps the link |
| Agent read the wrong package's conventions | No routing from code path to governing docs | Router table in the root file + generated reverse index |
| Citations in code point at notes that no longer exist | Sections were renumbered | IDs are permanent; `docs-check` verifies every `§N.N` in code resolves |

---

## 7. Making it self-maintaining

Discipline is not a maintenance strategy. Everything in this chapter converts a
rule that people are supposed to follow into a fact a machine can check or
compute. The test of the design is simple: **if the whole team stopped caring for
a month, what would still be true?**

Four mechanisms carry it: derive what's derivable, bound what grows, detect drift
from git, and give discoveries somewhere to land.

### 7.1 Derive the indexes from the notes

Note frontmatter is the source of truth for metadata; every index is a
projection of it:

| Generated file | Projection of |
|---|---|
| `SECTION-INDEX.md` | every note's `section` + path |
| Outline in `Map of Content.md` | `section` ordering + `title` |
| Status column in `Plans Index.md` | each plan's `status` frontmatter |
| `CODE-INDEX.md` | reverse of every note's `In code` block: path → governing notes |

Generated files carry a `<!-- generated by scripts/docs-check.py — do not edit -->`
header, and CI fails if regenerating produces a diff. This is what kills the
three-way outline drift that hand-maintained vaults always develop.

`CODE-INDEX.md` deserves special attention: it's the artifact that makes the
whole vault navigable *from the code inward*, which is how agents actually
arrive. Given a diff, you can answer "which design notes govern these files?"
without the agent having read anything first. It costs nothing to maintain,
because it's derived.

To keep it parseable while staying human-first, fix the `In code` line format:

```markdown
## In code

- `src/jobs/retry.rs` — backoff policy, attempt accounting
```

### 7.2 Bound the status

Status is the one thing in the system that's *supposed* to change constantly, so
give it a shape that can't sprawl:

```markdown
## Status

| Capability | State | Governing | Plan |
|---|---|---|---|
| Job retries | shipped | [[12.6-…\|§12.6]] | [[PLAN-RETRIES]] |
| Dead-letter queue | in progress (R3) | [[12.7-…\|§12.7]] | [[PLAN-RETRIES]] |
| Per-tenant policy | not started — do not build ahead | [[26-open-questions\|§26]] | — |
```

Rows get **replaced**, never appended. A capability that shipped is one row that
says `shipped`, not a sentence added to a paragraph of shipped things. Historical
narrative — what landed when, what it replaced — belongs to the plan's status
line and to git, which are both better at it and neither of which needs pruning.

The `not started — do not build ahead` rows matter more than they look: that's how
an agent distinguishes "missing" from "deliberately absent," which is the
difference between asking and inventing.

### 7.3 Detect drift with git

The strongest self-maintenance signal is free, and almost nobody uses it: **git
already knows when code moved and its governing note didn't.**

For each note, compare the timestamp of the last commit touching the note against
the last commits touching the paths in its `In code` block. A note whose code
moved months after it did is a drift candidate. Ranked by gap, this is a to-do
list for documentation maintenance that writes itself.

Two ways to surface it, and you want both:

- **Advisory PR comment.** For the files in the diff, look up `CODE-INDEX.md` and
  post: *"these notes govern files you touched; confirm they still hold."* Never
  blocking — a blocking docs check gets disabled the first time it's wrong at 6pm.
- **Periodic report.** A ranked staleness list, reviewed on whatever cadence you
  actually keep. The top five entries are almost always genuinely stale.

### 7.4 Gate the mechanical things

These are cheap, unambiguous, and safe to fail a build on:

1. Frontmatter schema valid (title, section, aliases including `§N.N`, tags, type).
2. Every wiki-link resolves to a note or alias.
3. Every path in an `In code` block exists.
4. Every `§N.N` cited from source code resolves to a note — the one-liner is
   `rg -o '§[0-9]+(\.[0-9]+)*' src/ | sort -u`, checked against the index.
5. Regenerating the derived files produces no diff.
6. The brief is under its line cap.

Reference implementation, stdlib-only, trimmed to the essentials:

```python
#!/usr/bin/env python3
"""docs-check — validate the design vault and regenerate its derived files."""
from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

VAULT = Path("docs/design")
NOTES = sorted(VAULT.rglob("*.md"))
FRONTMATTER = re.compile(r"\A---\n(.*?)\n---\n", re.S)
WIKILINK = re.compile(r"\[\[([^\]|#]+)")
CODE_REF = re.compile(r"^- `([^`]+)`")


def frontmatter(path: Path) -> dict[str, object]:
    match = FRONTMATTER.match(path.read_text())
    if not match:
        return {}
    fields: dict[str, object] = {}
    key = None
    for line in match.group(1).splitlines():
        if line.lstrip().startswith("- ") and key:
            fields.setdefault(key, []).append(line.split("- ", 1)[1].strip(' "'))
        elif ":" in line:
            key, _, value = line.partition(":")
            key = key.strip()
            if value.strip():
                fields[key] = value.strip(' "')
    return fields


def code_paths(path: Path) -> list[str]:
    inside, found = False, []
    for line in path.read_text().splitlines():
        if line.startswith("## "):
            inside = line.strip() == "## In code"
        elif inside and (match := CODE_REF.match(line)):
            found.append(match.group(1))
    return found


def last_touched(path: str) -> int:
    """Unix timestamp of the last commit touching path; 0 if never."""
    out = subprocess.run(
        ["git", "log", "-1", "--format=%ct", "--", path],
        capture_output=True, text=True,
    ).stdout.strip()
    return int(out) if out else 0


meta = {note: frontmatter(note) for note in NOTES}
resolvable = {note.stem for note in NOTES}
for note, fields in meta.items():
    resolvable.update(fields.get("aliases", []))

errors: list[str] = []
drift: list[tuple[int, str]] = []

for note, fields in meta.items():
    for target in WIKILINK.findall(note.read_text()):
        if target.strip() not in resolvable:
            errors.append(f"{note}: unresolved link [[{target.strip()}]]")

    referenced = code_paths(note)
    for ref in referenced:
        if not Path(ref).exists():
            errors.append(f"{note}: 'In code' path does not exist: {ref}")

    if referenced:
        note_touched = last_touched(str(note))
        code_touched = max(last_touched(ref) for ref in referenced)
        # A note or file with no history yet (new, untracked) is not drift.
        if note_touched and code_touched:
            gap_days = (code_touched - note_touched) // 86400
            if gap_days > 60:
                drift.append((gap_days, str(note)))

index = ["<!-- generated by scripts/docs-check.py — do not edit -->", "", "| § | Note |", "|---|---|"]
index += [
    f"| {fields['section']} | `{note.relative_to(VAULT)}` |"
    for note, fields in sorted(meta.items(), key=lambda kv: kv[1].get("section", ""))
    if fields.get("section")
]
generated = "\n".join(index) + "\n"
index_path = VAULT / "SECTION-INDEX.md"

if "--fix" in sys.argv:
    index_path.write_text(generated)
elif not index_path.exists() or index_path.read_text() != generated:
    errors.append("SECTION-INDEX.md is stale — run scripts/docs-check.py --fix")

for days, note in sorted(drift, reverse=True):
    print(f"drift: {note} — code moved {days}d after the note last changed")

for error in errors:
    print(f"error: {error}", file=sys.stderr)
sys.exit(1 if errors else 0)
```

### 7.5 The drift inbox

One file, `DRIFT.md`, append-only, triaged and emptied on a cadence. The rule in
the brief: *if you find code and docs disagreeing and you're not fixing it in
this change, add a dated line here.*

```markdown
- 2026-03-14 — §12.6 says retries cap at 5; `retry.rs` caps at 3. Which is right?
- 2026-03-16 — no note covers the migration runner (`src/db/migrate.rs`).
```

This is the cheapest high-value mechanism in the whole system. Agents notice
inconsistencies constantly and currently have nowhere to put them, so the
observation is discarded at the end of every session. An inbox converts thousands
of dead-end discoveries into a maintenance queue, and unlike a docs sprint, it's
sorted by what people actually touch.

### 7.6 Prune on a policy, not on a mood

- **Completed plans** move to `plans/archive/<year>/`; the index keeps one row
  with the link. The in-flight list stays scannable, which is the only reason the
  index exists.
- **Notes for deleted features** get deleted. Git holds the history. Leave a
  tombstone only if the id is widely cited, and say what replaced it.
- **Retired ids** are never reused.

### 7.7 Put the checklist where the work happens

The landing checklist works when it's unavoidable: a PR template with the four
questions, a `docs-check` job on every PR, and — if your agent tooling supports
hooks — a pre-finish hook that runs `docs-check` so the agent fixes its own
wiring before it reports done.

---

## 8. Scaling to a large monorepo

Everything above assumes one coherent system. A monorepo breaks two of those
assumptions: there is no single §-number space that many teams can share, and no
agent can read all the docs. The fix is federation plus routing.

### 8.1 Federate: facts live at the narrowest scope that owns them

```
repo/
├── AGENTS.md                     # router + repo-wide invariants + commands
├── docs/design/                  # cross-cutting: vocabulary, platform contracts,
│   └── …                         #   boundaries, conventions, repo-wide invariants
└── packages/
    ├── payments/
    │   ├── AGENTS.md             # package brief (auto-loaded when work is here)
    │   └── docs/                 # package-local design notes + plans
    └── search/
        ├── AGENTS.md
        └── docs/
```

The root vault holds only what is true across packages: the vocabulary, the
invariants everyone inherits, the conventions, and the **boundary contracts**
between packages. Everything else is local. A fact written at the root that only
one package cares about is a fact three other teams will read, mistrust, and
eventually contradict.

### 8.2 Namespace the ids, and make them permanent

Global numbering collapses in a monorepo: two teams both want §12, and inserting
a section means renumbering someone's citations. Prefix by scope:

```
CORE§4.2     platform / cross-cutting
PAY§3.1      packages/payments
SEARCH§7     packages/search
```

Then hold the line on permanence (rule 9): append new numbers, insert
sub-numbers (`3.2.1`) rather than resequencing, retire dead ids, never reuse.
Citations in code are a public API — treat renumbering the way you'd treat
renaming a public function with no deprecation path.

### 8.3 Route from path to docs

This is the single most important monorepo addition. The root operational file
opens with a table:

| Touching | Read first | Commands |
|---|---|---|
| `packages/payments/**` | `packages/payments/AGENTS.md` · `PAY§3` | `nx test payments` |
| `packages/search/**` | `packages/search/AGENTS.md` · `SEARCH§1` | `nx test search` |
| `packages/*/api/**` | `docs/design/sections/09-service-boundaries.md` | `make contract-tests` |
| Anything | repo invariants below | `make check` |

Three layers of routing, coarse to fine, and each is cheap:

1. **The router table** — hand-written, one row per package, rarely changes.
2. **Nested `AGENTS.md`** — most agent tools load the one nearest the work
   automatically, so being in the right directory *is* the routing.
3. **`CODE-INDEX.md`** — generated, file-level: exactly which notes govern the
   files in this diff.

### 8.4 Give boundaries their own notes

In a monorepo, the expensive failures are between packages, not inside them — and
a note owned by one team never describes the contract honestly. Make each
cross-package interface a note with a fixed shape: owner, consumers,
compatibility rules, versioning policy, and the contract test that enforces it.
Then require that changing the interface touches that note. The reverse index
makes it findable from either side of the boundary, which is the property that
makes it get read.

### 8.5 Ownership, or it rots

- `CODEOWNERS` entries covering `docs/` paths, so notes get reviewed by the people
  who'd notice they're wrong.
- Each package brief names an owner.
- The root vault is owned by whoever owns platform decisions. Unowned
  cross-cutting docs are the first thing to go stale and the last thing anyone
  notices.

### 8.6 Budget the context deliberately

At scale the binding constraint is what fits in one session, so treat these as
architectural limits rather than style preferences:

| Artifact | Cap | Why |
|---|---|---|
| Package brief | 250 lines | Must be readable in full before any task in that package |
| Section note | 200 lines | Read cost per lookup |
| "Read before writing code" list | 3 notes | More than that and nobody reads any of them |
| Router table | one row per package | Scannable at 100 packages |

If starting a task requires more than a brief plus three notes, the docs are
structured wrong — not too small. That's the signal to split a package's docs, or
to move a fact down to where it's used.

### 8.7 Scope the guards to the build graph

Docs checks are fast and stay global. Contract and package tests run only for
affected packages, using whatever affected-detection your build tool provides.
Guard cost is what determines whether guards survive contact with a busy repo.

### 8.8 Roll it out by churn, not by org chart

Don't attempt full coverage. Build the root vault and the router first — that
alone routes every agent correctly. Then rank packages by commit frequency
(`git log --format= --name-only | cut -d/ -f1-2 | sort | uniq -c | sort -rn`) and
document from the top down, one package per PR, using a shared brief template. The
highest-churn packages are simultaneously where agents work most and where docs
decay fastest, so they repay the investment first. Low-churn packages can stay
undocumented indefinitely, and that's the correct allocation.

---

## 9. Tooling notes

### Obsidian

Obsidian is optional but earns its place: backlinks show you what depends on a
note before you change it, the graph shows structural gaps (an orphan note is
usually a note nobody wired up), and quick-open on the `§N.N` alias is the
fastest possible path from a code citation to its design note.

Configure the minimum that's useful and commit it: file explorer, global search,
quick switcher, graph, backlinks, outgoing links, tag pane, properties, outline,
templates. Commit `app.json` and `core-plugins.json` so the setup is shared;
ignore personal and cache state:

```gitignore
<vault>/.obsidian/workspace.json
<vault>/.obsidian/workspace-mobile.json
<vault>/.obsidian/graph.json
<vault>/.obsidian/cache
<vault>/.obsidian/plugins
<vault>/.obsidian/themes
```

Keep it degradable. Wiki-links (`[[note|label]]`) are Obsidian-flavored but read
fine as plain text, and every agent reads the folder as plain markdown. Don't
adopt plugin-dependent syntax — the moment a note only renders correctly inside
one app, it stops being a shared source of truth.

### Agent instruction files

Different tools load different files. Pick one canonical operational file and
make the rest pointers. Nested `AGENTS.md` files are loaded by several tools when
work touches that subtree, which is a good way to give a subdirectory its own
local rules without bloating the root file.

If your tooling supports scoped rules (glob-triggered instructions), use them for
mechanical conventions — "test files in this folder follow this shape" — and keep
the root file for things that apply everywhere.

---

## 10. Retrofitting an existing repo

Greenfield is easy; most adoption is retrofit. The order that works:

1. **Write the brief from what exists.** One sitting, from your head, then
   fact-check against the code. What surprises you is what needed documenting.
2. **Inventory the docs you have.** Every markdown file, plus the tribal
   knowledge in issues and PR discussions. Sort into: normative (→ sections),
   operational (→ root file), historical (→ a history note or delete), duplicate
   (→ delete).
3. **Split the mega-doc.** If there's a 3,000-line ARCHITECTURE.md, cut it at its
   existing headings into numbered notes, keeping the heading numbers so existing
   citations survive. Add frontmatter and links per note. This is mechanical and
   an agent can do most of it — check the seams.
4. **Harvest gotchas.** Ask the team, or mine PR review comments, for "things you
   have to know or you'll break it." This is the single highest-value content you
   will add, and it usually exists nowhere written.
5. **Write plans for work in flight**, then stop worrying about the backlog.
6. **Add guards for whatever already broke twice.** Past incidents are a
   pre-filtered list of your real invariants.
7. **Then let it grow by use.** Each subsequent change adds or updates the note
   it touched. Don't schedule a documentation project; make documentation a step
   in the landing checklist.

---

## 11. Cadence

Size caps are in §8.6; two sizing rules don't fit a table:

- **Operational file:** as long as it needs to be, but every line must be durable.
  If a line could become false without anyone noticing, it belongs elsewhere.
- **Plan:** as long as the design discussion needs; the stages and acceptance
  criteria are the parts that must stay crisp.

Notes update per change, not per sprint — the landing checklist is the cadence.
Only two periodic tasks are worth scheduling, and both are short:

| Cadence | Task |
|---|---|
| Weekly | Triage the drift inbox to empty — each line becomes a fix, a plan, an open question, or a deletion |
| Quarterly | Skim the status table and plans index; archive completed plans. This is where optimism accumulates |

Everything else is either automated (§7) or event-driven (the checklist). If your
maintenance plan has more than two recurring calendar items, the automation isn't
carrying enough of the load.

---

## 12. Minimum viable version

If the full structure is too much for the project's size, this subset delivers
most of the benefit:

1. `AGENTS.md` at the root — commands, module map, invariants, gotchas, no status.
2. `docs/design/AGENTS.md` — the brief, with a bounded status **table**.
3. `docs/design/sections/` — notes for the subsystems you can describe today,
   numbered, with `In code` blocks.
4. `scripts/docs-check.py` — generates `SECTION-INDEX.md` and `CODE-INDEX.md`,
   validates links and paths. Even at ten notes this is cheaper than maintaining
   the index by hand once.
5. `DRIFT.md` — the inbox, and the rule that discoveries go in it.
6. One plan for the work in flight.
7. One CI guard for the rule that's been broken before.

Everything else — appendices, map of content, graph tooling, staleness reports,
per-package federation — is worth adding when navigation or context budget becomes
the bottleneck. Items 4 and 5 are what make the difference between a documentation
folder and a system that maintains itself, which is why they're in the minimum.

---

## Appendix: bootstrapping prompt

A starting prompt for an agent doing the initial build on an existing codebase.
Review its output rather than accepting it — the value is in the structure being
right and the facts being yours.

```
Set up an agentic development environment for this repo, following the
structure below. Explore the codebase first; do not invent facts.

1. Create docs/design/ with sections/, appendices/, plans/.
2. Draft docs/design/AGENTS.md — an end-to-end brief (150-250 lines):
   what this system is; the core bets/principles; the vocabulary
   (5-10 words with one-line definitions); the main data/control flow;
   the closed sets nobody may casually extend; a module map with a
   one-line job per module; the testing approach; built vs remaining;
   how to change the system safely; a suggested reading order.
   Link out for detail rather than explaining it inline.
   Status must be a fixed-shape table (capability | state | governing
   note | plan), never a narrative paragraph.
3. For each major subsystem, create sections/NN-<slug>.md with
   frontmatter (title, section, aliases including "§N", tags, type),
   an "In this vault" link block, an "In code" file list in the format
   "- `path` — role", and the normative description. Only create notes
   you can fill substantively; reserve numbers in the outline for the rest.
4. Add scripts/docs-check.py: generate SECTION-INDEX.md (§N.N -> path)
   and CODE-INDEX.md (path -> governing notes, reversed from the
   "In code" blocks); validate that wiki-links resolve, that every
   "In code" path exists, and that every §N.N cited in source resolves.
   Do not hand-maintain anything the script can generate.
5. Draft a root AGENTS.md: commands, module map, invariants, gotchas,
   file layout rules, and — if this is a monorepo — a router table
   mapping path globs to the docs and commands for that area.
   No status narrative, no roadmap.
6. Create DRIFT.md with a one-line explanation and any contradictions
   you found while exploring.
7. List, but do not write, the plans you think are needed for work
   currently in flight.

Report anything you could not determine from the code rather than
guessing, and put every doc/code contradiction you noticed in DRIFT.md.
```
