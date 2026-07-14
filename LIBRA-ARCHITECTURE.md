# LIBRA — Architecture & Design Document

**A native UI framework in Rust with a first-class testable foundation**

*Name: "Libra" — the scales: balance, measurement, and verifiable truth.*

*Status: Design complete; implementation through Phase 3 core. This document is the source of truth. Revised 2026-07: the developer-facing vocabulary is standardized on **state / actions / effects / components** and named test kinds (§3.1–§3.2); the code rename is staged, not yet applied.*

---

## Table of Contents

1. [Vision & Philosophy](#1-vision--philosophy)
2. [Prior Art & Influences](#2-prior-art--influences)
3. [Mental Model: React → Libra](#3-mental-model-react--libra)
4. [System Overview](#4-system-overview)
5. [State & Actions](#5-state--actions)
6. [The View Layer & Markup](#6-the-view-layer--markup)
7. [Components](#7-components)
8. [Layout](#8-layout)
9. [Rendering](#9-rendering)
10. [Text](#10-text)
11. [Theming & Design Tokens](#11-theming--design-tokens)
12. [The Effects Runtime](#12-the-effects-runtime)
13. [Persistence](#13-persistence)
14. [The High-Rate Data Plane](#14-the-high-rate-data-plane)
15. [Testing Architecture](#15-testing-architecture)
16. [The Automation Server](#16-the-automation-server)
17. [Accessibility](#17-accessibility)
18. [Developer Experience & Tooling](#18-developer-experience--tooling)
19. [Platform Integration](#19-platform-integration)
20. [Packaging & Distribution](#20-packaging--distribution)
21. [Workspace & Crate Layout](#21-workspace--crate-layout)
22. [Performance Budgets](#22-performance-budgets)
23. [Roadmap](#23-roadmap)
24. [Non-Goals](#24-non-goals)
25. [Risks & Mitigations](#25-risks--mitigations)
26. [Open Questions](#26-open-questions)

---

## 1. Vision & Philosophy

### 1.1 The one-sentence pitch

Libra is a Rust framework for building native desktop applications with React's mental model, where **every layer of the application — logic, views, effects, and rendered pixels — is deterministic and machine-verifiable from day one**.

### 1.2 The problem

Every mainstream UI stack treats testing as an afterthought bolted onto an inherently untestable core:

- **React/Electron**: state hides inside a component tree; testing requires a simulated DOM, mocked timers, `act()` incantations, and flaky async assertions. Visual regression testing requires a headless browser farm and tolerates pixel "fuzz" because rendering is non-deterministic across machines.
- **Native toolkits (Qt, AppKit, WinUI)**: testing means driving a live GUI with accessibility automation — slow, flaky, environment-dependent.
- **Game-engine-style immediate-mode UIs**: fast, but state and view are entangled; there is no stable tree to assert against.

The consequence: UI is the least-tested layer of most systems, including systems where the UI is safety-relevant. In regulated domains (DO-178C, NPR 7150.2), "the display showed the right thing" is a claim teams struggle to produce *evidence* for.

### 1.3 The bet

Libra's bet is that four architectural decisions, made together and enforced everywhere, produce a UI stack where testability is a property of the architecture rather than a discipline imposed on it:

1. **State lives in one place and changes through a closed set of actions.** A single `State` struct whose action methods compile to a serializable `Action` enum and one pure `reduce(state, action) -> Vec<Effect>` function. Every state transition is a plain function call — testable with zero runtime.
2. **Views are data, not code.** Views are declarative markup with a *closed grammar*: bindings and message dispatch, never expressions with side effects, never state mutation. Data can be parsed, diffed, hot-reloaded, statically validated, and compiled — code cannot.
3. **Effects are data, not calls.** Actions never perform I/O. They return *descriptions* of effects (`Effect::Http { .. }`, `Effect::KvSet { .. }`). The runtime executes them in production; tests assert on them and hand-feed results back. Nothing is ever mocked.
4. **Rendering is deterministic.** The renderer is a CPU rasterizer whose output is a pure function of the display list. The same component tree produces the same bytes on every machine, every time. Screenshot tests are byte-exact equality, runnable in any CI container with no GPU and no display server.

Everything else in this document is a consequence of these four decisions.

### 1.4 Design principles

- **Determinism is the product.** Wherever a choice exists between "fast but environment-dependent" and "slightly slower but bit-reproducible," choose reproducible. Speed is recovered through caching and damage tracking, not through nondeterminism.
- **Closed over open.** A closed component set, a closed markup grammar, a closed effect vocabulary. Every closed set is a set the tooling can exhaustively validate at compile time. Extensibility is *not* a goal (see §24); the escape hatches that exist (custom Rust components, the Canvas component) stay inside the determinism contract.
- **The 80% path must be sub-second.** UI development is dominated by view/style iteration. That path must never involve the Rust compiler (see §18).
- **Errors are teaching moments.** Every markup error carries `file:line:col`, the interpreter's message, and a suggestion. In release builds, markup errors *are compile errors*.
- **Steal shamelessly, attribute honestly.** The architecture openly borrows: the Elm Architecture for the loop; Vercel's Native SDK for views-as-data, the dual interpreter/compiled engine, determinism-first rendering, and the embedded automation server; React for the component/props surface; Redux DevTools for time travel.
- **Boring crates.** Every hard subsystem rides a proven crate: `winit`, `taffy`, `tiny-skia`, `parley`, `accesskit`, `redb`. Libra's novelty is the *contract between them*, not reinvented wheels.

### 1.5 Who this is for

The primary user is the author (and teams like the author's): engineers building **operational, data-dense, mission-critical desktop tools** — telemetry dashboards, control surfaces, ops consoles — who need to *prove* the UI behaves correctly, not just believe it. The secondary audience is AI agents: the same determinism that makes tests exact makes the framework legible to agents (see §16).

---

## 2. Prior Art & Influences

| Project | What Libra takes | What Libra rejects |
|---|---|---|
| **Elm** | The Model/Msg/update loop; managed effects (`Cmd`/`Sub`); "make impossible states unrepresentable" | Its own language; JS interop model |
| **Vercel Native SDK** | Views as watched data files; dual interpreter (dev) / compiled (release) engines sharing one grammar; deterministic CPU reference renderer; byte-exact PNG artifacts; embedded automation server; closed component catalog; CLI-owned build graph | Zig; GPU-on-one-platform asymmetry as a permanent state; general-purpose desktop ambition |
| **React** | Component-as-function-of-props; JSX-flavored authoring surface; one-way data flow | Hooks (state hidden in the tree); effects-as-calls (`useEffect`); runtime-only error discovery |
| **Redux / Redux DevTools** | Single store; action log; time-travel debugging; serializable state | Boilerplate (the macro system and codegen absorb it) |
| **Iced (Rust)** | Proof the Elm architecture works well in Rust; component trait shape | Views written in Rust (compile-time in the iteration loop); GPU-first rendering |
| **Slint** | Proof that a compiled UI DSL with an LSP is viable in Rust | Its own full DSL with logic; royalty/licensing model complexity |
| **Dioxus/Blitz** | Proof of demand for React-model native Rust | VDOM diffing focus; HTML/CSS semantics; hooks |
| **tiny-skia / resvg ecosystem** | The deterministic CPU rasterizer itself | — |
| **M4 / LTTB literature** | Pixel-perfect time-series downsampling | — |

**Why not just use Iced/Dioxus/Slint?** Three reasons, honestly stated. (1) None makes determinism a contract — their rendering is GPU-backed and environment-sensitive, so byte-exact CI screenshots are off the table. (2) None separates views into hot-reloadable *data* — all put Rust compilation inside the UI iteration loop (Dioxus mitigates with subsecond hot-patching, but the view is still code). (3) None ships an embedded automation server or treats the accessibility tree as the primary test-query API. Libra is not "another general-purpose framework"; it is a narrow stack where those three properties are the point.

---

## 3. Mental Model: React → Libra

For a developer fluent in React, the translation table:

| React concept | Libra equivalent | Notes |
|---|---|---|
| Component | A `.lx` markup file + optional Rust component fn | Same composition model, same word (§7) |
| JSX | `.lx` markup (JSX-flavored, closed grammar) | See §6 |
| Props | Typed attributes / a props struct | Validated at compile time in release |
| `useState` | Fields on the single `State` struct | State is centralized, not tree-resident |
| `setState` / reducers | **Actions** — methods on `State` in `#[libra::actions]` impls (§5) | Redux Toolkit's `createSlice`, with real types |
| `useEffect` / thunks & sagas | `Effect` values returned by actions; `Sub` for subscriptions | Effects are data (§12) |
| `useEffect` for lifecycle ("while mounted…") | Source elements in markup: `<Frames/>`, `<Interval/>`, `<Stream/>` (§12.5) | Declarations diffed on mount/unmount — data, never code |
| `fetch()` | `Effect::http(...)` | Runtime executes; tests assert |
| `localStorage` | `Effect::kv_set` / `Effect::kv_get` → `redb` | Better than localStorage (§13) |
| `key` prop | `key` attribute on `<for>` | Same purpose (identity across reorder) |
| Conditional render `{cond && <X/>}` | `<if test={cond}>` | Closed grammar form |
| List render `.map()` | `<for each={items} as="item" key="item.id">` | Closed grammar form |
| Context | State slices (§5.7) | No hidden ambient state |
| `useRef` + imperative escape | The data plane (§14) + Canvas component (§7.4) | Purity for logic, throughput escape hatch |
| React DevTools | Time-travel inspector (§18.5) | Every action logged, scrubbing, session export |
| Jest + Testing Library | Unit / component / integration tests (§15) | No mocks, no fake DOM |
| Storybook + Chromatic | `libra gallery` + snapshot tests (§15.5) | No fuzz tolerance, no browser farm |
| Playwright | Replay tests (§15.6) + automation server (§16) | Typed dispatch, byte-exact assertions |

The intended feeling: a React developer reads a Libra app and knows where everything is within minutes. The differences (centralized state, effects-as-data) are the *same* differences a React team adopts voluntarily when it brings in Redux Toolkit + MSW — here they are the defaults, and the tooling is built around them.

### 3.1 The vocabulary (five words, all of them already yours)

Libra's entire authoring surface is described by five nouns, each chosen to mean what a frontend developer already thinks it means:

| Word | Meaning in Libra | Where you know it from |
|---|---|---|
| **State** | The single app-state struct — the one source of truth | React state, the Redux store's state |
| **Action** | One recorded state change. *Authored* as a plain method on `State`; *recorded and replayed* as a serializable enum value (§5.2) | Redux actions — except here they are generated, never hand-written |
| **Effect** | A *description* of I/O returned by an action; the runtime executes it, tests intercept it | `useEffect`'s job, done as data |
| **Component** | Everything that renders: the built-in catalog, `.lx` compositions, Rust components | React components |
| **View** | A `.lx` file — the declarative markup projection of `State` | JSX, minus embedded logic |

Supporting cast, same familiarity rule: **props**, **slices** (§5.7), **subscriptions** (§12.2), the five **test kinds** (§15), and "the **store**" as the informal name for `State` + its actions, as in Redux/Zustand.

Two deliberate naming judgments:

- **Actions, not "state", for the change vocabulary.** The things you write to define state changes live *on* the `State` struct as methods, so the mental model reads as "state and the ways it changes" — but the recorded values themselves are named *actions*, because calling an event "state" would collide with the state it changes. This is precisely the Redux frame every React developer already carries: state + actions + reducer, with the reducer generated (§5.2).
- **Retired jargon.** *Model*, *Msg*, *update*, *Cmd*, *widget*, and *tier* no longer appear on the authoring surface. The Elm loop those words described is still exactly what runs — §5.2 shows the expansion — it just no longer names the API.

### 3.2 Terminology migration (complete — this document and the code now agree)

The rename below shipped in full, landing **snapshot-neutral** throughout (it renamed symbols, never behavior — every committed PNG hash survived every stage). Kept here as the historical map from the founding vocabulary, for reading old commits, old design notes, or anyone's muscle-memory names:

| This document (authoritative, and current code) | Founding name (historical) |
|---|---|
| `State` + `#[libra::state]` | `Model` + `#[derive(LibraReflect)]` |
| Action methods + generated `Action` enum | hand-written `Msg` enum + `update` arms |
| `reduce` / "the reducer" | `update` |
| `Effect` | `Cmd` |
| component, `Component` trait, `libra-components` | widget, `Widget`, `libra-widgets` |
| unit / component / integration / snapshot / replay tests | `tier1_`–`tier5_` prefixes in `tests/tiers.rs` |
| `expect_effect`, `assert_effect!`, `fx::` | `expect_cmd`, `assert_cmd!`, `cmd::` |
| camelCase handler/binding names in `.lx` (§3.3) | PascalCase `Msg`-variant names (`onPress={Reset}`) |

The staged execution record lives in `MIGRATION-NAMING.md` at the repo root — all 8 stages ran, including Stage 8's alias removal and the CI guard that keeps the founding names from reentering (§21).

### 3.3 Naming conventions: React's, verbatim

The `.lx` surface follows JSX conventions exactly; Rust stays idiomatic Rust. The reflect layer bridges the two mechanically — the same discipline as serde's `rename_all = "camelCase"`:

| Surface | Convention | Example |
|---|---|---|
| Components in markup | PascalCase | `<MetricCard />`, `<Button>` |
| Props / attributes | camelCase | `onPress`, `testId`, `maxLength` |
| Handler references in markup | camelCase | `onPress={increment}`, `onChange={filterChanged(value)}` |
| State-field bindings in markup | camelCase paths | `{count}`, `{logFilter}`, `{user.name}` |
| Whitelisted accessors & generated predicates | camelCase | `{items.isEmpty()}`, `<if test={connection.isLive()}>` |
| Rust state fields & action methods | snake_case | `log_filter`, `fn filter_changed(…)` |
| Rust types & generated `Action` variants | PascalCase | `AppState`, `Action::FilterChanged` |

The line to hold: in JSX, **PascalCase means "component"** — so `onPress={Reset}` (the pre-revision style) misreads as a component reference to anyone fluent in React, and handler references are camelCase like every JS function. Each item has **one canonical public name**: markup, the automation protocol (§16), and every diagnostic use the camelCase form; the snake_case Rust name never leaks into `.lx`. The mapping is deterministic, applied identically by both view engines (conformance-covered per §6.4), and typo suggestions cross it: `onPress={filter_changed}` → *did you mean `filterChanged`?*

---

## 4. System Overview

### 4.1 Layer diagram

```
┌─────────────────────────────────────────────────────────────────┐
│  AUTHORING                                                      │
│   src/views/*.lx        JSX-flavored markup (data)              │
│   src/lib.rs            state + actions + libra::app! (§5.8)    │
│   libra.toml             App manifest: windows, permissions,     │
│                         identity, theme                        │
└───────────────┬─────────────────────────────────────────────────┘
                │
┌───────────────▼─────────────────────────────────────────────────┐
│  VIEW ENGINE (one grammar, two engines)                         │
│   dev:     interpreter — parse .lx at runtime, hot reload       │
│   release: codegen — build.rs compiles .lx → static Rust fns    │
│   both →   ViewTree (typed, diffable)                           │
└───────────────┬─────────────────────────────────────────────────┘
                │ reconcile (keyed diff)
┌───────────────▼─────────────────────────────────────────────────┐
│  COMPONENT TREE                                                 │
│   retained tree of typed components; stable structural IDs;     │
│   semantics (role/label/state) → accesskit + test queries       │
└───────────────┬─────────────────────────────────────────────────┘
                │ taffy (flexbox subset)
┌───────────────▼─────────────────────────────────────────────────┐
│  LAYOUT → DISPLAY LIST                                          │
│   DrawCmd stream: rects, rrects, paths, glyphs, images,        │
│   layers, clips + cache actions (damage, layer reuse)           │
└───────────────┬─────────────────────────────────────────────────┘
                │
┌───────────────▼──────────────────────────┬──────────────────────┐
│  EXECUTORS                               │                      │
│   tiny-skia CPU rasterizer (canonical,   │  [future] wgpu       │
│   deterministic, byte-exact)             │  executor — same     │
│   → softbuffer blit into winit window    │  DrawCmd input       │
│   → PNG artifacts in tests               │                      │
└──────────────────────────────────────────┴──────────────────────┘

           ┌──────────────────────────────────────────┐
           │  RUNTIME LOOP (main thread)              │
           │  action queue → reduce() → Effects       │
           │  → rebuild view → diff → layout → paint  │
           └──────┬──────────────────────┬────────────┘
                  │                      │
     ┌────────────▼──────────┐  ┌───────▼────────────────────────┐
     │ EFFECTS RUNTIME       │  │ DATA PLANE                     │
     │ worker pool: http,    │  │ ingest threads → ring buffers  │
     │ kv, fs, timers,       │  │ → downsample at paint;         │
     │ streams; results      │  │ State holds handles only       │
     │ come back as Actions  │  │ (§14)                          │
     └───────────────────────┘  └────────────────────────────────┘

           ┌──────────────────────────────────────────┐
           │  AUTOMATION SERVER (embedded, optional)  │
           │  snapshot / query / dispatch /           │
           │  screenshot / assert over local socket   │
           └──────────────────────────────────────────┘
```

### 4.2 The frame lifecycle

1. **Inputs arrive**: OS events (pointer, key, IME) from winit; effect results from workers; data-plane `Frame` ticks. All are converted to `Action`s and queued.
2. **Drain & reduce**: the runtime drains the queue, calling `reduce(&mut state, action)` per action; returned `Effect`s are handed to the effects runtime.
3. **Rebuild view**: if any action was processed (or a hot reload occurred), the view engine rebuilds the `ViewTree` from the state.
4. **Reconcile**: keyed diff against the retained component tree; components are created/updated/removed; focus and scroll state persist across rebuilds.
5. **Layout**: taffy computes geometry for dirty subtrees only.
6. **Paint**: dirty components emit `DrawCmd`s; the damage tracker computes the union of dirty rects; the executor re-rasterizes only damaged regions, compositing cached layers for the rest.
7. **Present**: blit to the window (softbuffer). In headless/test mode, step 7 writes pixels to a buffer or PNG instead.

Every step is synchronous and deterministic given the action sequence — which is what makes session replay (§18.5) and headless full-loop tests (§15.4) exact.

---

## 5. State & Actions

### 5.1 The core declarations

An application is a state struct and the actions that change it. No event enum to maintain, no dispatch to write — those are generated (§5.2):

```rust
/// All application state. One struct. Serde-derived for
/// hot-reload rehydration, session export, and time travel.
#[libra::state]
pub struct AppState {
    pub count: i64,
    pub telemetry: SeriesHandle,        // data-plane handle, not data (§14)
    pub connection: Connection,          // enum: Disconnected | Connecting | Live { since: Instant }
    pub log_filter: String,
}

/// Every way state can change: plain methods. Pure — no I/O, no clocks,
/// no randomness (§5.3). Return effects, or nothing.
#[libra::actions]
impl AppState {
    pub fn increment(&mut self) { self.count += 1; }
    pub fn decrement(&mut self) { self.count -= 1; }

    pub fn filter_changed(&mut self, value: String) {
        self.log_filter = value;
    }

    pub fn connect(&mut self) -> Effect {
        self.connection = Connection::Connecting;
        Effect::http(Request::get("https://api.example.com/session"), Self::session_loaded)
    }

    // Takes an effect-result payload, so this is a *continuation*: reachable
    // only through the Effect above, never dispatchable by name from markup.
    fn session_loaded(&mut self, result: HttpResult) -> Vec<Effect> {
        // ...
    }

    pub fn frame(&mut self, info: FrameInfo) { /* vsync-coalesced tick (§14) */ }
}

/// Ambient event sources the app subscribes to, derived from state.
pub fn subscriptions(state: &AppState) -> Vec<Sub> {
    match state.connection {
        Connection::Live { .. } => vec![Sub::frames(), Sub::stream("telemetry")],
        _ => vec![],
    }
}
```

Action return types are anything `Into<Vec<Effect>>`: `()`, `Effect`, `Option<Effect>`, or `Vec<Effect>`.

Ambient sources can be declared in Rust (`subscriptions`, §12.2) or directly in markup as mount-scoped **source elements** (§12.5) — most apps use the markup form and never write `subscriptions` at all.

### 5.2 What the macro generates (one expansion, no magic)

`#[libra::actions]` expands to exactly the substrate the framework has always run on — the Elm loop, generated instead of hand-written:

```rust
/// A closed, serializable event set: one variant per action;
/// method parameters become the payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Action {
    Increment,
    Decrement,
    FilterChanged(String),
    Connect,
    SessionLoaded(HttpResult),
    Frame(FrameInfo),
}

/// The reducer: the only place state changes.
pub fn reduce(state: &mut AppState, action: Action) -> Vec<Effect> {
    match action {
        Action::Increment => { state.increment(); vec![] }
        // … one arm per action
    }
}
```

Everything downstream operates on `Action` values — the action log, time travel (§5.6), replay tests (§15.6), automation dispatch (§16), sequence fuzzing (§15.7) — so the ergonomic layer costs nothing in determinism, replayability, or runtime overhead. Reflection for markup handler binding and the automation schema derives from the same expansion (subsuming the standalone `LibraReflect` derive).

**Dispatchability rule.** A method whose parameters are markup-expressible (none, or control payloads: `String`, `bool`, ids) is a **handler** — bindable in `.lx` under its camelCase public name (§3.3): `onPress={increment}`, `onChange={filterChanged(value)}`. A method taking an effect-result type (`HttpResult`, `KvResult`, `io::Result<…>`) is a **continuation** — constructed only by the runtime delivering that result, never by name. This is the unit-variant/payload-variant split the reflect layer already enforced, now derived from signatures instead of maintained by hand.

Hand-writing `Action` + `reduce` remains fully supported — it is literally the expansion output, and slices with intricate cross-field transitions sometimes prefer it. The macro is sugar; the contract is the enum and the reducer.

### 5.3 Purity rules for actions (enforced by convention, checked by lint)

Action bodies MUST NOT: perform I/O; read clocks (`Instant::now`) — time arrives *inside* action payloads (`FrameInfo.now`, `HttpResult.received_at`); use randomness — entropy arrives via `Effect::Random`; spawn threads; block.

These rules are what make unit tests total: any `(state, action)` pair fully determines `(state', effects)`. A custom `clippy` lint (`libra-lint`) flags violations mechanically.

### 5.4 Actions are events, not setters

Name user-intent actions for the *intent* (`connect`, `filter_changed`) and continuations for *what happened* (`session_loaded`) — never for the state they set (`set_connecting`). This is the event-sourcing discipline: the action log is a complete causal history of the session — every entry is a serialized `Action` value — which is what makes the log replayable, auditable, and exportable as a test fixture. (If you have built event-sourced systems, this is the same instinct: actions are events, `State` is the projection.)

### 5.5 Init

```rust
impl AppState {
    pub fn init() -> (AppState, Vec<Effect>) {
        (AppState::default(), vec![Effect::kv_get("session", Self::session_restored)])
    }
}
```

Startup effects (load persisted state, open connections) are ordinary `Effect`s — so startup is testable exactly like everything else.

### 5.6 Time travel & replay as a consequence

Because the reducer is pure and `Action` is serializable, the runtime can record `(action, state_hash)` per step at negligible cost. Replaying an action log against `init()` reproduces the session exactly. This is not a feature built later; it falls out of §5.3 and is wired into the inspector (§18.5) and the test harness (§15.6) from day one.

### 5.7 Scaling state: slices, not fragmentation

Large apps keep one `State` but organize it as nested structs — **slices** — each with its own `#[libra::actions]` impl. The root composes them:

```rust
#[libra::state]
pub struct AppState {
    #[slice] pub charts: ChartsState,    // has its own #[libra::actions] impl
    #[slice] pub log: LogState,
    pub connection: Connection,          // root-owned
}

#[libra::actions]
impl AppState {
    // cross-cutting actions live here, with full-state visibility
    pub fn disconnect(&mut self) -> Effect { /* touches connection AND charts */ }
}
```

The generated root `Action` nests slice actions (`Action::Charts(charts::Action)`), and slice effects lift automatically into the root action space — Redux Toolkit's `createSlice` + `combineReducers`, with the compiler checking the wiring. This preserves the single source of truth while keeping domains separable and separately testable. There is deliberately **no** Context-style ambient state.

### 5.8 App assembly: one declaration

Everything outside state and actions is wiring, and wiring is generated. A complete app crate:

```rust
#[libra::state]
pub struct Counter { pub count: i64 }

#[libra::actions]
impl Counter {
    pub fn increment(&mut self) { self.count += 1; }
    pub fn decrement(&mut self) { self.count -= 1; }
    pub fn reset(&mut self)     { self.count = 0; }
}

libra::app! {
    state: Counter,
    view: "views/counter.lx",       // dev: watched + interpreted; release: the codegen entry
    window: (400.0, 240.0),         // optional; libra.toml wins when both exist
    title: "Libra Counter",
}
```

`app!` generates the wiring and nothing else: the `App` impl (init from `State::init()`, falling back to `Default`), the **view bundle** (the watched dev path and the `include_str!` release embedding, from one declaration), and `main()` in bin crates. It contains no logic and admits none — every field is a literal or a path, so there is nothing in it to test or to break. The same bundle feeds the windowed shell, the harness (`Harness::render(Counter::init())` is unchanged), release codegen, and `libra check` — exactly one place attaches a view to an app. `libra new` scaffolds precisely this shape. `libra.toml` remains the manifest of record for windows/permissions/identity in multi-window apps (§26.3); `app!` is the single-window shorthand.

---

## 6. The View Layer & Markup

### 6.1 The `.lx` file

Views live in `src/views/*.lx` — JSX-flavored files that are **data**. The counter:

```jsx
// src/views/counter.lx
<Column gap={12} padding={16}>
  <Row gap={8} cross="center">
    <Text grow bold>Counter</Text>
    <Button size="sm" variant="ghost" onPress={reset}>Reset</Button>
  </Row>
  <Row gap={8} main="center" grow>
    <Button variant="secondary" onPress={decrement}>-</Button>
    <Text size="xl" testId="count">{count}</Text>
    <Button variant="primary" onPress={increment}>+</Button>
  </Row>
  <StatusBar>count: {count} · {connection}</StatusBar>
</Column>
```

### 6.2 The closed grammar — what markup CAN do

- **Bind** state fields and paths: `{count}`, `{user.name}`, `{items.len()}` (a fixed whitelist of pure accessors: `len`, `isEmpty`, formatting — camelCase in markup per §3.3). Collection fields (`Vec<T>`) bind as iterable paths for `<for>`; there is no index expression (`{series[3]}` is not grammar — iterate, or model a named field).
- **Dispatch** actions by name, with payloads from bindings or literals: `onPress={increment}`, `onChange={filterChanged(value)}` (`value` is the control-provided payload), `onPress={select(item.id)}`. Handlers bind to the reflected action names of §5.2, exposed in camelCase per §3.3 (`fn filter_changed` binds as `filterChanged`).
- **Declare** ongoing sources: `<Frames onTick={frame} />`, `<Interval every={1000} onTick={poll} />`, `<Stream id={feed} onEvent={packet} />` — invisible source elements, subscribed while mounted, cancelled on unmount (§12.5).
- **Conditionals**: `<if test={connection.isLive()}>…<else>…</if>` — tests are bindings or whitelisted predicates; enum variants get generated `is*` predicates (camelCase in markup, §3.3).
- **Iteration**: `<for each={items} as="item" key={item.id}>…</for>` — `key` is required when the collection can reorder; the reconciler uses it for identity.
- **Composition**: `<LogPanel entries={log.visible} onSelect={logSelected} />` — invoking another `.lx` view or Rust component with typed props.
- **Slots**: `<Card><slot/></Card>` children pass-through.
- **Format strings**: `{count:>6}` / `{temp:.2}` — Rust `format!` spec subset, compiled, no runtime parse.

### 6.3 What markup CANNOT do — and why each restriction pays

| Restriction | What it buys |
|---|---|
| No arbitrary expressions | Static validation; the interpreter and codegen stay simple and provably equivalent |
| No state mutation | Views are pure projections; hot reload can never corrupt state |
| No inline closures | Every handler is a named `Action` — greppable, loggable, replayable |
| No side effects | Rebuilding a view is always safe, any number of times |
| No arbitrary computation | Anything complex belongs in the State as a derived field — where it is testable |

The rule of thumb, borrowed intact from Elm: **if you want to compute it, put it on the State.** `visible_items` is a state field maintained by actions (or a memoized derivation), not a filter expression in markup.

The same rule settles the tempting middle ground of letting `.lx` hook into "small reusable pure functions": rejected. That reusable-pure-function layer already exists — it is the actions impl (§5) — and it lives in Rust, where the type system checks it, unit tests exercise it, and the action log records it. A function hook in markup would reopen everything the closed grammar buys: static validation, interp≡codegen equivalence, and hot reload that cannot corrupt state.

### 6.4 One grammar, two engines

This is the single most important DX decision (see §18), lifted directly from the Native SDK design:

- **Dev — the interpreter.** `libra dev` parses `.lx` files at startup and re-parses on change (~1ms). The interpreter walks the AST against the live state to build the `ViewTree`. No compiler in the loop; hot reload preserves all state; parse errors keep the last good view and overlay the diagnostic.
- **Release — the compiler.** `build.rs` parses the *same files with the same parser crate* and emits static Rust: one `fn build(state: &State) -> ViewTree` per view, with bindings compiled to direct field accesses and handlers compiled to `Action` constructors.
- **The no-drift guarantee.** Both engines share one crate (`libra-markup`): one lexer, one AST, one attribute table, one value-conversion module. The codegen backend is a fold over the same AST the interpreter walks. A golden test suite runs every fixture through both engines and asserts identical `ViewTree`s — structurally, node for node, id for id.

### 6.5 Compile-time validation (the part React cannot give you)

Because release codegen has the app's actual types in scope, markup errors become **rustc compile errors** with `.lx` line/column via `#[track_caller]`-style span mapping:

- `onPress={incremnt}` → `error: no action 'incremnt' on AppState — did you mean 'increment'? (counter.lx:9:34)`
- `{cuont}` → `error: no field 'cuont' on AppState — did you mean 'count'?`
- `filterChanged(item.id)` where the action takes `String` but `id` is `u64` → type mismatch at the `.lx` span.
- Unknown element, unknown attribute, missing required `key` on a reorderable `<for>` → compile errors.

In dev, the interpreter produces the *same diagnostics* at reload time (it reflects over the state struct and its actions via the same §5.2 expansion). `libra check` runs the full validation suite in <100ms without building anything.

### 6.6 Parser

Hand-rolled recursive-descent in `libra-markup` (no parser-generator dependency): a JSX-shaped grammar is small, and owning the parser means owning error quality. Spans on every node. The parser is also consumed by the LSP (§18.6) and the formatter (`libra fmt`).

---

## 7. Components

### 7.1 The built-in catalog (closed, curated)

Scoped deliberately to the mission-control/ops domain — approximately 30 components, not 50:

- **Layout**: Column, Row, Grid, Split (draggable), Panel, Scroll, Spacer, Separator
- **Text & display**: Text, Badge, Status, Icon, Image, Markdown (read-only subset), Tooltip
- **Controls**: Button, ButtonGroup, Input, Textarea, Checkbox, Switch, Radio, Select, Combobox, Slider, Stepper
- **Data**: Table (virtual; headless column model à la TanStack Table — column defs, sorting/filtering/grouping live as state, so table logic is unit-testable), VirtualList, Tree, Chart (line/area/scatter/bar; data-plane-aware, §14), Sparkline, Gauge, LogView (virtual, follow-tail)
- **Chrome**: Tabs, StatusBar, Toolbar, CommandPalette, Dialog, Toast, ContextMenu
- **Media**: Video (§9.5) — Phase 5+
- **Sources** (invisible, zero-layout; §12.5): Frames, Interval, Stream
- **Escape hatch**: Canvas (§7.4)

Every component ships with: semantics (role/label/state for accesskit + queries), keyboard behavior, focus behavior, theme-token styling, and its own snapshot fixtures.

### 7.2 Composed components (markup)

A `.lx` file with a props declaration is a component:

```jsx
// src/views/metric_card.lx
<props label: String, value: String, trend: Trend />

<Card padding={12}>
  <Text muted size="sm">{label}</Text>
  <Row gap={8} cross="baseline">
    <Text size="xl" mono>{value}</Text>
    <TrendArrow trend={trend} />
  </Row>
</Card>
```

Props are typed; codegen validates every `<MetricCard …/>` call site.

### 7.3 Rust components

For components needing computation beyond the grammar, implement the `Component` trait in Rust:

```rust
pub trait Component {
    type Props: PartialEq;
    fn measure(&self, props: &Self::Props, constraints: Constraints, ctx: &MeasureCtx) -> Size;
    fn paint(&self, props: &Self::Props, bounds: Rect, ctx: &mut PaintCtx);   // emits DrawCmds
    fn event(&mut self, ev: &ComponentEvent, ctx: &mut EventCtx) -> Option<DynAction>;
    fn semantics(&self, props: &Self::Props) -> Semantics;                     // role, label, state
}
```

Registered Rust components are first-class in markup (`<K8sTopology graph={topo} onSelect={nodeSelected} />`), participate in layout/semantics/testing identically, and — because `paint` emits display-list commands rather than touching pixels — remain snapshot-testable and deterministic by construction.

### 7.4 Canvas: advanced custom UI

`Canvas` is the fully custom escape hatch: the paint context exposes the complete drawing vocabulary (paths, béziers, gradients, glyph runs, images, layers, clips, transforms), plus retained-layer helpers so static portions cache. This is where bespoke visualizations live — topology graphs, timelines, mini-maps, orbital plots. Animation uses renderer-side animated tokens (§9.5) or `Sub::frames()`; the data plane (§14) feeds it at high rate. The contract: Canvas emits commands, never owns pixels — so a custom component's output is exactly as testable as a Button's.

---

## 8. Layout

**taffy** (the flexbox/grid engine from the Dioxus/Bevy ecosystem) computes all geometry.

- Exposed model: flexbox subset + grid — `gap`, `padding`, `margin`, `grow`, `shrink`, `basis`, `main`/`cross` alignment, `wrap`, absolute positioning within a Panel, and `Grid` templates. No floats, no CSS cascade, no stylesheet matching: layout attributes live on the node that owns them.
- Layout is incremental: the component tree marks dirty nodes on prop change; taffy recomputes affected subtrees only.
- Text measurement enters layout through a measure callback into the text stack (§10), with a content-hash cache in front of it (measuring is the expensive path).
- Determinism note: all layout math is f32 with a fixed rounding policy (round-to-nearest-half-up at physical-pixel boundaries) applied *once*, at paint time — so layout results are identical across platforms and never accumulate drift.

---

## 9. Rendering

### 9.1 The display list

The universal currency between components and executors:

```rust
pub enum DrawCmd {
    FillRect   { rect: Rect, fill: Fill },                       // Fill: solid | linear-gradient
    FillRRect  { rect: Rect, radius: Radius, fill: Fill },
    StrokeRect { rect: Rect, stroke: Stroke },
    FillPath   { path: PathId, transform: Affine, fill: Fill },
    StrokePath { path: PathId, transform: Affine, stroke: Stroke },
    GlyphRun   { run: GlyphRunId, origin: Point, color: Color },
    Image      { image: ImageId, dst: Rect, fit: Fit, sampling: Sampling },
    Shadow     { rect: Rect, radius: Radius, blur: f32, color: Color },
    PushLayer  { id: LayerId, bounds: Rect, opacity: f32 },      // cacheable subtree
    PopLayer,
    PushClip   { rect: Rect, radius: Option<Radius> },
    PopClip,
}
```

Paths, glyph runs, and images are interned resources referenced by id — the display list stays small, diffs cheaply, and hashes fast. A frame is `(Vec<DrawCmd>, CacheActions)` where `CacheActions` tells the executor which layers/atlases/geometries to create, reuse, or evict.

### 9.2 The canonical executor: tiny-skia (CPU)

`tiny-skia` rasterizes the display list into an RGBA framebuffer; `softbuffer` blits it into the winit window. Properties this buys:

- **Bit-exact reproducibility** across machines, OSes, CI containers. No GPU, no driver, no display server needed. (tiny-skia is a pure-Rust port of Skia's software pipeline — this is its explicit design goal.)
- **Screenshot tests are `assert_eq!` on bytes** (§15.5). No perceptual-diff thresholds, no "2% fuzz allowed."
- One rendering truth: unlike stacks where the software path is a poor cousin, here the CPU renderer *is* the product.

### 9.3 Making CPU fast enough: damage + layers

The performance strategy (lifted from the Native SDK's cache-action design, and from every browser compositor):

1. **Damage tracking.** Reconciliation marks changed components; paint computes the union of dirty rects; the executor re-rasterizes only those regions. Steady-state cost is proportional to *change*, not screen size.
2. **Layer caching.** `PushLayer` subtrees rasterize once into retained buffers; unchanged layers composite by memcpy. Static chrome (toolbars, panel frames, axis labels) costs ~nothing per frame.
3. **Interned geometry & glyph atlas.** Path tessellation and glyph rasterization cache by content hash; text repaints are atlas blits.
4. **Scroll fast path.** Scrolling a cached layer is an offset change + newly-exposed-strip rasterization.

Realistic envelope (validated by shipping precedents — this is the same regime Native SDK ships on Linux/Windows): full-window redraws at 1080p–1440p in low single-digit milliseconds; damage-tracked dashboard frames well under 2ms. §22 sets hard budgets; §14 keeps data from ever forcing full redraws.

### 9.4 The future GPU executor (insurance, not dependency)

Because executors consume `DrawCmd` streams, a `wgpu` executor can be added later without touching components, markup, tests, or apps. Policy decisions now: (a) the CPU renderer remains the *canonical* output — snapshot tests always assert against it; (b) the GPU path must pass a perceptual-equivalence suite against the reference; (c) it is built only when profiling on a real app demands it. Also available earlier and cheaper: per-component GPU *surfaces* (a video view, a 3D globe) composited as images into the CPU frame.

### 9.5 Video (planned, not early-phase)

Operational deployments eventually show live feeds. The design (specced now, built late):

- **Decode off-thread**: GStreamer/FFmpeg behind a custom effect executor (`Effect::VideoOpen { source, handle }`); decoded RGBA frames land in a bounded frame queue following the data-plane pattern — the State holds a `VideoHandle`, frames never pass through `update`.
- **Present path 1 (CPU)**: the Video component blits the latest frame as an interned image at paint. Comfortably handles one or two 1080p feeds inside the damage budget; frames are full-rect damage for their component only.
- **Present path 2 (GPU surface)**: a feed *wall* or 4K feeds pull §9.4's per-component GPU surface forward — decode-to-texture composited into the frame, bypassing CPU raster for those rects only.
- **Determinism**: in tests, video sources are frame tapes; the component renders a stated frame index, keeping snapshot tests exact. Live playback is inherently wall-clock territory and is excluded from snapshot identity (like animations, it evaluates to a pinned frame in headless mode).

Roadmap: Phase 5+, or pulled forward the moment a real deployment requires feeds.

### 9.6 Animation

Animations do not churn the State. Style transitions (hover fades, panel slides, spring physics) are **renderer-side animated tokens**: a component declares `transition(bg, 120ms, ease-out)`; the executor interpolates between committed values and schedules repaints of that component's damage rect only. State-driven animation (a countdown, a progress value) uses `Sub::frames()` and ordinary actions. Determinism: in test/headless mode, animated tokens evaluate at their target values (or at an explicitly injected clock), so snapshots never race an animation.

---

## 10. Text

Text is where custom UI stacks historically die; the strategy is explicit and staged.

- **Shaping & layout**: `parley` (with `swash` underneath) — the Linebender text stack. Real shaping, font fallback chains, bidi, wrapping, and rich spans. This is a deliberate upgrade over Native SDK's single-font/no-kerning subset: Libra targets ops tools that display arbitrary user/log data, so "one Latin font" is not an acceptable ceiling.
- **Rasterization**: `swash` renders glyphs into the atlas; subpixel positioning quantized to a fixed grid (¼-pixel) for determinism.
- **Determinism policy — bundled fonts only.** The app ships its fonts (default set: one UI face, one mono face; overridable in `libra.toml`). System font fallback is *off by default* because it destroys cross-machine reproducibility; a `system-fonts` opt-in exists for apps that prefer nativeness over byte-exact snapshots (their snapshot tests then pin a font manifest).
- **Editing**: fully specced in §10.1 — plain-text entry, complete. *Still the highest-risk subsystem in the project* (§25), which is exactly why its scope is fenced to plain text and its core is a pure state machine.
- **Monospace fast path**: LogView and Table cells detect pure-ASCII mono runs and skip shaping — the high-volume text path in the target domain is cheap by construction.

### 10.1 Text entry — the full spec (in scope, committed)

Generic text entry is a first-class, fully specced commitment — not a minimal afterthought. The perimeter: **plain text in, plain text out**; styling of entered text is the app's concern (tokens on the component), never the user's (no rich formatting UI).

**Components**: `Input` (single-line) and `Textarea` (multi-line, wrapping, scrolling, resize policies). Shared editing core:

- **Caret & selection**: click/drag, double-click word, triple-click line/paragraph, Shift+arrows/Home/End, Ctrl/Effect+A; multi-line vertical movement with sticky column.
- **Editing operations**: insert, delete (char/word/line variants), cut/copy/paste (`arboard`), drag-select; **per-field undo/redo** (grouped by pause/word boundaries, standard editor feel).
- **Keyboard model**: full platform-conventional bindings (macOS vs Windows/Linux differences respected via a binding table, not scattered ifs).
- **IME**: composition regions with underline styling, candidate-window positioning via winit IME APIs, dead keys; correctness across CJK input methods is a v0.1 acceptance criterion, validated by the hostile corpus.
- **Touch**: tap-to-place caret, selection handles (drag grips), long-press to select word — required for tablet targets; OSK summon/dismiss ties into §19.1 (focus → OSK, with layout hint: `text | numeric | hex | none`).
- **Input affordances for ops tooling**: placeholder, max-length, `mask`/validator hooks (numeric ranges, hex, regex — validation errors as ordinary state), redacted mode (password/secrets, also `#[libra(redact)]`-aware in automation dumps), read-only/disabled states, monospace mode.
- **Scale**: pasting a multi-megabyte log into a Textarea must not jank — rope-backed buffer (`ropey`) above a size threshold, incremental shaping, and the §10 mono fast path.
- **Semantics**: role=TextInput with value/selection exposed to accesskit and to component-test queries (`h.type_text(by::label("Callsign"), "AX-4")` is a first-class harness verb; IME composition is synthesizable in tests).

Everything here is unit- and component-testable: the editing core is a pure state machine (`EditState × EditOp → EditState`) exercised without rendering, and caret/selection rendering is snapshot-covered.

**Explicitly out of scope**: rich/styled text *entry* of any kind (bold buttons, markdown-as-you-type, block editors). If a rich-content flow ever materializes, this editing core is the substrate it would build on — but nothing in the framework anticipates it.
---

## 11. Theming & Design Tokens

- All component styling resolves through **named tokens**: `color.surface`, `color.accent`, `radius.md`, `space.3`, `type.mono.sm`, `elevation.panel`. No hex codes in components or markup (markup may reference tokens: `bg={color.warning.bg}`).
- Themes are token tables (TOML), hot-reloadable in dev, switchable at runtime (`Effect::SetTheme`); the renderer re-resolves live. Two ship built in: `libra-dark` (mission-control default) and `libra-light`.
- Semantic aliasing: `color.telemetry.nominal` → `color.green.400`, so domain themes re-map meaning without touching components.
- Density: a global scale token (`compact`/`normal`) multiplies the space ramp — ops screens want compact.
- Tokens are part of the determinism contract: a snapshot test's identity includes the theme hash.

---

## 12. The Effects Runtime

### 12.1 The Effect vocabulary (closed, versioned)

```rust
pub enum Effect {
    // Network
    Http      { req: HttpRequest, on_done: ActionCtor<HttpResult> },
    WsConnect { url: Url, stream: StreamId },              // events arrive via Sub::stream
    WsSend    { stream: StreamId, data: Bytes },

    // Persistence (§13)
    KvGet     { key: Key, on_done: ActionCtor<Option<Bytes>> },
    KvSet     { key: Key, value: Bytes, on_done: Option<ActionCtor<KvResult>> },
    KvDelete  { key: Key },
    KvScan    { prefix: Key, on_done: ActionCtor<Vec<(Key, Bytes)>> },

    // Files & processes
    FileRead  { path: PathBuf, on_done: ActionCtor<io::Result<Bytes>> },
    FileWrite { path: PathBuf, data: Bytes, on_done: ActionCtor<io::Result<()>> },
    Spawn     { command: Command, stream: StreamId },         // stdout/stderr as stream events

    // Time & entropy (keeping actions pure)
    Timer     { after: Duration, action: Action },
    Interval  { every: Duration, id: TimerId, action_ctor: ActionCtor<Instant> },
    CancelTimer { id: TimerId },
    Random    { bytes: usize, on_done: ActionCtor<Vec<u8>> },

    // Platform
    Clipboard(ClipboardOp), Dialog(DialogSpec, ActionCtor<DialogResult>),
    Notify(Notification), OpenUrl(Url), SetTheme(ThemeId),
    WindowOp(WindowOp),                                    // resize, title, fullscreen…

    // Data plane (§14)
    IngestOpen  { source: SourceSpec, series: SeriesHandle },
    IngestClose { series: SeriesHandle },
}
```

`ActionCtor<T>` is a function pointer `fn(T) -> Action` — serializable by registry id, so effect logs replay.

### 12.2 Subscriptions

`Sub` covers ongoing sources, declared by `subscriptions(state)` and diffed by the runtime (start new, cancel removed): `Sub::frames()` (vsync-coalesced ticks), `Sub::stream(id)` (websocket/process/data-plane events), `Sub::interval(...)`, `Sub::hotkeys(...)`, `Sub::window_events()`. Deriving subs from state means "connect while this screen is open" is a pure function — and testable as one. The same sources can be declared in markup as mount-scoped source elements (§12.5); the runtime diffs the union of both forms.

### 12.3 Execution model

- **Main thread**: winit event loop, reduce, view, layout, paint. Never blocks.
- **Effects workers**: a small thread pool (`ureq` for HTTP keeps the stack synchronous and simple; no tokio in v0 — an async runtime is an implementation detail hidden behind Effect, addable later without API change). Results post back as actions via the runtime channel.
- **Ordering guarantee**: actions from one effect arrive in causal order; cross-effect ordering is explicitly *not* guaranteed — the State must tolerate reordering, and the fuzz harness (§15.7) exploits exactly this to find races at the logic level.
- **Cancellation**: effects carry ids; `Effect::CancelTimer`/`IngestClose`/sub-removal cancel; late results for cancelled ops are dropped by the runtime (never delivered), so `update` never sees zombies.

### 12.4 Why effects-as-data is the testing keystone

A React test of "click connect → fetch → persist → re-render" needs a mock server, mocked storage, and async test plumbing. The Libra equivalent (an integration test, §15.4) is synchronous and mock-free: click Connect, assert the intercepted `Effect::Http` describes the right request, feed an `HttpResult` fixture to its continuation, assert the state and the follow-on `Effect::KvSet`, and assert the rebuilt view shows Live. Every branch — timeout, 500, malformed body — is one more fixture, zero more infrastructure.

### 12.5 Declarative subscriptions — effects as mount-scoped data

The React instinct this serves is `useEffect`'s legitimate core: *"while this UI is on screen, keep X running."* Libra's answer keeps it data. A closed set of **source elements** may appear in markup; they render nothing, occupy no layout, and declare an ongoing source:

```jsx
<if test={running}>
  <Frames onTick={frame} />                      // vsync ticks while mounted
  <Interval every={1000} onTick={poll} />        // ms; literal or binding
  <Stream id={telemetryFeed} onEvent={packet} />
</if>
```

Semantics:

- **Mount = subscribe, unmount = cancel.** After each commit the runtime collects mounted source elements from the ViewTree, unions them with `subscriptions(state)` (§12.2), and diffs the union against the running set — start new, cancel removed. Identity is (element kind, resolved props): the moral equivalent of a React dependency array, diffed by the runtime instead of managed in userland.
- **Purity is preserved, not spent.** Views are pure projections of state, so the mounted set is itself a pure function of state — exactly the property §12.2 already requires. Neither engine *executes* anything at build: interpreter and codegen both fold source elements into inert data nodes, conformance-covered per §6.4.
- **The closed grammar holds.** Props are literals or bindings, nothing computed — a per-series `SourceSpec`, a backoff policy, anything parameterized stays in actions or `subscriptions(state)`. The element set (`Frames`, `Interval`, `Stream`) is closed like the rest of the catalog.
- **Results are ordinary actions.** Ticks and events arrive through the recorded channel, so replay, time travel, and sequence fuzzing see them like every other action; `fire_timer`/`dispatch` drive them in tests unchanged.
- **Testable as data.** Source elements appear in the semantics tree under a test-only role (excluded from the AccessKit projection): `h.get(by::source("frames")).assert_mounted()` is a component-test assertion, and a paused dashboard asserts the absence.
- **Snapshot-neutral by construction** — no DrawCmds, no layout node — and hot reload re-diffs declarations like any other view edit: state preserved, sources started or cancelled to match the new markup.

What this is **not**: hooks as code. No state lives in the tree, nothing executes at render, and there is no user-supplied closure — a declaration names an action; the runtime owns the lifecycle. (Markup-declared *queries* — `<Query>` à la Relay — would ride the same mechanism but need dedup/waterfall design with `libra-query`; deferred, see §26.)

---

## 13. Persistence

### 13.1 The KV store (localStorage, but honest)

Backed by **redb** (pure-Rust, ACID, MVCC, single-file B-tree) in the platform app-data directory (`directories` crate: `%APPDATA%`, `~/Library/Application Support`, `~/.local/share`).

- API is the `Effect::Kv*` family — string-ish keys, byte values, serde helpers (`kv_set_json("layout", &state.layout)`).
- Properties localStorage never had: transactional writes, no 5MB ceiling, no string-only values, no synchronous main-thread jank (all Kv effects run on workers), prefix scans.
- Conventions: `settings/…`, `session/…`, `cache/…` prefixes; a schema-version key with a `migrate(old_version, kv)` hook called before `init`'s effects run.

### 13.2 State snapshots

Because `State: Serialize`, the runtime provides `Effect::SnapshotState { key }` and rehydration at startup — used for hot-reload state preservation (§18.2), crash-recovery ("restore session?"), and session export for bug reports (§18.5). Snapshots exclude data-plane buffers (handles reattach; buffers are re-fed or empty).

### 13.3 Beyond KV

Apps needing relational queries add `rusqlite` behind app-defined effects (the Effect vocabulary is closed *in the framework*, but apps register custom effect executors under the same data-not-calls contract — the one sanctioned extension point, because it preserves testability: a custom effect is still just data you assert on). The same mechanism carries messaging fabrics: a NATS executor (`Effect::NatsPublish`, `Sub::nats(subject)`) or a DDS client runs in-process — notably collapsing the sidecar-bridge pattern (Rust sidecar translating DDS/NATS for a TypeScript UI) that non-Rust UI stacks force.

---

## 14. The High-Rate Data Plane

The subsystem that makes Libra fit for telemetry-class workloads — and the sharpest deliberate deviation from "everything through the loop."

### 14.1 The problem it solves

A 10 kHz telemetry stream pushed through `Action` dispatch would mean 10,000 state clones, view rebuilds, and diffs per second — architectural malpractice. Purity is for *control*; throughput needs a parallel lane.

### 14.2 Architecture

```
sources (ws / udp / sockets / files / generators)
        │  ingest threads (owned by the runtime, opened via Effect::IngestOpen)
        ▼
   RingBuffer<Sample> per series      // SPSC, lock-free, fixed capacity,
        │                             // monotonic sequence numbers
        │            ┌────────────────────────────────────┐
        ├──────────► │ Chart/LogView components read at  │
        │   paint    │ paint time via SeriesHandle:       │
        │            │ window query → downsample → draw   │
        │            └────────────────────────────────────┘
        ▼
   watermarks/alerts → coalesced control Actions (ThresholdCrossed, SourceStalled)
```

- **The State holds handles, not data**: `SeriesHandle` = series id + view window + pause/live flag + downsample policy. Control decisions (pause, zoom, which series visible) are pure actions; samples never enter `update`.
- **One `frame` action per vsync** (via `Sub::frames()`), carrying coalesced info (now, dropped-sample counts, buffer watermarks). 10 kHz ingest → 60 Hz of action traffic, constant regardless of rate.
- **Paint-time reads**: the Chart component queries the buffer for its window, downsamples, and emits geometry. Reads are wait-free (SPSC + sequence-validated snapshot reads).

### 14.3 Downsampling: M4

Charts downsample with **M4** (per pixel column: first, last, min, max) — the published result is that M4 at target width is *pixel-identical* to rendering every point for line charts, at a bounded ~4×width points. A 10M-point series draws as ~8K points. LTTB is available as a policy for scatter aesthetics. Downsampled geometry caches by (series generation, window, width) — panning reuses; zooming recomputes only the delta.

### 14.4 Determinism at the boundary

The data plane is the *one* nondeterministic region, so its boundary is fortified: (a) in tests, ingest sources are replaced by **recorded tapes** (`.ltape` files: timestamped sample logs) played through the same ring buffers — a chart snapshot test feeds a tape and asserts bytes; (b) buffer reads are versioned — a paint that races a writer retries the snapshot read, so rendered frames are always internally consistent; (c) `Frame` actions carry everything `update` is allowed to know about time.

### 14.5 Log-volume text

LogView follows the same pattern: ingest → ring buffer of lines → virtual window render → follow-tail as a handle flag. Filtering runs on a worker (`Effect`-issued, result action carries a match-index generation), so a regex over a million lines never blocks paint.

---

## 15. Testing Architecture

The reason the framework exists. Five kinds of test — named for what a frontend developer already calls them, not for a numbering scheme — all headless, all runnable in a bare CI container, all deterministic. Target: an app's full suite in **seconds**.

| Kind | Exercises | Lives in | JS-world analog | What's different here |
|---|---|---|---|---|
| **Unit** | Actions: pure state transitions + returned effects | `#[cfg(test)]` mod next to the slice | Jest reducer tests | Actions are plain methods — no store setup, no dispatch boilerplate |
| **Component** | The real view tree: queries, clicks, typing | `tests/components.rs` | Testing Library | Real tree, real hit testing — no jsdom, no `act()` |
| **Integration** | Full flows across effects | `tests/integration.rs` | MSW / saga tests | No mocks at all: effects intercepted as data, results hand-fed, fully synchronous |
| **Snapshot** | Rendered pixels | `tests/snapshots.rs` | Chromatic / jest-image-snapshot | Byte-exact PNG equality, zero tolerance thresholds |
| **Replay** | Recorded sessions end-to-end | `tests/replays/` | Playwright traces | Exact re-execution: final state hash + final frame bytes |

Test names are plain descriptions (`fn connect_requests_session()`); the kind lives in the file, never in a prefix.

### 15.1 The determinism contract (what makes every kind exact)

1. Actions are pure (§5.3) — enforced by lint.
2. Views are data with no computation (§6.3).
3. Effects are data; the runtime is the only I/O actor (§12).
4. Rendering is a pure function of the display list (§9.2); text quantized (§10); fonts bundled; animations evaluate at targets or injected clocks (§9.5).
5. High-rate data crosses one fortified boundary (§14.4).

### 15.2 Unit tests (actions)

```rust
#[test]
fn connect_requests_session() {
    let mut s = AppState::default();
    let fx = s.connect();                          // actions are plain methods
    assert_eq!(s.connection, Connection::Connecting);
    assert_effect!(fx, Effect::Http { req, .. } if req.url.path() == "/session");
}
```

Plain `cargo test`. No runtime, no framework imports beyond assert helpers; dispatching through the generated enum (`reduce(&mut s, Action::Connect)`) is equivalent when a test wants the recorded form. Milliseconds for hundreds of tests. This is where most of an app's tests live.

### 15.3 Component tests (headless tree queries)

```rust
#[test]
fn count_renders_and_increments() {
    let mut h = Harness::render(AppState::init());
    h.get(by::test_id("count")).assert_text("0");
    h.click(by::role(Role::Button).with_text("+"));
    h.get(by::test_id("count")).assert_text("1");
}
```

The harness builds the real view (interpreter in test builds), lays it out at a virtual viewport, and dispatches through the real routing — a click hit-tests real geometry and dispatches the component's real action. Queries run against the semantics tree (§17): `by::role`, `by::label`, `by::test_id`, `by::text` — the same selectors Testing Library made standard, resolved against the same tree assistive tech sees.

### 15.4 Integration tests (full flows, no mocks)

```rust
#[test]
fn connect_flow_persists_session() {
    let mut h = Harness::render(AppState::init());
    h.click(by::text("Connect"));
    let req = h.expect_effect(fx::http());            // intercepted, not executed
    h.resolve(req, fixtures::session_ok());           // feed the continuation
    h.get(by::test_id("status")).assert_text("Live");
    h.expect_effect(fx::kv_set("session/current"));
    h.expect_no_effects();
}
```

The harness intercepts effects into a script queue; the test plays results back in any order it wants — including adversarial orders (§15.7). An entire fetch→parse→persist→render flow runs synchronously in microseconds.

### 15.5 Snapshot tests (byte-exact pixels)

```rust
#[test]
fn dashboard_nominal() {
    let mut h = Harness::render(AppState::init()).viewport(1280, 800).theme("libra-dark");
    h.feed_tape("fixtures/telemetry_nominal.ltape");
    h.dispatch(Action::Frame(FrameInfo::at(t(3.0))));
    h.assert_snapshot("dashboard_nominal");            // PNG byte equality
}
```

Snapshots are deterministic PNGs (fixed encoder settings) committed to the repo. Failure output: expected / actual / diff-heatmap triplet. `libra test --bless` re-records after intentional changes. Because rendering is bit-exact, there are **no tolerance thresholds** — a one-pixel change is a real change. This includes *charts*: recorded tape in, rendered plot bytes out — regression tests for visualizations, which effectively no mainstream stack offers.

### 15.6 Replay tests (recorded sessions)

Any recorded session (§18.5) exports as a `.lreplay` (init snapshot + action log + effect results + tapes). `libra test --replay bug_1042.lreplay` re-runs it and asserts final state hash + final frame bytes. A user's bug report becomes a committed regression test verbatim.

### 15.7 Fuzzing & property tests

- **Action-sequence fuzzing**: `proptest` generates action sequences (and adversarial effect-result orderings per §12.3) against invariants (`no panic`, `state.validate()`, "paused chart never advances").
- **Markup fuzzing**: hostile `.lx` corpora against the parser (must never panic, must always span-diagnose).
- **Text hostile corpus**: bidi, ZWJ sequences, combining-mark storms, 10MB single lines through Input/LogView.

### 15.8 CI shape

`cargo test` runs all five kinds on a bare `ubuntu-latest` — no Xvfb, no GPU, no browser. Snapshot artifacts upload on failure. Coverage of the actions layer is meaningful line coverage of actual behavior, reportable against requirements. Traceability is first-class: tests carry `#[libra::verifies("REQ-UI-042")]` annotations; `libra test --trace-matrix` emits requirement → test → artifact (snapshot PNG / replay hash) as JSON+HTML — the evidence bundle compliance postures like NPR 7150.2 or DO-178C-adjacent processes ask for, generated from the suite that already runs in CI. External traceability harnesses consume the same JSON.

---

## 16. The Automation Server

Every §15 kind tests *inside* the process. The automation server drives the *real running app* from outside — the layer that makes Libra apps legible to external harnesses and AI agents.

- Enabled by `libra dev` (and `--automation` on release builds when the manifest permits). Listens on a local unix socket / named pipe; JSON-line protocol.
- **Verbs**: `snapshot` (the full semantics tree with geometry, as JSON), `query` (selector → node ids), `dispatch` (typed action by public camelCase name + payload — validated against the reflected `Action` schema, §3.3), `input` (synthetic pointer/key at coordinates), `screenshot` (deterministic PNG of the live framebuffer), `state` (redacted-by-annotation state dump), `perf` (frame timings).
- Because dispatch is *typed actions*, an agent doesn't scrape pixels to press a button — it can act at the semantic level, with the pixel level available for verification. `libra skills` emits an agent-skill bundle documenting the app's action schema, view ids, and protocol — the app self-describes.
- Security: off in release unless manifested; local-only; peer-credential checked; `#[libra(redact)]` state fields never serialize over it.

---

## 17. Accessibility

Accessibility is load-bearing, not charitable: the semantics tree is simultaneously the a11y surface and the test-query index, so it cannot rot.

- Every component emits `Semantics { role, label, value, state, actions }`; composed views inherit/override via markup attributes (`label=`, `testId=` lives here too).
- The tree feeds **accesskit**, which bridges to VoiceOver / Narrator / AT-SPI — real screen-reader support on all three platforms from one tree.
- Keyboard: full tab-order/focus-ring system in the component layer; CommandPalette and hotkey subs make ops tools keyboard-first.
- `libra check --a11y` audits statically (labels on interactive components, contrast against theme tokens, focus reachability), and the audit runs in CI.


---

## 18. Developer Experience & Tooling

### 18.1 The CLI

```
libra new <app>        scaffold (managed build — no build files to maintain)
libra dev              run with hot reload + inspector + automation server
libra check            validate all .lx + libra.toml + a11y audit, <100ms, no build
libra test             all five §15 kinds; --bless re-records snapshots; --replay <file>
libra build            release binary (codegen engine, LTO)
libra package          platform bundles (§20)
libra fmt              canonical .lx formatting
libra skills           emit agent-skill bundle (§16)
```

The CLI owns the build graph for the default path; `libra eject` writes a plain Cargo workspace you own thereafter.

### 18.2 The iteration loop (the DX centerpiece)

- **Edit `.lx`** → watcher re-parses (~1ms) → window updates <1s, **state preserved** (the view is a projection; the state never moved). Parse/validation error → last good view stays, overlay shows `file:line:col` + suggestion.
- **Edit theme TOML** → live re-resolve, no reparse.
- **Edit Rust** (actions, `State`, Rust components) → incremental rebuild (thin app crate, framework precompiled: target 2–5s) → runtime snapshots the state (serde), restarts, rehydrates → session continues. State shape changed? Rehydration falls back per-field, defaults for new fields, and says so.
- **The rule that protects this**: view codegen runs *only* in release builds. Dev builds never pay proc-macro/codegen cost for view edits. (This is the trap most Rust UI DX falls into, and the single Native SDK decision most worth copying.)
- Terminal 2 runs `cargo watch -x test`: because unit, component, and integration tests are pure and headless, the suite finishes in a few hundred ms — logic TDD faster than alt-tabbing to the window.

### 18.3 Errors

Dev-overlay and rustc errors share one diagnostic database (code, span, message, suggestion, docs link). `libra check` in the editor-save path surfaces everything the release build would reject — no "worked in dev, failed in release" class.

### 18.4 Debugging

- `libra dev` opens the **inspector** (itself a Libra app): component tree with live geometry/semantics, damage-rect visualization, layer-cache view, frame timings, effect queue.
- Standard Rust debugging applies untouched to actions and components (lldb/gdb, `dbg!`), because they are plain functions.

### 18.5 Time travel & session capture

The runtime records `(action, state_delta_hash)` continuously (ring-bounded). The inspector scrubs backward/forward — every historical frame re-renders exactly (purity + determinism). `Export session` writes a `.lreplay`; a bug report is an attachment that replays locally and commits as a replay test. This is Redux DevTools' promise, kept all the way down to pixels.

### 18.6 Editor tooling (staged honestly)

- **Day 1**: full rust-analyzer for all logic; TextMate grammar for `.lx` (syntax highlight in VS Code/Zed/Helix — an afternoon); `libra check` on save.
- **v0.2**: `libra lsp` — completion (elements, attributes, State fields, Action variants), hover docs, go-to-definition (binding → State field, handler → Action variant), diagnostics as-you-type. Weeks of work, high leverage, deferrable because `libra check` already gives exact diagnostics.

---

## 19. Platform Integration

| Concern | Crate / approach |
|---|---|
| Windows & event loop | `winit` (also: multi-window, monitors, DPI) |
| Framebuffer present | `softbuffer` |
| Menus (native menubar, context) | `muda` |
| System tray | `tray-icon` |
| Native file/message dialogs | `rfd` |
| Clipboard | `arboard` |
| Notifications | `notify-rust` / platform APIs |
| App directories | `directories` |
| Global hotkeys | `global-hotkey` |

Policy: window *chrome*, menus, and dialogs are native (they must feel like the OS); window *content* is Libra-rendered (it must be deterministic). DPI: layout in logical pixels; physical rounding once at paint (§8); per-monitor scale changes are just damage. IME caveats per platform tracked in §25.

### 19.1 Touch (first-class, not an afterthought)

Target hardware includes touch-first Linux tablets, so touch is a Phase 1–2 concern:

- **Gesture recognizer** in `libra-components`: tap, double-tap, long-press, drag, momentum/fling scroll (with the scroll fast path of §9.3), and pinch — pinch is wired into Chart zoom and Canvas transforms. Gestures resolve to ordinary actions; the component-test harness synthesizes gesture sequences, so touch interactions are as testable as clicks.
- **Hit-target tokens**: `touch.target.min` (44 logical px default) enforced by `libra check --a11y` on interactive components; a `touch`/`glove` density mode scales targets and spacing up the same way `compact` scales down.
- **On-screen keyboard — the honest gap**: winit delivers touch events but does not summon an OSK, and Linux OSK integration (squeekboard/onboard/portal) is fragmented. The reliable path: a **framework-owned OSK component** (deterministic, themeable, testable, layouts for text/numeric/hex) that Input/Textarea summon on touch focus; system-OSK integration is an opt-in behind `libra.toml [platform.linux] osk = "system"`. Risk tracked in §25.
- **Hover-independence audit**: `libra check` flags any interaction reachable only via hover (tooltips get long-press equivalents automatically).

---

## 20. Packaging & Distribution

- `libra package` wraps `cargo-packager`: `.app`/DMG (macOS), MSI/NSIS (Windows), AppImage/deb (Linux). Icon pipeline from one square PNG.
- Signing/notarization: config-file driven (identities in `libra.toml [package.sign]`), documented per platform; CI recipes ship in the scaffold.
- Binary size expectation: single-digit MB (no browser, no JS runtime; fonts dominate).
- Auto-update: **out of scope v0** (§24) — the packager emits update-feed metadata so `cargo-packager`-compatible updaters slot in later.

---

## 21. Workspace & Crate Layout

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
└─ conformance/        golden fixtures: interp≡codegen suite, snapshot corpus, hostile inputs
```

Dependency spine: `markup` and `reflect` at the root; `interp` and `codegen` both depend on `markup` only (the no-drift guarantee is structural); apps depend on `libra` (a facade crate) + `libra-test` (dev-dep). The app crate stays thin so §18.2's rebuild target holds.

> **Naming migration (§3.2): complete.** The mechanical rename ran to completion per `MIGRATION-NAMING.md`'s stage-by-stage checklist, landing snapshot-neutral throughout (every committed PNG hash survived every stage). The shipped crates now match the names above: `libra-components` (not `libra-widgets`), `Component` (not `Widget`), State/Action/`reduce`/`Effect` (not Model/Msg/update/Cmd), and named test-kind files — `tests/unit.rs`, `tests/components.rs`, `tests/integration.rs`, `tests/snapshots.rs` — rather than `tier*_` prefixes in one `tests/tiers.rs`. Stage 8 removed the deprecated pre-rename aliases and the legacy PascalCase markup-handler acceptance (`onPress={Reset}` is now a load-time/compile-time error suggesting the camelCase form, enforced identically by both engines); a CI naming guard (`.github/workflows/naming-guard.yml`) blocks the old names from reentering. `MsgCtor`/`Widget`/`ReflectModel`/`ReflectMsg`/`tier*_` survive only as historical references in this document and in `MIGRATION-NAMING.md` itself.

---

## 22. Performance Budgets

Budgets are CI-enforced (criterion benches + frame-timing assertions in snapshot-harness runs on a pinned reference machine class):

| Metric | Budget |
|---|---|
| `.lx` reparse + revalidate (dev) | < 5 ms per file |
| Hot-reload edit → visible | < 500 ms |
| App-crate incremental rebuild (logic edit) | < 5 s |
| `reduce` dispatch overhead (runtime, excl. app logic) | < 5 µs |
| Reconcile + layout, 1k-component tree, small delta | < 1 ms |
| Full repaint 1440p (cold) | < 8 ms |
| Damage-tracked dashboard frame, 12 live charts @60fps | < 2 ms |
| M4 downsample, 1M points → 1200 px | < 3 ms (cached: ~0) |
| Unit+component+integration suite, 500 tests | < 2 s |
| Snapshot test, 1280×800 | < 50 ms each |
| Cold start to first frame | < 150 ms |
| Release binary (counter) | < 8 MB |

Two standing benchmark apps: `dashboard` (data-plane stress: 16 series × 10 kHz tape) and `log-explorer` (1M-line corpus, live filter).

---

## 23. Roadmap

**Phase 0 — Spike (weeks 1–2).** `libra-core` loop + `libra-markup` parser + interpreter + minimal components (Column/Row/Text/Button) + taffy + tiny-skia + winit window. Counter runs; unit and component tests pass; a snapshot test passes byte-exact in a container. *Go/no-go gate: the determinism claim proven end-to-end.*

**Phase 1 — The testable foundation (months 1–2).** Effects runtime + redb KV + Harness with effect interception (integration tests) + snapshot infrastructure with bless/diff + hot reload with state rehydration + error overlay + `libra check`. `Remote<T,E>` + error boundaries land here. The counter-that-fetches-and-persists example, fully tested at every kind.

**Phase 2 — The data plane (months 2–4).** Ring buffers, ingest, tapes, M4, Chart/Sparkline/LogView/VirtualList/Table, damage tracking + layer cache hardened, `Sub::frames`, the `dashboard` benchmark app hitting §22 budgets.

**Phase 3 — Release engine & polish (months 4–6).** `libra-codegen` + conformance suite (interp≡codegen), compile-time markup errors, the full §10.1 text-entry spec to v0.1 acceptance (Input + Textarea, IME, touch selection, undo, validators, large-paste), remaining catalog components, accesskit integration, theming, `libra package`, `libra-query`, `FormState`, screen-stack helper, `libra gallery`.

**Phase 4 — Legibility & documents (months 6–8).** Automation server + skills emitter, time-travel inspector, `.lreplay` replay tests, the §3.2 naming migration (aliases → hard rename, snapshot-neutral), the ergonomics layer (`#[libra::actions]` §5.2, `libra::app!` §5.8, collection bindings §6.2, source elements §12.5 — staged plan in `PLAN-ERGONOMICS.md`), `libra lsp`, docs site generated from the component catalog's own fixtures.

**Phase 5+ — Video** (§9.5) and, if demanded, the wgpu executor (§9.4) and mobile platform glue (§24).

**v0.1 ships after Phase 3; Phase 4 is what makes it distinctive in public.** Phases assume nights-and-weekends solo pace; compress accordingly if staffed.

---

## 24. Non-Goals

Stated so scope creep has to argue with a document:

- **Not general-purpose.** No ambition to host arbitrary consumer apps; the catalog and grammar serve data-dense ops tools.
- **No plugin/extension system.** Custom Rust components and app-defined effect executors are the only extension points, both inside the determinism contract.
- **No web target, no iOS/Android** in v0.x. Everything above the platform layer (raster, layout, text, loop) is mobile-portable; the gap is platform glue (winit maturity, touch/IME, lifecycle, store packaging) — a Phase 5+ project, not a redesign. Note: Linux touch tablets are NOT mobile in this taxonomy — they are the Linux target plus touch input, in scope from Phase 1.
- **No WebView embedding.** If a surface needs a browser, this is the wrong framework for that surface.
- **No CSS, no stylesheets, no cascade.** Tokens + node attributes only.
- **No GPU renderer until profiling demands it** (§9.4), and never as the canonical test target.
- **No async runtime in the public API.** Concurrency is the runtime's problem; apps see actions.
- **No auto-updater, no i18n framework** in v0.x (text *display* is international from day one; *editing* scope is §10's).
- **No rich-text editing of any kind** — no block editors, no markdown-as-you-type, no formatting UI. Generic plain-text entry is fully specced and committed (§10.1); styled text is display-only.
- **Video is planned, not early** (§9.5): decode path specced, built Phase 5+ or when a deployment demands it — no longer a hard non-goal.

---

## 25. Risks & Mitigations

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | **Text editing & IME** — the historically fatal subsystem | High | Ride parley/swash (don't invent shaping); scope fenced to plain-text entry (§10.1) — the rich-text death traps are excluded by policy; pure editing state machine tested without rendering; hostile corpus + CJK acceptance criteria from day 1 |
| 2 | **CPU rendering ceiling** — a real app outgrows the raster budget | Medium | Damage+layers first (usually sufficient); per-component GPU surfaces second; wgpu executor third — architecture already decoupled, decision deferred to profiling evidence |
| 3 | **Interpreter/codegen drift** | Medium | Structural prevention (one parser crate), golden conformance suite in CI, fuzzed equivalence |
| 4 | **Determinism erosion** (a dependency update changes rasterization) | Medium | Pin tiny-skia/swash exactly; snapshot corpus is the canary; version-bump PRs re-bless deliberately, reviewed as diffs |
| 5 | **Solo-scope overrun** | High | §24 exists; Phase-0 gate is honest; the catalog is 30 components not 50; DX polish (LSP) staged behind shipping the test ladder |
| 6 | **Ecosystem overlap** (Dioxus/Iced ship a determinism story) | Low-Med | Libra's moat is the *combined* contract (§15.1) + data plane + automation; if an incumbent gets there first, Libra's test/automation layers were designed as separable crates — pivot to riding them |
| 7 | **winit gaps** (IME quirks, platform behaviors) | Medium | winit is the ecosystem's shared battlefield — issues are known and workarounds documented; libra-platform isolates all of it |
| 8 | **Serde rehydration friction** as State evolves in dev | Low | Per-field fallback + defaults + explicit report; worst case is a clean restart, never corruption |
| 9 | **Linux touch/OSK fragmentation** on tablet targets | High | Framework-owned OSK component as the guaranteed path (§19.1); system-OSK strictly opt-in; gesture layer rides winit touch events which are mature on Linux; touch test synthesis in the harness from Phase 2 |
| 10 | **Team adoption risk** (theoretical → production) | Medium | Adoption path in Appendix D: internal ops tool pilot first, coexistence with existing Tauri surfaces, closed markup grammar keeps non-Rust contributors productive and safe |

---

## 26. Open Questions

1. **`.lx` expression whitelist breadth** — how many pure accessors before the grammar stops being "closed"? Start minimal (`len`, `isEmpty`, format specs, generated enum predicates); every addition needs a use case the State-field pattern genuinely can't serve.
2. **~~Msg payload ergonomics~~ — resolved by the action layer (§5).** Payloads are method parameters; the struct-vs-tuple-variant question dissolves into signature style, and markup dispatch follows the reflected signature.
3. **Multi-window model — resolved: one State, per-window view roots.** Ground-control consoles are multi-monitor; a single source of truth across a window wall is the point. Each window declares a root view in `libra.toml`; all windows dispatch into one action queue; damage/paint are per-window; snapshot tests address windows individually. Per-window state fragmentation is rejected for the same reason Context was.
4. **`Frame` cadence off-vsync** — headless/test mode uses injected clocks; should `libra dev` offer a fixed-step mode for reproducing timing bugs live?
5. **Tape format** — bespoke `.ltape` vs. embedding something existing (Arrow IPC?) for interop with the analysis ecosystem.
6. **Markup-declared queries** — should `<Query key={...} into={userLoaded}>` join the §12.5 source elements? Same mechanism (declaration diffed on mount), real precedent (Relay), but it moves fetch policy into views and needs dedup/waterfall semantics designed jointly with `libra-query`. Deferred past v0.1; revisit with a real app's fetch graph in hand.
7. **Name — resolved: Libra.** Before first public publish, verify crates.io availability for `libra`/`libra-*` (a `libra` crate may exist from the 2019 Facebook Libra era — if squatted, publish under `libra-ui`/`libraui` and keep the brand) and check domain options (libra.dev, libra-ui.dev).

---

## Appendix A — The determinism contract, as a checklist

A change is contract-safe iff: no I/O or clocks enter actions; no computation enters markup; no effect executes outside the runtime; no draw output depends on machine, driver, thread timing, or wall clock; every new component emits semantics; every new effect is serializable and interceptable; every new render feature lands with a snapshot fixture.

## Appendix B — Glossary

**State** the single app-state struct (né *Model*) · **Action** one recorded state change — authored as a method on `State`, recorded as data (né *Msg*) · **Reducer** the generated `reduce(state, action) -> Vec<Effect>` (né *update*) · **Effect** effect description (né *Cmd*) · **Sub** ongoing event source · **Slice** nested state struct with its own actions (§5.7) · **Component** anything that renders (né *widget*) · **ViewTree** typed output of a view engine · **Component tree** retained UI state · **DrawCmd** display-list op · **Executor** display-list consumer · **Tape** recorded sample log · **Replay** recorded session · **Bless** accept new snapshot truth · **Harness** in-process test runtime · **Semantics** role/label/state node (a11y + queries) · **Source element** invisible markup element declaring a mount-scoped subscription (§12.5) · **Unit / component / integration / snapshot / replay tests** the five-kind ladder (né *Tiers 1–5*).


## Appendix C — Second-round React ecosystem borrowings (adopted)

| Borrow | Source | Libra form | Phase |
|---|---|---|---|
| Query cache | TanStack Query | `libra-query`: `Effect::Query { key, req, policy }` — dedup, stale-while-revalidate, retry/backoff, invalidation (`Effect::Invalidate(key)`); cache state arrives as actions; fully interceptable in integration tests | 2–3 |
| Async-state type | Suspense (via Elm's RemoteData) | `Remote<T, E> = NotAsked \| Loading \| Success(T) \| Failure(E)` in `libra-core`; markup `<match>` renders per-variant; makes loading/error impossible-states unrepresentable | 1 |
| Component gallery | Storybook | `libra gallery`: generated app rendering every catalog component × state × theme from the components' own snapshot fixtures; doubles as PR visual-review surface | 3 |
| Error boundaries | React error boundaries | `catch_unwind` at the Rust-component/Canvas paint boundary → error placeholder component + dev-overlay backtrace; interpreter errors already contained by design | 1 |
| Form state | React Hook Form | `FormState<T>` model helper: per-field dirty/touched/error, pure validators, submit gating — plain state machinery, unit-testable | 3 |
| Screens/routing | React Router | `Screen` enum + navigation-stack helper (push/pop/replace as actions); `libra://` deep-link protocol maps URLs → screen state (post-v0.1) | 3 |
| Mount-scoped effects | `useEffect` (lifecycle core) | Source elements `<Frames/>`/`<Interval/>`/`<Stream/>` (§12.5): declarations diffed on mount/unmount; results arrive as recorded actions — never code in the tree | 4 |
| Zero-boilerplate app entry | `create-react-app` / Vite scaffold ethos | `libra::app!` (§5.8): state + actions + one wiring declaration is a complete app | 4 |

Evaluated and rejected: Context (ambient hidden state), hooks as *code* in any form — state in the tree and effects executed at render stay excluded; the §12.5 source elements are the data-shaped answer to `useEffect`'s legitimate core — concurrent rendering/transitions (damage tracking + the data plane already address the underlying contention), portals (the overlay layer in `libra-components` covers dialogs/toasts/menus), CSS-in-JS (tokens only).


## Appendix D — Team & Use-Case Fit (ground/crew operations reference case)

The reference use case: a React/TypeScript team (with Rust capability) building ground-control and crew-tablet operations software — telemetry-dense, touch-capable, compliance-bearing, multi-window. This appendix maps the existing stack and team onto Libra and defines the adoption path.

### D.1 Stack translation

| Existing | Libra equivalent | Transfer quality |
|---|---|---|
| React components / JSX | `.lx` markup, typed props, composition, slots | Near 1:1 (surface preserved deliberately, §3) |
| Redux Toolkit / reducer patterns | Actions + the generated reducer (§5) | 1:1 — it is this |
| TanStack Query | `libra-query` (keys, staleness, invalidation, retry) | Near 1:1 vocabulary |
| TanStack Table | Table component's headless column model (§7.1) | Near 1:1 |
| TanStack Router | `Screen` enum + nav stack; `libra://` deep links later | Conceptual |
| vanilla-extract (typed tokens/sprinkles) | Design tokens (§11): same discipline, TOML + typed resolution | Near 1:1 — design-system practice transfers whole |
| Jest + Testing Library | Unit / component / integration tests, same selector philosophy | Near 1:1, minus all mocking |
| Storybook | `libra gallery` | 1:1 intent |
| Playwright/e2e | Automation server (§16) + replay tests | Better: typed dispatch, byte-exact |
| Rust services / sidecars | In-process custom effect executors (NATS/DDS, §13.3) | Sidecar layer deleted |
| Node + Postgres backend | **Unchanged.** Libra is client-only; service topology stays | N/A |
| Tauri 2 tablet apps | Coexist during pilot; Linux+touch is a native Libra target (§19.1) | Migration, not big-bang |

Type sharing across the boundary: if services are (or become) Rust, serde structs are shared directly between service and UI crates — deleting the codegen'd-TS-types pipeline. With a Node backend retained, schema-first types (JSON Schema/OpenAPI → serde derive) fill the same role.

### D.2 Division of labor (multidisciplinary team)

- **Design & markup-oriented frontend** → `.lx` views, tokens, themes, gallery review. Sub-second hot reload, zero Rust required, and the closed grammar makes it *impossible* to introduce logic or effect bugs from markup. This is the property that keeps most of a UI team productive on day one.
- **Rust-capable engineers** → state, actions, Rust components, Canvas visualizations, custom effect executors, the data plane.
- **Test engineering** → owns the five test kinds, tape fixtures, the trace matrix, automation-server harnesses; UI testing stops being the flaky tier.
- **UX research** → gallery for stimuli; `.lreplay` capture from usability sessions gives pixel-exact session records for analysis.

### D.3 Operational properties for crew-facing software

- **Flight recorder**: continuous action-log ring (§18.5) means any anomaly exports as a replay that reproduces exactly during investigation — an operational capability, not a dev convenience.
- **Bounded resources**: fixed-capacity ring buffers, interned resources, no GC — memory behavior is analyzable, a certification-friendly property.
- **Evidence generation**: `libra test --trace-matrix` (§15.8) produces the requirement→test→artifact bundle from the ordinary CI run.
- **Degraded operation**: actions-as-events + serializable State align naturally with event-sourced sync architectures (offline queues, reconnect replay); sync itself remains application/service concern, deliberately outside the framework.

### D.4 Adoption path (containing the theoretical)

1. **Phase-0 gate** (framework): determinism proven end-to-end before any adoption conversation.
2. **Pilot**: one internal, non-crew ops tool (an ops console or monitoring surface) built by 2–3 people; validates DX claims, §22 budgets on real telemetry shapes, and the D.2 division of labor.
3. **Coexistence**: Libra surfaces ship alongside existing Tauri surfaces; no big-bang rewrite is ever on the table.
4. **Crew surfaces last**: touch/OSK (§19.1) hardened and field-validated on the pilot class of hardware before anything crew-facing.

---

*End of document. Next step: Phase 0 scaffold — `libra-core`, `libra-markup`, the counter, and the first byte-exact snapshot test passing in a container.*
