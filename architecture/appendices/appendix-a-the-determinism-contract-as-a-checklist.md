---
title: "Appendix A — The determinism contract, as a checklist"
section: "Appendix A"
aliases:
  - "Appendix A"
  - "Appendix A — The determinism contract, as a checklist"
tags:
  - architecture
  - appendix
type: architecture
---

# Appendix A — The determinism contract, as a checklist

A change is contract-safe iff: no I/O or clocks enter actions; no computation enters markup; no effect executes outside the runtime; no draw output depends on machine, driver, thread timing, or wall clock; every new component emits semantics; every new effect is serializable and interceptable; every new render feature lands with a snapshot fixture.

---

← [[Architecture Home]] · [[Map of Content]]
