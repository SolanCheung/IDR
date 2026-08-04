# IDR V1.3 Trust-Chain Closure Round 12 re-audit package

This archive is the 2026-07-30 remediation candidate for The Human-Centered
Intent & Decision Runtime (IDR), plus the constrained Aegis Life reference
host. It responds to the Round 11 independent review.

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

1. `IDR/docs/audit/reviews/2026-07-30-round11-independent/IDR-V1.3-round11-independent-reaudit-report-2026-07-30.md`
2. `ROUND12-REAUDIT-RESPONSE.md`
3. `AUDIT-MANIFEST.md`
4. `VALIDATION-SUMMARY.md`
5. `IDR/docs/trust-chain-closure/idr-v13-round12-validation-output.txt`
6. migration 0007, the PostgreSQL authority runtime, attack tests and
   role-separation document listed in the manifest.

Before review:

```bash
unzip -t IDR-V1.3-trust-chain-closure-round12-2026-07-30.zip
cd IDR-V1.3-trust-chain-closure-round12-2026-07-30
shasum -a 256 -c SHA256SUMS
```

The outer `.zip.sha256` authenticates the delivered archive. `SHA256SUMS`
authenticates every regular file inside the extracted package except itself.

The archive is assembled from explicit source allowlists. It excludes Git
metadata, build output, dependencies, runtime state, real environment files,
`.aegis`, credentials and key containers. It contains no symlinks. Secret
scanning reports redacted metadata and SHA-256 fingerprints only.

P0-00 remains externally open: source cannot prove that the previously exposed
provider credential was revoked/rotated or that usage and billing history were
reviewed. Round 12 also does not provide production KMS/WORM/Trust Root control
plane evidence.
