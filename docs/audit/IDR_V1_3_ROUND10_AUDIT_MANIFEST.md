# IDR V1.3 Round 10 independent re-audit manifest

Audit candidate date: 2026-07-30

Start with:

1. `docs/audit/reviews/2026-07-30-round9-independent/IDR-V1.3-round9-independent-reaudit-report-2026-07-30.md`
2. `docs/audit/reviews/2026-07-30-round9-independent/EVIDENCE-NOTE.md`
3. `docs/trust-chain-closure/ROUND10-REAUDIT-RESPONSE.md`
4. `docs/trust-chain-closure/ARCHITECTURE.md`
5. `docs/trust-chain-closure/PROOF-BINDINGS.md`
6. `docs/trust-chain-closure/STATE-MACHINES.md`
7. `docs/trust-chain-closure/THREAT-MODEL.md`
8. `docs/trust-chain-closure/VALIDATION-SUMMARY.md`
9. `docs/trust-chain-closure/idr-v13-round10-validation-output.txt`
10. `crates/idr-runtime/src/trust_chain.rs`
11. `crates/idr-runtime/src/postgres_authority.rs`
12. `crates/idr-store/migrations/0004_round9_domain_validity_and_immutability.sql`
13. `crates/idr-store/migrations/0005_round10_execution_integrity.sql`
14. `crates/idr-store/tests/postgres_trust_chain.rs`
15. `crates/idr-store/tests/postgres_vertical_slice.rs`
16. `tools/check_audit_package_hygiene.sh`
17. `tools/scan_package_secrets.mjs`
18. `tools/build_round10_audit_package.sh`

Recommended independent attack order:

- verify the reported `coding-plan` key fingerprint is revoked at the provider
  without receiving or copying the raw key into the review environment;
- submit Intent, Decision, Action, Turn-derived Action and Response send after
  the respective upstream record has expired;
- admit an Action with five proofs, rotate/revoke each proof issuer key in turn,
  and attempt reserve, recovery, delivery and dispatch;
- update reservation operation/idempotency, Action/Admission/provider/owner,
  validity or digest identity; update attempt permit/nonce/lease/provider; then
  attempt deletion and recreation of the exactly-once slot;
- disable execution guards as a privileged role, alter an execution index, and
  verify startup rejects the projection and execution-state checkpoint root;
- mutate a checkpoint root or signed body while leaving publication time valid;
- apply migration 0004 to a non-empty Round 8 fixture and verify it aborts
  before any schema alteration; separately replay verified data into a fresh
  schema and apply 0005;
- repeat Round 9 cross-domain, authority-forgery, rollback, Receipt, Outcome,
  Human Model and Aegis Production-gate attacks;
- inspect all archive names, symlinks, duplicate paths and secret-scan output
  before extracting.

The package uses explicit source allowlists for IDR and Aegis Life. It excludes
Git metadata, build/dependency output, runtime state, `.aegis`, real
environment files and credential/key containers. The secret scanner never
prints a discovered raw value.

Release status remains:

```text
IDR_TRUST_CHAIN_CLOSURE_COMPLETE = NO
PRODUCTION TRUST ROOT = BLOCKED
PRODUCTION AUTHORIZATION = NOT AUTHORIZED
PRODUCTION EXECUTION = NOT AUTHORIZED
HUMAN MODEL LONG-TERM WRITE = NOT AUTHORIZED
AEGIS PRODUCTION ADAPTER = NOT AUTHORIZED
```
