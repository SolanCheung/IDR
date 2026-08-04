# IDR V1.3 Trust-Chain Closure Round 8 re-audit package

This archive is the 2026-07-30 remediation candidate for The Human-Centered
Intent & Decision Runtime (IDR), plus the constrained Aegis Life reference
host. It responds to the five P0 findings in the Round 7 independent review.

This is not a production release:

```text
IDR_TRUST_CHAIN_CLOSURE_COMPLETE = NO
PRODUCTION TRUST ROOT = BLOCKED
PRODUCTION AUTHORIZATION = NOT AUTHORIZED
PRODUCTION EXECUTION = NOT AUTHORIZED
HUMAN MODEL LONG-TERM WRITE = NOT AUTHORIZED
AEGIS PRODUCTION ADAPTER = NOT AUTHORIZED
```

Review order:

1. `IDR/docs/audit/reviews/2026-07-30-round7-independent/IDR-V1.3-round7-independent-reaudit-report-2026-07-30.md`
2. `ROUND8-REAUDIT-RESPONSE.md`
3. `IDR/docs/audit/IDR_V1_3_ROUND8_AUDIT_MANIFEST.md`
4. `VALIDATION-SUMMARY.md`
5. `idr-v13-round8-validation-output.txt`
6. `IDR/crates/idr-runtime/src/trust_chain.rs`
7. `IDR/crates/idr-runtime/src/postgres_authority.rs`
8. `IDR/crates/idr-store/migrations/0003_round8_trust_domains_and_human_model.sql`
9. `IDR/crates/idr-store/tests/postgres_trust_chain.rs`
10. `IDR/crates/idr-store/tests/postgres_vertical_slice.rs`

Before review:

```bash
unzip -t IDR-V1.3-trust-chain-closure-round8-2026-07-30.zip
cd IDR-V1.3-trust-chain-closure-round8-2026-07-30
shasum -a 256 -c SHA256SUMS
```

The outer `.zip.sha256` authenticates the delivered archive. `SHA256SUMS`
authenticates every regular file inside the extracted package except the
manifest itself.

The archive intentionally excludes Git metadata, prior audit-bundle staging,
build outputs, dependency install directories, local databases/runtime state,
real environment files and credentials. It contains no symlinks or
path-traversal entries.
