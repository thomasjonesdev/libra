---
title: "D.3 Operational properties for crew-facing software"
section: "D.3"
aliases:
  - "D.3"
  - "D.3 Operational properties for crew-facing software"
tags:
  - architecture
  - appendix
type: architecture
---

# D.3 Operational properties for crew-facing software

## In this vault

- Parent: [[appendix-d-team-and-use-case-fit-ground-crew-operations-reference-case]]

---

- **Flight recorder**: continuous action-log ring ([[18.5-time-travel-and-session-capture|§18.5]]) means any anomaly exports as a replay that reproduces exactly during investigation — an operational capability, not a dev convenience.
- **Bounded resources**: fixed-capacity ring buffers, interned resources, no GC — memory behavior is analyzable, a certification-friendly property.
- **Evidence generation**: `libra test --trace-matrix` ([[15.8-ci-shape|§15.8]]) produces the requirement→test→artifact bundle from the ordinary CI run.
- **Degraded operation**: actions-as-events + serializable State align naturally with event-sourced sync architectures (offline queues, reconnect replay); sync itself remains application/service concern, deliberately outside the framework.

---

← [[Architecture Home]] · [[Map of Content]] · parent [[appendix-d-team-and-use-case-fit-ground-crew-operations-reference-case]]
