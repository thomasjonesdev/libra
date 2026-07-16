---
title: "2. Prior Art & Influences"
section: "2"
aliases:
  - "§2"
  - "2"
  - "2. Prior Art & Influences"
tags:
  - architecture
  - section/2
type: architecture
---

# 2. Prior Art & Influences

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

---

← [[Architecture Home]] · [[Map of Content]]
