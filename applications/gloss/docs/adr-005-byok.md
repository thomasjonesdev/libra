---
title: ADR-005 — BYOK inference, no hosted subscription
type: adr
status: accepted
created: 2026-08-24
tags: [gloss, adr]
---

# ADR-005 — BYOK inference, no hosted subscription

## Context
A hosted subscription requires billing, auth, key custody, abuse controls,
uptime, margin math, and a legal entity — all before a single user who isn't me.
Hosted only becomes *necessary* when shipping a custom-trained model, which is
deferred entirely.

## Decision
Bring your own key. One `Provider` trait with Anthropic, OpenAI, and a local
OpenAI-compatible implementation from day one. Keys in the macOS Keychain.

## Consequences
- Roughly zero infrastructure for v1.
- The local provider makes dogfooding on a sensitive vault legal — see
  [[../06-operations#Sensitive-vault mode]].
- Cost visibility becomes a required feature rather than an internal metric,
  which forces the budget-gate problem to be solved in M1 instead of at scale.
- Rate limits become the user's tier; background work needs a queue that
  degrades quietly.
- Embeddings stay local, so indexing costs the user nothing.

## What would reverse it
Shipping a custom Tab model — you can't BYOK a model you trained. That is a
volume-justified decision far past v1.
