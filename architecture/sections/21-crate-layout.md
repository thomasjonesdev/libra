---
title: "21. Workspace & Crate Layout"
section: "21"
aliases:
  - "§21"
  - "21"
  - "21. Workspace & Crate Layout"
tags:
  - architecture
  - section/21
type: architecture
---

# 21. Workspace & Crate Layout

```
libra/
├─ crates/
│  ├─ libra-core        State/Action/Effect/Sub traits, runtime loop, action queue, replay log
│  ├─ libra-markup      lexer, parser, AST, validation, diagnostics (shared by ALL consumers)
│  ├─ libra-interp      dev engine: AST × State(reflect) → ViewTree
│  ├─ libra-codegen     release engine: AST × types → Rust (build-dependency)
│  ├─ libra-reflect     #[libra::state] / #[libra::actions] macros (drive interp + automation schema)
│  ├─ libra-components     catalog, component tree, reconciler, semantics, focus/keyboard
│  ├─ libra-layout      taffy integration, measure cache, rounding policy
│  ├─ libra-draw        DrawCmd, resources, damage tracker, layer cache
│  ├─ libra-raster      tiny-skia executor, PNG artifact writer
│  ├─ libra-text        parley/swash integration, atlas, editing core, IME state machine
│  ├─ libra-effects     Effect/Sub executors, worker pool, cancellation
│  ├─ libra-kv          redb store, snapshots, migrations
│  ├─ libra-data        ring buffers, ingest, M4/LTTB, tapes (.ltape)
│  ├─ libra-test        Harness, queries, effect interception, snapshot assert, proptest strategies
│  ├─ libra-automation  socket server, protocol, skills emitter
│  ├─ libra-platform    winit/softbuffer/muda/rfd/tray glue
│  └─ libra-theme       tokens, TOML themes, resolution
├─ cli/libra            the CLI (dev/check/test/build/package/fmt/lsp)
├─ examples/           counter, dashboard (data-plane), inbox, kanban, log-explorer
├─ applications/       first-party apps built for real use ([[27-first-party-applications|§27]])
└─ conformance/        golden fixtures: interp≡codegen suite, snapshot corpus, hostile inputs
```

Dependency spine: `markup` and `reflect` at the root; `interp` and `codegen` both depend on `markup` only (the no-drift guarantee is structural); apps depend on `libra` (a facade crate) + `libra-test` (dev-dep). The app crate stays thin so [[18.2-iteration-loop|§18.2]]'s rebuild target holds.

> **Naming migration ([[03.2-terminology-migration|§3.2]]): complete.** The mechanical rename ran to completion per `MIGRATION-NAMING.md`'s stage-by-stage checklist, landing snapshot-neutral throughout (every committed PNG hash survived every stage). The shipped crates now match the names above: `libra-components` (not `libra-widgets`), `Component` (not `Widget`), State/Action/`reduce`/`Effect` (not Model/Msg/update/Cmd), and named test-kind files — `tests/unit.rs`, `tests/components.rs`, `tests/integration.rs`, `tests/snapshots.rs` — rather than `tier*_` prefixes in one `tests/tiers.rs`. Stage 8 removed the deprecated pre-rename aliases and the legacy PascalCase markup-handler acceptance (`onPress={Reset}` is now a load-time/compile-time error suggesting the camelCase form, enforced identically by both engines); a CI naming guard (`.github/workflows/naming-guard.yml`) blocks the old names from reentering. `MsgCtor`/`Widget`/`ReflectModel`/`ReflectMsg`/`tier*_` survive only as historical references in this document and in `MIGRATION-NAMING.md` itself.

## In code

- `Cargo.toml` — `[workspace.dependencies]` (single source of versions)
- `crates/` — every workspace crate (auto-membered by the `crates/*` glob)

---

---

← [[Architecture Home]] · [[Map of Content]]
