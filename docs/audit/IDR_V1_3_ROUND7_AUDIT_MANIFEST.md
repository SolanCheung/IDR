# IDR V1.3 Round 7 independent re-audit manifest

Audit date: 2026-07-29

Start with:

1. `docs/audit/reviews/2026-07-29-round6-independent/IDR-V1.3-round6-independent-reaudit-report-2026-07-29.md`
2. `docs/trust-chain-closure/ROUND7-REAUDIT-RESPONSE.md`
3. `docs/trust-chain-closure/API-VISIBILITY.md`
4. `docs/trust-chain-closure/TEST-MATRIX.md`
5. `docs/trust-chain-closure/VALIDATION-SUMMARY.md`
6. `docs/trust-chain-closure/idr-v13-round7-validation-output.txt`
7. `crates/idr-runtime/src/trust_chain.rs`
8. `crates/idr-runtime/src/postgres_authority.rs`
9. `crates/idr-store/migrations/0002_round7_integrity_and_idempotency.sql`
10. `crates/idr-store/tests/postgres_trust_chain.rs`
11. `crates/idr-store/tests/postgres_vertical_slice.rs`

Recommended independent attack order:

- compile downstream authority/repository/transition/pool/inspect bypasses;
- replay a command ID with different canonical bytes;
- mutate projection JSON and unhashed audit columns;
- switch subject and turn within an existing Run;
- substitute Decision option/operation/parameters in Action;
- issue a successor and verify recursive dependent invalidation;
- invoke execution lifecycle commands after cancellation and under the wrong
  owner/provider;
- reuse operation/idempotency scope across Action revisions;
- substitute Receipt request digest;
- substitute Outcome and promoted Human Model content;
- inspect production exports for any unauthenticated raw read/outbox control.

The package includes the complete constrained Aegis Life reference host so the
dependency direction and production compile block can be reproduced. It
excludes Git metadata, build outputs, dependency directories, local runtime
state and credentials.

Release status remains:

```text
IDR_TRUST_CHAIN_CLOSURE_COMPLETE = NO
PRODUCTION TRUST ROOT = BLOCKED
PRODUCTION AUTHORIZATION = NOT AUTHORIZED
PRODUCTION EXECUTION = NOT AUTHORIZED
HUMAN MODEL LONG-TERM WRITE = NOT AUTHORIZED
AEGIS PRODUCTION ADAPTER = NOT AUTHORIZED
```
