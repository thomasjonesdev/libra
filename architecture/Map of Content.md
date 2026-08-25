---
title: "Map of Content"
aliases:
  - "MOC"
  - "Architecture MOC"
tags:
  - architecture
  - moc
type: moc
---

# Map of Content

Graph-friendly outline of the architecture vault. Prefer this over scrolling a single document.

**New here?** Read [[AGENTS]] first (end-to-end brief), then use this outline to dive.

## Pipeline (mental model)

```
.lx → markup → interp/codegen → components/layout → draw → raster
         ↑ state/actions          ↑ effects runtime
```

See [[04-system-overview|§4 System Overview]] and [[04.1-layer-diagram|§4.1 Layer diagram]].

## Full outline

### [[01-vision|1. Vision & Philosophy]]

- [[01.1-pitch|1.1 The one-sentence pitch]]
- [[01.2-problem|1.2 The problem]]
- [[01.3-the-bet|1.3 The bet]]
- [[01.4-design-principles|1.4 Design principles]]
- [[01.5-audience|1.5 Who this is for]]

### [[02-prior-art|2. Prior Art & Influences]]

### [[03-mental-model|3. Mental Model: React → Libra]]

- [[03.1-vocabulary|3.1 The vocabulary (five words, all of them already yours)]]
- [[03.2-terminology-migration|3.2 Terminology migration (complete — this document and the code now agree)]]
- [[03.3-naming-conventions|3.3 Naming conventions: React's, verbatim]]

### [[04-system-overview|4. System Overview]]

- [[04.1-layer-diagram|4.1 Layer diagram]]
- [[04.2-frame-lifecycle|4.2 The frame lifecycle]]

### [[05-state-and-actions|5. State & Actions]]

- [[05.1-core-declarations|5.1 The core declarations]]
- [[05.2-macro-expansion|5.2 What the macro generates (one expansion, no magic)]]
- [[05.3-purity-rules|5.3 Purity rules for actions (enforced by convention, checked by lint)]]
- [[05.4-actions-are-events|5.4 Actions are events, not setters]]
- [[05.5-init|5.5 Init]]
- [[05.6-time-travel-replay|5.6 Time travel & replay as a consequence]]
- [[05.7-state-slices|5.7 Scaling state: slices, not fragmentation]]
- [[05.8-app-assembly|5.8 App assembly: one declaration]]
- [[05.9-module-layout|5.9 App module layout: splitting lib.rs without breaking the macros]]

### [[06-view-and-markup|6. The View Layer & Markup]]

- [[06.1-the-lx-file|6.1 The `.lx` file]]
- [[06.2-closed-grammar|6.2 The closed grammar — what markup CAN do]]
- [[06.3-grammar-restrictions|6.3 What markup CANNOT do — and why each restriction pays]]
- [[06.4-one-grammar-two-engines|6.4 One grammar, two engines]]
- [[06.5-compile-time-validation|6.5 Compile-time validation (the part React cannot give you)]]
- [[06.6-parser|6.6 Parser]]

### [[07-components|7. Components]]

- [[07.1-builtin-catalog|7.1 The built-in catalog (closed, curated)]]
- [[07.2-composed-components|7.2 Composed components (markup)]]
- [[07.3-rust-components|7.3 Rust components]]
- [[07.4-canvas|7.4 Canvas: advanced custom UI]]

### [[08-layout|8. Layout]]

### [[09-rendering|9. Rendering]]

- [[09.1-display-list|9.1 The display list]]
- [[09.2-canonical-executor|9.2 The canonical executor: tiny-skia (CPU)]]
- [[09.3-damage-layers|9.3 Making CPU fast enough: damage + layers]]
- [[09.4-gpu-executor|9.4 The future GPU executor (insurance, not dependency)]]
- [[09.5-video|9.5 Video (planned, not early-phase)]]
- [[09.6-animation|9.6 Animation]]

### [[10-text|10. Text]]

- [[10.1-text-entry|10.1 Text entry — the full spec (in scope, committed)]]

### [[11-theming|11. Theming & Design Tokens]]

### [[12-effects-runtime|12. The Effects Runtime]]

- [[12.1-effect-vocabulary|12.1 The Effect vocabulary (closed, versioned)]]
- [[12.2-subscriptions|12.2 Subscriptions]]
- [[12.3-execution-model|12.3 Execution model]]
- [[12.4-effects-as-data|12.4 Why effects-as-data is the testing keystone]]
- [[12.5-declarative-subscriptions|12.5 Declarative subscriptions — effects as mount-scoped data]]
- [[12.6-the-process-effect|12.6 The process effect]]

### [[13-persistence|13. Persistence]]

- [[13.1-kv-store|13.1 The KV store (localStorage, but honest)]]
- [[13.2-state-snapshots|13.2 State snapshots]]
- [[13.3-beyond-kv|13.3 Beyond KV]]

### [[14-data-plane|14. The High-Rate Data Plane]]

- [[14.1-problem|14.1 The problem it solves]]
- [[14.2-architecture|14.2 Architecture]]
- [[14.3-downsampling-m4|14.3 Downsampling: M4]]
- [[14.4-determinism|14.4 Determinism at the boundary]]
- [[14.5-log-volume-text|14.5 Log-volume text]]

### [[15-testing|15. Testing Architecture]]

- [[15.1-determinism-contract|15.1 The determinism contract (what makes every kind exact)]]
- [[15.2-unit-tests|15.2 Unit tests (actions)]]
- [[15.3-component-tests|15.3 Component tests (headless tree queries)]]
- [[15.4-integration-tests|15.4 Integration tests (full flows, no mocks)]]
- [[15.5-snapshot-tests|15.5 Snapshot tests (byte-exact pixels)]]
- [[15.6-replay-tests|15.6 Replay tests (recorded sessions)]]
- [[15.7-fuzzing-property-tests|15.7 Fuzzing & property tests]]
- [[15.8-ci-shape|15.8 CI shape]]

### [[16-automation-server|16. The Automation Server]]

### [[17-accessibility|17. Accessibility]]

### [[18-developer-experience|18. Developer Experience & Tooling]]

- [[18.1-cli|18.1 The CLI]]
- [[18.2-iteration-loop|18.2 The iteration loop (the DX centerpiece)]]
- [[18.3-errors|18.3 Errors]]
- [[18.4-debugging|18.4 Debugging]]
- [[18.5-time-travel-capture|18.5 Time travel & session capture]]
- [[18.6-editor-tooling|18.6 Editor tooling (staged honestly)]]

### [[19-platform-integration|19. Platform Integration]]

- [[19.1-touch|19.1 Touch (first-class, not an afterthought)]]
- [[19.2-keyboard-shortcuts-declarations-not-handlers|19.2 Keyboard shortcuts: declarations, not handlers]]

### [[20-packaging|20. Packaging & Distribution]]

### [[21-crate-layout|21. Workspace & Crate Layout]]

### [[22-performance-budgets|22. Performance Budgets]]

### [[23-roadmap|23. Roadmap]]

### [[24-non-goals|24. Non-Goals]]

### [[25-risks|25. Risks & Mitigations]]

### [[26-open-questions|26. Open Questions]]

### [[27-first-party-applications|27. First-Party Applications]]

- [[27.1-aws-sso-session-manager|27.1 AWS SSO Session Manager]]

### [[appendix-a-determinism-checklist|Appendix A — The determinism contract, as a checklist]]

### [[appendix-b-glossary|Appendix B — Glossary]]

### [[appendix-c-react-borrowings|Appendix C — Second-round React ecosystem borrowings (adopted)]]

### [[appendix-d-use-case-fit|Appendix D — Team & Use-Case Fit (ground/crew operations reference case)]]

- [[appendix-d.1-stack-translation|D.1 Stack translation]]
- [[appendix-d.2-division-of-labor|D.2 Division of labor (multidisciplinary team)]]
- [[appendix-d.3-operational-properties|D.3 Operational properties for crew-facing software]]
- [[appendix-d.4-adoption-path|D.4 Adoption path (containing the theoretical)]]

---

[[Architecture Home]] · [[Plans Index]]
