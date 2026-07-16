---
title: "Appendix B — Glossary"
section: "Appendix B"
aliases:
  - "Appendix B"
  - "Appendix B — Glossary"
tags:
  - architecture
  - appendix
type: architecture
---

# Appendix B — Glossary

**State** the single app-state struct (né *Model*) · **Action** one recorded state change — authored as a method on `State`, recorded as data (né *Msg*) · **Reducer** the generated `reduce(state, action) -> Vec<Effect>` (né *update*) · **Effect** effect description (né *Cmd*) · **Sub** ongoing event source · **Slice** nested state struct with its own actions ([[05.7-scaling-state-slices-not-fragmentation|§5.7]]) · **Component** anything that renders (né *widget*) · **ViewTree** typed output of a view engine · **Component tree** retained UI state · **DrawCmd** display-list op · **Executor** display-list consumer · **Tape** recorded sample log · **Replay** recorded session · **Bless** accept new snapshot truth · **Harness** in-process test runtime · **Semantics** role/label/state node (a11y + queries) · **Source element** invisible markup element declaring a mount-scoped subscription ([[12.5-declarative-subscriptions-effects-as-mount-scoped-data|§12.5]]) · **Unit / component / integration / snapshot / replay tests** the five-kind ladder (né *Tiers 1–5*).

---

← [[Architecture Home]] · [[Map of Content]]
