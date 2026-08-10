# IDR V1.5 Embeddable Runtime

Status: Design baseline

## Purpose

V1.5 changes the product boundary from a standalone execution runtime into an embeddable intent and decision runtime.

IDR does not own the host LLM, agent loop, tools, or domain execution.

## Core flow

Host Agent
→ Host LLM (optional)
→ Intent Candidate
→ IDR Core
→ Decision Contract
→ Host Agent execution
→ Outcome feedback

## Core responsibilities

- Intent resolution
- Context resolution
- Human Model assertion management
- Evidence assessment
- Decision resolution
- Decision Contract generation

## Host-owned capabilities

The host provides:

- Model provider
- Context provider
- Human model storage adapter
- Tool execution
- Domain state

## Optional extensions

Governance:

- Policy
- Risk evaluation
- Authorization

Trusted Execution:

- Reservation
- Permit
- Receipt
- Audit

These are not mandatory dependencies of IDR Core.

## Compatibility

All public boundaries are language-neutral contracts. Rust, TypeScript, Python, Java, .NET and other hosts communicate through the same contract model.
