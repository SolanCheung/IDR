# IDR V1.3 Trust-Chain Closure Round 13 re-audit package

This archive is the 2026-07-30 remediation candidate for The Human-Centered
Intent & Decision Runtime (IDR), plus the constrained Aegis Life reference
host. It responds to the Round 12 independent static audit.

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

1. `IDR/docs/audit/reviews/2026-07-30-round12-independent/README.md`
2. `IDR/docs/audit/reviews/2026-07-30-round12-independent/idr-v13-round12-static-audit-checks.txt`
3. `ROUND13-REAUDIT-RESPONSE.md`
4. `AUDIT-MANIFEST.md`
5. `VALIDATION-SUMMARY.md`
6. `IDR/docs/trust-chain-closure/idr-v13-round13-validation-output.txt`
7. migration 0008, the PostgreSQL authority runtime, the TEMP-shadow attack
   test and role-separation document listed in the manifest.

Before review:

```bash
unzip -t IDR-V1.3-trust-chain-closure-round13-2026-07-30.zip
cd IDR-V1.3-trust-chain-closure-round13-2026-07-30
shasum -a 256 -c SHA256SUMS
```

The archive is assembled from explicit source allowlists. It excludes Git
metadata, build output, dependencies, runtime state, real environment files,
credentials, secret containers and symlinks. The secret scanner reports
redacted metadata and SHA-256 fingerprints only.

Provider credential closure and production KMS/WORM/Trust Root control-plane
evidence remain externally open.

