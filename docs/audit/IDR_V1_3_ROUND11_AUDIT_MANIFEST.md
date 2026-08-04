# IDR V1.3 Round 11 independent re-audit manifest

Audit candidate date: 2026-07-30

Start with:

1. `docs/audit/reviews/2026-07-30-round10-independent/IDR-V1.3-round10-independent-reaudit-report-2026-07-30.md`
2. `docs/audit/reviews/2026-07-30-round10-independent/idr-v13-round10-independent-test-output.txt`
3. `docs/trust-chain-closure/ROUND11-REAUDIT-RESPONSE.md`
4. `docs/trust-chain-closure/POSTGRES-ROLE-SEPARATION.md`
5. `docs/trust-chain-closure/ARCHITECTURE.md`
6. `docs/trust-chain-closure/MIGRATION-GUIDE.md`
7. `docs/trust-chain-closure/VALIDATION-SUMMARY.md`
8. `docs/trust-chain-closure/idr-v13-round11-validation-output.txt`
9. `crates/idr-runtime/src/trust_chain.rs`
10. `crates/idr-runtime/src/postgres_authority.rs`
11. `crates/idr-store/migrations/0006_round11_runtime_roles_and_idempotency_fence.sql`
12. `crates/idr-store/tests/postgres_trust_chain.rs`
13. `crates/idr-store/tests/postgres_vertical_slice.rs`
14. `tools/idr-production-migrator/Cargo.toml`
15. `tools/idr-production-migrator/src/main.rs`
16. `tools/run_round11_validation.sh`
17. `tools/build_round11_audit_package.sh`
18. `tools/check_audit_package_hygiene.sh`
19. `tools/scan_package_secrets.mjs`

Recommended independent attack order:

- verify provider-side revocation/rotation and incident review through redacted
  control-plane evidence; do not receive or copy either old or replacement
  credential values;
- prove the Production Runtime binary cannot contain migration authority and
  Production startup does not apply a missing migration;
- connect using the actual deployment Runtime login and verify PostgreSQL
  error `42501` for `ALTER TABLE`, `DISABLE TRIGGER`, execution identity-column
  UPDATE, Reservation/Fence DELETE and object-owner role assumption;
- connect using the auditor login and verify read access plus write denial;
- as a privileged test role, disable Reservation guards and rewrite the
  Reservation operation/idempotency fields; before restart or integrity scan,
  attempt to create the original tuple and confirm the Fence unique constraint
  still rejects it;
- mutate, delete and recreate Fence rows and verify ACL/trigger/unique-root
  protections independently;
- replay an exact successful command after CallerAuthentication issuer
  revocation or proof expiry and verify no Receipt is returned;
- replay the same command ID with different content and verify
  `IdempotencyConflict` rather than proof-dependent behavior;
- test Receipt→Outcome at both sides of the 86,400-second boundary and
  Outcome→Human Model at both sides of the 604,800-second boundary, including
  invalidation/replacement;
- invalidate or expire an Action during Permit delivery and dispatch and
  verify the cancellation/reconciliation event and projection commit;
- repeat all Round 10 proof, execution root, rollback, response, Outcome,
  Human Model, package-hygiene and Aegis Production-gate attacks.

Release status remains:

```text
IDR_TRUST_CHAIN_CLOSURE_COMPLETE = NO
PRODUCTION TRUST ROOT = BLOCKED
PRODUCTION AUTHORIZATION = NOT AUTHORIZED
PRODUCTION EXECUTION = NOT AUTHORIZED
HUMAN MODEL LONG-TERM WRITE = NOT AUTHORIZED
AEGIS PRODUCTION ADAPTER = NOT AUTHORIZED
```
