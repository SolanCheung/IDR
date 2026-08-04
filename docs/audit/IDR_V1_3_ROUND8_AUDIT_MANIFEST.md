# IDR V1.3 Round 8 independent re-audit manifest

Audit date: 2026-07-30

Start with:

1. `docs/audit/reviews/2026-07-30-round7-independent/IDR-V1.3-round7-independent-reaudit-report-2026-07-30.md`
2. `docs/trust-chain-closure/ROUND8-REAUDIT-RESPONSE.md`
3. `docs/trust-chain-closure/ARCHITECTURE.md`
4. `docs/trust-chain-closure/VALIDATION-SUMMARY.md`
5. `docs/trust-chain-closure/idr-v13-round8-validation-output.txt`
6. `crates/idr-runtime/src/trust_chain.rs`
7. `crates/idr-runtime/src/postgres_authority.rs`
8. `crates/idr-store/migrations/0003_round8_trust_domains_and_human_model.sql`
9. `crates/idr-store/tests/postgres_trust_chain.rs`
10. `crates/idr-store/tests/postgres_vertical_slice.rs`
11. `tools/production-test-support-compile-fail`
12. `tools/production-shadow-compile-fail`

Recommended independent attack order:

- compile `production + test-support` and `production + shadow-mode`;
- inspect exports for a generic connector, raw pool, inspect or outbox helpers;
- attempt to connect Shadow to a non-Shadow schema and rewrite persisted
  `trust_domain`;
- sign CancelRun for Run A, then redirect it to Run B or mutate aggregate
  version/correlation/causation;
- sign an HM promotion for one exact target, then swap target record/revision;
- request provisional Promotion materialization with
  `user_confirmed`/10,000-basis-point Assertion fields;
- issue terminal `ReconcileExecution` without an exact Receipt;
- commit success/failure/rejection Receipts and inspect Run and Step state;
- replay every migration against empty and non-empty pre-production fixtures.

The package includes the constrained Aegis Life reference host needed to
reproduce dependency direction and its Production compile block. It excludes
Git metadata, build outputs, dependency directories, runtime state and
credentials.

Release status remains:

```text
IDR_TRUST_CHAIN_CLOSURE_COMPLETE = NO
PRODUCTION TRUST ROOT = BLOCKED
PRODUCTION AUTHORIZATION = NOT AUTHORIZED
PRODUCTION EXECUTION = NOT AUTHORIZED
HUMAN MODEL LONG-TERM WRITE = NOT AUTHORIZED
AEGIS PRODUCTION ADAPTER = NOT AUTHORIZED
```
