# IDR V1.3 Round 12 response to the Round 11 independent audit

Audit input:
`docs/audit/reviews/2026-07-30-round11-independent/IDR-V1.3-round11-independent-reaudit-report-2026-07-30.md`

Release status remains:

```text
IDR_TRUST_CHAIN_CLOSURE_COMPLETE = NO
PRODUCTION TRUST ROOT = BLOCKED
PRODUCTION AUTHORIZATION = NOT AUTHORIZED
PRODUCTION EXECUTION = NOT AUTHORIZED
HUMAN MODEL LONG-TERM WRITE = NOT AUTHORIZED
AEGIS PRODUCTION ADAPTER = NOT AUTHORIZED
```

Local remediation is not provider or deployment evidence. In particular,
Round 12 does not claim that the previously exposed `coding-plan` credential
has been revoked, rotated or billing-reviewed.

## Remediation matrix

| Round 11 finding | Round 12 remediation candidate | Local evidence |
| --- | --- | --- |
| P0-01 Runtime login can write authority tables directly | Migration 0007 moves all 22 authority tables behind view names, assigns base tables to a protected owner and gives Runtime zero base-table DML/sequence privilege. | dynamic Production migration ACL check; raw Runtime Run/Contract/Receipt/Fence/attestation attacks |
| P1-01 Receipt has no unforgeable attestation | Every command transaction opens an HMAC attestation bound to transaction, command, Run, tenant, versions and digests. `IdrCommandReceiptV1` contains its attestation ID and its exact canonical content has a separate `receipt_mac`; startup and replay recompute both MACs and cross-bind Run/audit/checkpoint evidence. | Receipt replay, startup verification, forged-Receipt and privileged Receipt-content tamper attacks |
| P1-02 proof revocation rolls back without governed state | A bound Admission proof failure creates an authoritative governance transition. Before Dispatch it invalidates the Admission and records a cancellation event while returning the Run to `WaitingAuthorization`; after Dispatch it persists `ReconciliationRequired`. | revoked Policy-after-Admission PostgreSQL test |
| P1-12 startup role checks are incomplete | Production startup now checks the exact SHA-256-derived role, every real-table/sequence/view/function/key-table ACL and owner-equivalent authority. | migration probe and production-only compile/Clippy |
| P1-13 migrator leaks DB URL in argv | Migrator accepts only the schema argument. DB URL and HMAC key path are environment inputs; key file must be mode 0600. | CLI implementation and dynamic migration |
| P2-02/P2-03 weak/shared role derivation | Owner, Runtime and auditor names use 128 bits of SHA-256 and Shadow roles are schema-specific. | migration source and SET LOCAL ROLE attacks |

## Authority attestation protocol

For each semantic command the Rust Orchestrator:

1. starts a SERIALIZABLE PostgreSQL transaction and reads DB time;
2. verifies the current Trust Root, caller and exact proof set;
3. computes the semantic transition and canonical transition digest;
4. reads `txid_current()` and `pg_backend_pid()`;
5. HMAC-signs the complete binding with its protected 32-byte key;
6. calls the sole Runtime-executable opener;
7. writes through the authority views; and
8. atomically commits attestation, projection, Contract state, proof
   consumption, events, audit, outbox, Receipt and checkpoint.

The view bridge rejects absent, malformed, wrong-transaction or wrong-identity
attestations. The attestation and key tables are immutable through triggers;
Runtime cannot write either table or read key bytes.

## Items deliberately still open

- provider-side credential revocation/rotation/last-use/billing evidence;
- protected production Trust Root distribution and transactional epoch;
- WORM/KMS-backed checkpoint signer and atomic publish protocol;
- provider-facing signed Permit and production outbox/delivery implementation;
- full historical Trust Root proof replay;
- Authority/Capability/Policy registries and issuer governance;
- production Outcome and Human Model governance;
- corpus scale, Git provenance and two consecutive independent clean reviews.

These keep every Production release gate blocked.
