---
title: "PLAN-PACKAGE"
aliases:
  - "PLAN-PACKAGE"
tags:
  - plans
  - architecture
type: plan
---

# `libra package` + packaging (§18)

**Status: planned.** After CLI `check`/`dev` foundations.

1. `libra.toml` manifest (windows, identity, permissions)
2. `libra package` — release bundle (codegen views, assets, themes)
3. Document install path in README

**Downstream, and no longer hypothetical:** app *identity* from step 1 is what
unblocks macOS/Windows notifications ([[PLAN-NOTIFY]] NT3) and macOS
launch-at-login ([[PLAN-RESIDENCY]] R5). Both ship degrading gracefully without
it, so this plan does not block them — but the `libra.toml` identity fields
(bundle id, AppUserModelID, signing identity) should be designed with those two
consumers in mind rather than retrofitted.

---

← [[Plans Index]] · [[20-packaging|§20]] · [[PLAN-NOTIFY]] · [[PLAN-RESIDENCY]] · [[18-developer-experience|§18]] if present; else [[23-roadmap|§23]]
