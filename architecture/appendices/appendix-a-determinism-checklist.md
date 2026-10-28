---
title: "Appendix A — Determinism Contract Checklist"
section: "Appendix A"
aliases:
  - "Appendix A"
  - "Appendix A — Determinism Contract Checklist"
tags:
  - architecture
  - appendix
type: architecture
---

# Appendix A — Determinism Contract Checklist

A change is contract-safe iff: no I/O or clocks enter actions; no computation enters markup; no effect executes outside the runtime; no draw output depends on machine, driver, thread timing, or wall clock; **the same display list produces the same PNG on macOS, Windows, and Linux** ([[19.4-desktop-os-matrix|§19.4]]); OS chrome stays off this path and is explicit per OS (native API or typed `Unsupported`, never a silent no-op); every new component emits semantics; every new effect is serializable and interceptable; every new render feature lands with a snapshot fixture.

---

← [[Architecture Home]] · [[Map of Content]]
