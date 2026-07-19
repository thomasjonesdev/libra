---
title: "D.4 Adoption Path"
section: "D.4"
aliases:
  - "D.4"
  - "D.4 Adoption Path"
tags:
  - architecture
  - appendix
type: architecture
---

# D.4 Adoption Path

## In this vault

- Parent: [[appendix-d-use-case-fit]]

---

1. **Phase-0 gate** (framework): determinism proven end-to-end before any adoption conversation.
2. **Pilot**: one internal, non-crew ops tool (an ops console or monitoring surface) built by 2–3 people; validates DX claims, [[22-performance-budgets|§22]] budgets on real telemetry shapes, and the [[appendix-d.2-division-of-labor|D.2]] division of labor.
3. **Coexistence**: Libra surfaces ship alongside existing Tauri surfaces; no big-bang rewrite is ever on the table.
4. **Crew surfaces last**: touch/OSK ([[19.1-touch|§19.1]]) hardened and field-validated on the pilot class of hardware before anything crew-facing.

---

*End of document. Next step: Phase 0 scaffold — `libra-core`, `libra-markup`, the counter, and the first byte-exact snapshot test passing in a container.*

---

← [[Architecture Home]] · [[Map of Content]] · parent [[appendix-d-use-case-fit]]
