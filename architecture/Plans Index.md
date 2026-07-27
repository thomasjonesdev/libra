---
title: "Plans Index"
aliases:
  - "Plans"
tags:
  - plans
  - architecture
type: index
---

# Plans

Execution / staging plans that implement architecture decisions. Architecture notes stay normative; plans are the how/when.

| Plan | Status | Ties to |
|---|---|---|
| [[MIGRATION-NAMING]] | complete | [[03.1-vocabulary|§3.1]], [[03.2-terminology-migration|§3.2]], [[21-crate-layout|§21]] |
| [[PLAN-ERGONOMICS]] | complete | [[05-state-and-actions|§5]], [[06.2-closed-grammar|§6.2]], [[12.5-declarative-subscriptions|§12.5]] |
| [[PLAN-DASHBOARD-PERF]] | complete | [[09-rendering|§9]], [[14-data-plane|§14]], [[22-performance-budgets|§22]] |
| [[PLAN-ITEM-FIELDS]] | complete | [[06.2-closed-grammar|§6.2]], [[06.3-grammar-restrictions|§6.3]], [[06.4-one-grammar-two-engines|§6.4]] |
| [[PLAN-IF]] | complete | [[06.2-closed-grammar|§6.2]], [[12.5-declarative-subscriptions|§12.5]], [[26-open-questions|§26]] |
| [[PLAN-SOURCES]] | complete | [[12.5-declarative-subscriptions|§12.5]], [[12.1-effect-vocabulary|§12.1]] |
| [[PLAN-ACCESSORS]] | complete | [[06.2-closed-grammar|§6.2]], [[26-open-questions|§26]] |
| [[PLAN-NOTES-APP]] | complete (M0–M9) | [[10.1-text-entry|§10.1]], [[07.1-builtin-catalog|§7.1]], [[PLAN-UI-SURFACE]] |
| [[PLAN-TEXTAREA]] | in progress (stage 1 done) | [[10.1-text-entry|§10.1]] |
| [[PLAN-UI-SURFACE]] | planned | [[07.1-builtin-catalog|§7.1]], [[11-theming|§11]], [[19-platform-integration|§19]] |
| [[PLAN-TOKENS]] | in progress (T1–T4 done) | [[11-theming|§11]], [[PLAN-UI-SURFACE]] |
| [[PLAN-GEIST]] | complete (G1–G3) | [[11-theming|§11]], [[09-rendering|§9]], [[PLAN-TOKENS]] |
| [[PLAN-CATALOG]] | planned | [[07-components|§7]], [[07.1-builtin-catalog|§7.1]], [[PLAN-UI-SURFACE]] |
| [[PLAN-COMPONENTS]] | in progress (C0–C3 done) | [[07.1-builtin-catalog|§7.1]], [[11-theming|§11]], [[PLAN-UI-SURFACE]], [[PLAN-TOKENS]] |
| [[PLAN-GALLERY]] | in progress (G0–G3 + light matrix) | [[07.1-builtin-catalog|§7.1]], [[18-developer-experience|§18]], [[PLAN-CATALOG]], [[PLAN-COMPONENTS]] |
| [[PLAN-SCROLL]] | shipped (S1–S4) | [[07.1-builtin-catalog|§7.1]], [[09.1-display-list|§9.1]], [[09.2-canonical-executor|§9.2]], [[PLAN-GALLERY]] |
| [[PLAN-AWS-LOGIN]] | v1 shipped (P1, P2, A1–A7); A8–A10 planned | [[27.1-aws-sso-session-manager\|§27.1]], [[12.6-the-process-effect\|§12.6]], [[PLAN-NOTIFY]], [[PLAN-RESIDENCY]], [[PLAN-STREAM-DELIVERY]] |
| [[PLAN-STREAM-DELIVERY]] | planned (D1–D6) | [[12.5-declarative-subscriptions\|§12.5]], [[12.6-the-process-effect\|§12.6]], [[12.3-execution-model\|§12.3]], [[PLAN-SOURCES]] |
| [[PLAN-NOTIFY]] | planned (NT1–NT4) | [[12.1-effect-vocabulary\|§12.1]], [[19-platform-integration\|§19]], [[PLAN-NATIVE-INTEGRATIONS]] (N3), [[PLAN-PACKAGE]] |
| [[PLAN-RESIDENCY]] | planned (R1–R6) | [[19-platform-integration\|§19]], [[19.2-keyboard-shortcuts-declarations-not-handlers\|§19.2]], [[20-packaging\|§20]], [[PLAN-PACKAGE]] |
| [[PLAN-NATIVE-INTEGRATIONS]] | planned | [[12.1-effect-vocabulary|§12.1]], [[19-platform-integration|§19]], [[PLAN-UI-SURFACE]] |
| [[PLAN-PACKAGE]] | planned | [[23-roadmap|§23]], [[20-packaging\|§20]] |
| [[PLAN-PHASE4-DX]] | planned | [[23-roadmap|§23]], [[16-automation-server|§16]] |
| [[PLAN-SLICE]] | planned | [[05.7-state-slices|§5.7]] |
| [[PLAN-RECONCILER]] | planned | [[06.2-closed-grammar|§6.2]] |
| [[PLAN-COMPOSE]] | done | [[07.2-composed-components|§7.2]], [[06.4-one-grammar-two-engines|§6.4]], [[06.5-compile-time-validation|§6.5]], [[26-open-questions|§26]] |
| [[PLAN-TEST-CLI]] | in progress (v1) | [[18.1-cli|§18.1]], [[15-testing|§15]], [[15.8-ci-shape|§15.8]] |

← [[Architecture Home]] · [[Map of Content]]
