# IDR V1.3 Round 12 independent re-audit manifest

Audit candidate date: 2026-07-30

Start with:

1. `docs/audit/reviews/2026-07-30-round11-independent/IDR-V1.3-round11-independent-reaudit-report-2026-07-30.md`
2. `docs/audit/reviews/2026-07-30-round11-independent/idr-v13-round11-independent-test-output.txt`
3. `docs/trust-chain-closure/ROUND12-REAUDIT-RESPONSE.md`
4. `docs/trust-chain-closure/POSTGRES-ROLE-SEPARATION.md`
5. `docs/trust-chain-closure/ARCHITECTURE.md`
6. `docs/trust-chain-closure/MIGRATION-GUIDE.md`
7. `docs/trust-chain-closure/VALIDATION-SUMMARY.md`
8. `docs/trust-chain-closure/idr-v13-round12-validation-output.txt`
9. `crates/idr-runtime/src/trust_chain.rs`
10. `crates/idr-runtime/src/postgres_authority.rs`
11. `crates/idr-store/migrations/0007_round12_attested_authority_boundary.sql`
12. `crates/idr-store/tests/postgres_trust_chain.rs`
13. `crates/idr-store/tests/postgres_vertical_slice.rs`
14. `tools/idr-production-migrator/src/main.rs`
15. `tools/run_round12_validation.sh`
16. `tools/build_round12_audit_package.sh`
17. `tools/check_audit_package_hygiene.sh`
18. `tools/scan_package_secrets.mjs`

Recommended independent attack order:

- independently confirm provider credential revocation, rotation, last use and
  billing review without receiving old or replacement secret values;
- migrate a fresh `idr_production_*` schema and verify version 7;
- verify Runtime has zero DML on every real table, zero sequence privilege,
  no HMAC key read and EXECUTE only on the attestation opener;
- using only the Runtime login, attempt raw Run, Contract, command Receipt,
  Fence and attestation inserts/updates and confirm fail-closed behavior;
- attempt to replay a captured MAC from another transaction/backend and alter
  command, Run, tenant, versions or either digest;
- commit an authentic command and verify Receipt, Run event, audit event,
  checkpoint, immutable transition attestation and exact Receipt-content MAC
  bindings on replay and restart;
- revoke/expire one bound Admission proof before Reserve and after Dispatch;
  verify a persistent cancellation/invalidation before Dispatch and
  `ReconciliationRequired` after Dispatch;
- verify the migrator DB URL is absent from argv and the key-file mode and
  canonical encoding checks fail closed;
- repeat the permanent Fence, CAS, privileged base-table tamper, external
  rollback, historical evidence, Response, Outcome, Human Model, package and
  Aegis Production-gate attacks.

Release status remains:

```text
IDR_TRUST_CHAIN_CLOSURE_COMPLETE = NO
PRODUCTION TRUST ROOT = BLOCKED
PRODUCTION AUTHORIZATION = NOT AUTHORIZED
PRODUCTION EXECUTION = NOT AUTHORIZED
HUMAN MODEL LONG-TERM WRITE = NOT AUTHORIZED
AEGIS PRODUCTION ADAPTER = NOT AUTHORIZED
```
