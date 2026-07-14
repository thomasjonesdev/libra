# Naming Migration Checklist

Executes the vocabulary rename adopted in LIBRA-ARCHITECTURE.md §3.1–§3.2 (2026-07 revision):
**Model → State · Msg → Action · update → reduce · Cmd → Effect · widget → component · tiers → named test kinds.**
Scheduled as a Phase-4 backlog item (§23); the doc leads, this checklist makes the code follow.

> **Interleaves with `PLAN-ERGONOMICS.md`** — its Combined Execution Order table is the master sequence: Stages 0–4 here, then ergonomics steps E1–E3, then Stages 5–8 here (Stage 6 as amended there).

## Ground rules (apply to every stage)

- [ ] Each stage is **one PR**, landed with `cargo test --workspace` green.
- [ ] Every stage is **snapshot-neutral**: `git status examples/*/tests/snapshots/` shows no modified PNGs after the full suite runs, and the SHA-256 hashes in README.md are untouched. A rename PR that re-blesses anything is wrong by definition.
- [ ] No stage mixes rename with behavior change. The one stage that adds behavior (Stage 4, the actions macro) adds it *around* the existing substrate, never inside it.
- [ ] Old names survive each stage as `#[deprecated]` aliases until Stage 8 removes them all at once.

---

## Stage 0 — Preflight

- [ ] `cargo test --workspace` green on the starting commit.
- [ ] Record baseline: `shasum -a 256 examples/*/tests/snapshots/*.png > /tmp/snapshot-baseline.txt` (compare after every stage).
- [ ] Confirm no in-flight branches touch the rename surface (crate names, `Cmd`, harness API) — land or park them first.

## Stage 1 — Crate rename: `libra-widgets` → `libra-components`

- [ ] `git mv crates/libra-widgets crates/libra-components`; update `[package] name`.
- [ ] Update `[workspace.dependencies]` in the root `Cargo.toml` and every dependent crate's `Cargo.toml`.
- [ ] Sweep `use libra_widgets::` → `use libra_components::` across crates/, examples/, cli/, conformance/.
- [ ] Grep gate: `grep -rn "libra.widgets\|libra_widgets" --include='*.rs' --include='*.toml' .` returns nothing outside this checklist and LIBRA-ARCHITECTURE.md §3.2.

## Stage 2 — Core type renames (with deprecated aliases)

Definition sites found in: `crates/libra-core/src/lib.rs`, `crates/libra-reflect/src/lib.rs`, `crates/libra-interp/src/lib.rs`, `crates/libra-platform/src/lib.rs` (a11y), `crates/libra-test/src/lib.rs`.

- [ ] `Cmd` → `Effect` (enum + constructors). Keep `#[deprecated] pub type Cmd = Effect;`. **Do not touch the hand-written `PartialEq`** — it ignores msg-ctor identity deliberately (fn-pointer equality breaks across crate boundaries).
- [ ] `ReflectModel` → `ReflectState`, `ReflectMsg` → `ReflectAction` (supertraits in libra-core); deprecated re-exports under the old names.
- [ ] `Widget` trait → `Component` (plus companions: `WidgetEvent` → `ComponentEvent`, any `DynMsg` → `DynAction`); `pub use Component as Widget` re-export, deprecated.
- [ ] `MsgCtor` → `ActionCtor` (or equivalent registry type) in the effect vocabulary.
- [ ] Rename internal identifiers/fields mechanically (`msg` → `action`, `model` → `state`) *within the renamed items only* — don't chase every local variable in this stage.
- [ ] Diagnostics text: interp/codegen error strings that say "Msg variant" / "no variant on Msg" now say "action" (e.g. `no action 'incremnt' — did you mean 'increment'?`). Update the tests that assert on these strings in the same PR.
- [ ] Snapshot check vs `/tmp/snapshot-baseline.txt`.

## Stage 3 — Harness API rename

- [ ] `expect_cmd` → `expect_effect`, `expect_no_cmds` → `expect_no_effects`, `cmd::` matcher module → `fx::` — all with deprecated aliases.
- [ ] `assert_cmd!` → `assert_effect!` (macros can't be deprecated-aliased cleanly; keep the old macro delegating to the new one with a doc note).
- [ ] Optional, decide here: add `Harness::render(...)` as the documented constructor (§15.3), keeping `Harness::new` working.
- [ ] Update in-repo callers (examples, conformance) to the new names so the deprecated paths have zero internal users.

## Stage 4 — The actions layer (`#[libra::state]` / `#[libra::actions]`) — **the only behavior-adding stage**

Implements LIBRA-ARCHITECTURE.md §5.1–§5.2 in `libra-reflect`. This is a feature PR (or small series), not a sweep.

- [ ] `#[libra::actions]` on an impl block generates: the `Action` enum (one variant per method, params = payload), `reduce(state, action) -> Vec<Effect>`, and reflection metadata (handler names for markup + automation schema).
- [ ] Dispatchability rule per §5.2: markup-expressible params ⇒ handler; effect-result params ⇒ continuation (never constructible by name). Reuse the existing unit-variant/payload-variant reflect split.
- [ ] Action return types accept `()`, `Effect`, `Option<Effect>`, `Vec<Effect>`.
- [ ] `#[libra::state]` subsumes `#[derive(LibraReflect)]` (which stays, deprecated, as a thin wrapper).
- [ ] Hand-written enum + reducer path stays fully supported and tested (it is the expansion target).
- [ ] **Markup handler naming (§3.3)**: `.lx` handlers and bindings use the **camelCase public name**, mapped mechanically from the snake_case Rust name (`fn filter_changed` ⇒ `onChange={filterChanged}`; serde `rename_all = "camelCase"` discipline). Legacy PascalCase variant names (`onPress={Reset}`, today's style) keep resolving with a deprecation warning in `libra check`. This touches the grammar's name-resolution rule, so per §6.4 it lands in **both** engines (libra-interp AND libra-codegen) **with conformance fixtures** (one for the camelCase mapping, one for the legacy fallback), and `interp_and_codegen_produce_identical_trees` stays green.
- [ ] Typo suggestions cross the mapping: `onPress={filter_changed}` → "did you mean `filterChanged`?"; whitelisted accessors/predicates expose camelCase in markup (`isEmpty`, `isLive`).
- [ ] Tier-1-style unit tests for the macro itself in libra-reflect (expansion output, dispatchability classification, error cases).
- [ ] Slices (`#[slice]`, §5.7) may be deferred to a follow-up PR — note it here if so.
- [ ] Snapshot check: identical bytes (same views, same rendering).

## Stage 5 — Test suite reorganization (tiers → kinds)

For each of `examples/counter`, `examples/fetcher`, `examples/dashboard`, `examples/notes` (all have `tests/tiers.rs`):

- [ ] Split `tests/tiers.rs` into `tests/components.rs` (old tier2), `tests/integration.rs` (old tier3), `tests/snapshots.rs` (old tier4); move old tier1 tests inline as `#[cfg(test)] mod tests` next to the state they exercise (or `tests/unit.rs` where inline doesn't fit).
- [ ] Drop `tier*_` prefixes from test fn names; names become plain descriptions.
- [ ] **Keep every `assert_snapshot("name")` string unchanged** — snapshot PNG identity must not depend on this reorg.
- [ ] `examples/gallery/tests/gallery.rs`: rename any tier-prefixed tests the same way.
- [ ] Update any docs/scripts that invoke tests by prefix (`cargo test -p fetcher tier3_` in CLAUDE.md — see Stage 7).

## Stage 6 — Port example apps to the actions style

Reference apps should model the documented style. One example per PR; counter first as the template.

- [ ] `examples/counter`: `Model` → `#[libra::state] struct CounterState` + `#[libra::actions]` impl; `.lx` handlers to camelCase public names (`onPress={increment}`, multi-word ones like `filterChanged` where they occur); tests updated.
- [ ] `examples/fetcher` (includes continuations for Http/Kv results and `Remote<T,E>` display-projection per §6.3 — keep that pattern).
- [ ] `examples/dashboard` (frame action, data-plane handles).
- [ ] `examples/notes` (Input/EditOp routing unchanged — the editing core stays pure).
- [ ] Conformance fixtures that reference example-style handler names updated in the same PRs (both engines, fixture-covered).
- [ ] Snapshot check after each: byte-identical.

## Stage 7 — Documentation & tooling sweep

- [ ] CLAUDE.md: new crate name, new test layout/commands (`cargo test -p fetcher tier3_` → the new file-based invocation), state/actions/effects vocabulary.
- [ ] README.md: prose only — **the committed snapshot SHA-256 hashes must not change**; if a hash changed, a prior stage broke the ground rules.
- [ ] Doc comments: re-point architecture § citations shifted by the 2026-07 revision (old §5.2 purity → new §5.3; old §5.6 lenses → new §5.7 slices; §15.x tier headings → kind headings).
- [ ] `libra-cli` / `libra check` user-facing text: "widget" → "component", "msg" → "action".
- [ ] `themes/`, `conformance/` READMEs or comments mentioning old names.
- [ ] Final wording pass: `grep -rniE '\bwidget|\bmsg\b|\bcmd\b|tier[0-9]' --include='*.rs' --include='*.md' --include='*.lx' .` — remaining hits should only be deprecated aliases (Stage 8 removes them) and the historical tables in LIBRA-ARCHITECTURE.md §2/§3.2/Appendix B.

## Stage 8 — Alias removal (gate: before first public publish)

- [ ] Remove every `#[deprecated]` alias from Stages 2–4 in one PR.
- [ ] Remove legacy PascalCase variant-name acceptance (`onPress={Reset}`) from both engines + its conformance fixture (replace with a fixture asserting the *error*, whose suggestion names the camelCase form).
- [ ] Add a CI grep deny-list so old names can't reenter: fail on `libra_widgets|ReflectModel|ReflectMsg|expect_cmd|assert_cmd|tier[0-9]_` outside LIBRA-ARCHITECTURE.md and this file.
- [ ] Final full verification (below), then mark the §21 migration note in LIBRA-ARCHITECTURE.md as completed.

---

## Verification (run at every stage boundary)

```sh
cargo test --workspace                                  # all kinds, headless
cargo run -p libra-cli -- check .                       # .lx validation still clean
shasum -a 256 examples/*/tests/snapshots/*.png | diff - /tmp/snapshot-baseline.txt   # empty diff or the stage is invalid
cargo run -p dashboard --release -- bench               # §22 budget unaffected
```
