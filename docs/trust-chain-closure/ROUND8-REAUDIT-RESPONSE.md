# IDR V1.3 Round 8 response to the Round 7 independent audit

Date: 2026-07-30

> Historical correction (Round 9): the Round 8 statement that the Shadow
> boundary was closed was too strong. Round 8 separated database schemas and
> runtime constructors, but did not bind trust domain/environment into the
> signed Command, Proof, Trust Root, public record reference or checkpoint.
> The Round 8 independent audit correctly reopened this as P0-01. See
> `ROUND9-REAUDIT-RESPONSE.md`; this file remains as historical evidence.

Release decision remains:

```text
IDR_TRUST_CHAIN_CLOSURE_COMPLETE = NO
PRODUCTION TRUST ROOT = BLOCKED
PRODUCTION AUTHORIZATION = NOT AUTHORIZED
PRODUCTION EXECUTION = NOT AUTHORIZED
HUMAN MODEL LONG-TERM WRITE = NOT AUTHORIZED
AEGIS PRODUCTION ADAPTER = NOT AUTHORIZED
```

This is a remediation candidate for independent review, not a claim that a
clean independent review has occurred.

## Round 7 P0 response

| Finding | Round 8 change | Local evidence |
| --- | --- | --- |
| P0-01 production + test-support bypass | Compile error for the feature pair; database/inspect/outbox helpers require both `shadow-mode` and `test-support` | downstream expected-compile-failure crate |
| P0-02 independent release gates | Production startup checks closure, Trust Root, authorization, execution and Human Model gates; command-family checks remain at `handle`; `human-model-write` now gates every HM mutation | production-only unit test and source checks |
| P0-03 caller-selectable Shadow bypass | Removed `PostgresStartupModeV1` and generic `connect`; separate feature-gated constructors and mandatory `idr_shadow_*`/`idr_production_*` schemas; persisted domain on Run/Record/Proof/Audit/Outbox/Receipt state | migration 0003, PostgreSQL domain-rewrite attack |
| P0-04 incomplete Proof target | Every proof binds `UnsignedCommandEnvelopeV1`: command ID, Run, expected version, principals, tenant/scope/purpose/policy, correlation, causation and the complete command/target refs; only proof envelopes are excluded | CancelRun cross-Run/version/correlation/causation attacks and HM target test |
| P0-05 Human Model escalation | Assertion request may contain only the source digest; lifecycle derives from Promotion outcome, impact derives from the source candidate, and the materialized assertion binds exact Promotion record ID/revision/digest | runtime attack test plus database trigger v3 |

## Additional closures

- P1-09: `ReconcileExecution` can only preserve `Unknown`; terminal
  reconciliation must submit an exact `CommitExecutionReceipt` bound to Permit,
  provider, dispatch nonce, attempt and request digest.
- P1-10: terminal execution Receipt now maps the Run to
  `Succeeded`/`Failed`/`Rejected`; post-terminal Outcome/HM processing accepts
  the evidence-bearing terminal states.
- P2-01: documentation now distinguishes unsigned proof-target digest from the
  full envelope storage/idempotency digest.
- P2-03: `idr-runtime` defaults to no features and `idr-store` defaults only to
  the explicit development file store; PostgreSQL Shadow/Production are
  opt-in.

## Deliberately unresolved deployment work

The Round 7 P1 findings that require deployment infrastructure or broader
product semantics remain open: external WORM/KMS anchoring, protected Trust
Root control service, atomic anchor publication protocol, provider-facing
signed Permit, delivery Receipt/fan-out, independent issuer registries,
multi-observation Outcome conflict handling, sensitive HM retention/decay and
production query API, migration/fuzz/chaos release evidence, Aegis Production
adapter authorization, and two consecutive clean independent reviews.

The source directory also still has no Git provenance. The audit package uses
`SHA256SUMS` to identify this snapshot, but that does not replace signed
repository history.
