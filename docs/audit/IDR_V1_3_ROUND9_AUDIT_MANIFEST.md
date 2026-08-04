# IDR V1.3 Round 9 independent re-audit manifest

Audit candidate date: 2026-07-30

Start with:

1. `docs/audit/reviews/2026-07-30-round8-independent/IDR-V1.3-round8-independent-reaudit-report-2026-07-30.md`
2. `docs/trust-chain-closure/ROUND9-REAUDIT-RESPONSE.md`
3. `docs/trust-chain-closure/ARCHITECTURE.md`
4. `docs/trust-chain-closure/PROOF-BINDINGS.md`
5. `docs/trust-chain-closure/STATE-MACHINES.md`
6. `docs/trust-chain-closure/THREAT-MODEL.md`
7. `docs/trust-chain-closure/VALIDATION-SUMMARY.md`
8. `docs/trust-chain-closure/idr-v13-round9-validation-output.txt`
9. `crates/idr-protocol/src/production.rs`
10. `crates/idr-runtime/src/trust_chain.rs`
11. `crates/idr-runtime/src/postgres_authority.rs`
12. `crates/idr-store/migrations/0004_round9_domain_validity_and_immutability.sql`
13. `crates/idr-store/tests/postgres_trust_chain.rs`
14. `crates/idr-store/tests/postgres_vertical_slice.rs`

Recommended independent attack order:

- replay one signed proof across Shadow/Production and two environment-specific
  schemas while attempting to reuse audience, Trust Root and issuer keys;
- mutate domain/environment in Command, Proof, key policy, Trust Root, public
  record reference, receipt and checkpoint;
- admit an Action, then expire or revoke its exact Authorization before
  reserve, permit delivery and dispatch;
- make lease or permit validity exceed Action, Admission or Authorization;
- UPDATE/DELETE Contract, proof, dependency, audit, receipt, outcome and Human
  Model rows with the application role;
- disable a trigger as a privileged role, mutate Contract JSON or a specialized
  projection, then restart and run integrity verification;
- crash after database commit and before anchor publication, then test
  verified-tail recovery, anchor-ahead rejection and fork rejection;
- cancel an old Action execution, record a new Action and reserve attempt 1;
- submit duplicate Human Model evidence references and oversized rendered bytes
  or idempotency keys;
- replay migration 0004 against empty and non-empty pre-production fixtures.

The package includes the constrained Aegis Life reference host needed to
reproduce dependency direction and its Production compile block. It excludes
Git metadata, build outputs, dependencies, runtime state and credentials.

Release status remains:

```text
IDR_TRUST_CHAIN_CLOSURE_COMPLETE = NO
PRODUCTION TRUST ROOT = BLOCKED
PRODUCTION AUTHORIZATION = NOT AUTHORIZED
PRODUCTION EXECUTION = NOT AUTHORIZED
HUMAN MODEL LONG-TERM WRITE = NOT AUTHORIZED
AEGIS PRODUCTION ADAPTER = NOT AUTHORIZED
```
