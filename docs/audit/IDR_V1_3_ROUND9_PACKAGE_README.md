# IDR V1.3 Trust-Chain Closure Round 9 re-audit package

This archive is the 2026-07-30 remediation candidate for The Human-Centered
Intent & Decision Runtime (IDR), plus the constrained Aegis Life reference
host. It responds to the three P0, fourteen P1 and four P2 findings in the
Round 8 independent review. It does not claim every P1 is closed.

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

1. `IDR/docs/audit/reviews/2026-07-30-round8-independent/IDR-V1.3-round8-independent-reaudit-report-2026-07-30.md`
2. `ROUND9-REAUDIT-RESPONSE.md`
3. `IDR/docs/audit/IDR_V1_3_ROUND9_AUDIT_MANIFEST.md`
4. `VALIDATION-SUMMARY.md`
5. `idr-v13-round9-validation-output.txt`
6. the protocol/runtime/PostgreSQL files listed in the audit manifest.

Before review:

```bash
unzip -t IDR-V1.3-trust-chain-closure-round9-2026-07-30.zip
cd IDR-V1.3-trust-chain-closure-round9-2026-07-30
shasum -a 256 -c SHA256SUMS
```

The outer `.zip.sha256` authenticates the delivered archive. `SHA256SUMS`
authenticates every regular file inside the extracted package except the
manifest itself.

The archive intentionally excludes Git metadata, prior audit staging, build
outputs, dependency install directories, local databases/runtime state, real
environment files and credentials. It contains no symlinks or path-traversal
entries.
