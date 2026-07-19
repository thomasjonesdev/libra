---
title: "D.2 Division of Labor"
section: "D.2"
aliases:
  - "D.2"
  - "D.2 Division of Labor"
tags:
  - architecture
  - appendix
type: architecture
---

# D.2 Division of Labor

## In this vault

- Parent: [[appendix-d-use-case-fit]]

---

- **Design & markup-oriented frontend** → `.lx` views, tokens, themes, gallery review. Sub-second hot reload, zero Rust required, and the closed grammar makes it *impossible* to introduce logic or effect bugs from markup. This is the property that keeps most of a UI team productive on day one.
- **Rust-capable engineers** → state, actions, Rust components, Canvas visualizations, custom effect executors, the data plane.
- **Test engineering** → owns the five test kinds, tape fixtures, the trace matrix, automation-server harnesses; UI testing stops being the flaky tier.
- **UX research** → gallery for stimuli; `.lreplay` capture from usability sessions gives pixel-exact session records for analysis.

---

← [[Architecture Home]] · [[Map of Content]] · parent [[appendix-d-use-case-fit]]
