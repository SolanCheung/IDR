# IDR V1.3 Round 11 response to the Round 10 independent audit

Date: 2026-07-30

Release decision remains:

```text
IDR_TRUST_CHAIN_CLOSURE_COMPLETE = NO
PRODUCTION TRUST ROOT = BLOCKED
PRODUCTION AUTHORIZATION = NOT AUTHORIZED
PRODUCTION EXECUTION = NOT AUTHORIZED
HUMAN MODEL LONG-TERM WRITE = NOT AUTHORIZED
AEGIS PRODUCTION ADAPTER = NOT AUTHORIZED
```

This snapshot is a remediation candidate for independent review. Local tests
do not close an audit finding or authorize Production.

## Round 10 finding response

| Finding | Round 11 remediation candidate | Local evidence |
| --- | --- | --- |
| P0-00 provider-side leaked-key response | No code claim. Known local copies remain absent and packaging remains allowlisted/scanned. Provider revocation, rotation, last-use and billing review are still required. | source/package secret scans; known leaked paths absent |
| P0-01 privileged Runtime can release exactly-once slot | Production migration and Runtime features are compile-time exclusive. Production no longer auto-migrates; a separate migrator applies DDL. Migration 0006 creates schema-specific restricted Runtime and read-only auditor roles. Production startup rejects superuser/DDL/owner/trigger-capable identities. | separate migrator dynamic run; feature compile failures; PostgreSQL `42501` role attacks |
| P0-01 permanent operation/idempotency identity | New append-only `idr_operation_idempotency_fences` owns the unique trust-domain/environment/tenant/operation/idempotency tuple. Reservation rows reference the Fence and cannot replace its identity. Fence contents are included in startup integrity checks and the externally published execution-state root. | Fence UPDATE/DELETE rejection; privileged Reservation trigger bypass followed by pre-restart duplicate Fence INSERT rejection |
| P1-01 replay authentication bypass | Every command requires a current CallerAuthentication proof plus its command-specific proof set. Exact Receipt replay first rejects command-ID content conflicts, then revalidates the full original proof set using current database time and current Trust Root before returning the Receipt. | successful live replay; replay rejected after issuer revocation |
| P1-02 Receipt→Outcome historical time semantics | A dedicated historical-evidence gate requires exact current identity, matching trust domain, non-invalidation, observation after `valid_from`, and observation before `valid_until + 86,400` seconds. | boundary and invalidation tests |
| P1-03 Outcome→Human Model time semantics | The same explicit gate applies a 604,800-second maximum cognitive-use window to the exact Outcome record. | boundary and invalidation tests |
| P1-04 invalid Action state rollback | Deliver/dispatch discovery of invalid or expired authority now returns a successful governance transition that persists `Cancelled` before dispatch or `ReconciliationRequired` after dispatch. | both transition paths tested |
| P1-08 migration and Runtime identity conflation | Separate migration feature/binary, no Production `MIGRATOR.run`, schema-version-only startup, restricted Runtime role and read-only auditor role. | compile, migration and ACL matrix |

## Database replay boundary

Migration 0006 intentionally refuses a schema containing any execution
reservation or attempt. It does not claim a safe in-place conversion of
existing execution identities. Pre-production history must be verified and
replayed into a fresh `idr_production_*` schema so every Reservation is created
with its permanent Fence.

The Runtime role has table INSERT/SELECT needed by the authority, but only
column-level UPDATE on:

- Reservation `state`, `aggregate_version`, `updated_at`;
- Attempt `state`, `started_at`, `completed_at`.

It has no execution identity-column UPDATE, Fence UPDATE/DELETE, DDL, TRIGGER
or ownership authority. Triggers remain defense in depth.

## Explicitly unresolved blockers

P0-00 remains open until the provider supplies redacted evidence of revocation,
rotation, last-use review, billing/usage review and replacement-key storage
boundaries.

The following Round 10 findings are not falsely claimed closed by this code
snapshot:

- transactionally serialized Trust Root control-plane epoch;
- commit-to-anchor publication gap and protected WORM/KMS signer/backend;
- historical Trust Root retention and full historical Proof signature replay;
- provider-facing signed Execution Permit and provider verification receipt;
- real response delivery/fan-out receipts and reconciliation;
- richer Outcome conflict/causality/partial-result semantics;
- complete Human Model sensitivity, retention, decay, inspection,
  correction/delete and legal-hold governance;
- verified production corpus migration/restore evidence;
- Git provenance and two consecutive clean independent reviews.

The release latches and Aegis Production compile gate must remain blocked.
