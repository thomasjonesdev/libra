---
title: "14. The High-Rate Data Plane"
section: "14"
aliases:
  - "§14"
  - "14"
  - "14. The High-Rate Data Plane"
tags:
  - architecture
  - section/14
type: architecture
---

# 14. The High-Rate Data Plane

## In this vault

- [[14.1-problem]]
- [[14.2-architecture]]
- [[14.3-downsampling-m4]]
- [[14.4-determinism]]
- [[14.5-log-volume-text]]

## In code

- `crates/libra-data/src/lib.rs` — rings, `.ltape` tapes, generators, M4, the §14.3 column cache

---

The subsystem that makes Libra fit for telemetry-class workloads — and the sharpest deliberate deviation from "everything through the loop."

---

← [[Architecture Home]] · [[Map of Content]]
