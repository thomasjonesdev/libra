---
title: ADR-006 — Build for myself first
type: adr
status: accepted
created: 2026-08-24
tags: [gloss, adr]
---

# ADR-006 — Build for myself first

## Context
PKM monetizes badly and the Obsidian audience self-selected for free and local.
Picking a paying segment now would be a guess dressed as strategy.

## Decision
Design for one user — me — and defer the paying-customer question to an explicit
gate after M2 and R1.

## Consequences
- Dogfooding sets an honest quality bar; there is no way to fake using it daily.
- The use case I actually have (vault as guide to a codebase) happens to be the
  one with a buyer attached later, which is fortunate rather than planned.
- Scope stays small; features that only make sense for a hypothetical user get
  cut automatically.
- Risk: building something only I want. The gate exists to catch that.

## What would reverse it
The reassessment gate — see [[../04-roadmap#Reassessment gate]]. Both outcomes
(product, or very good personal tool) are acceptable. Conflating them is not.
