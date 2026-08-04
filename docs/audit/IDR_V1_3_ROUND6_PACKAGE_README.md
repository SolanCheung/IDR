# IDR V1.3 Trust-Chain Closure Round 6 audit package

This archive is the complete 2026-07-29 audit candidate for The
Human-Centered Intent & Decision Runtime (IDR), including the constrained Aegis
Life reference host.

This is not a production release:

```text
IDR_TRUST_CHAIN_CLOSURE_COMPLETE = NO
PRODUCTION TRUST ROOT = BLOCKED
PRODUCTION AUTHORIZATION = NOT AUTHORIZED
PRODUCTION EXECUTION = NOT AUTHORIZED
HUMAN MODEL LONG-TERM WRITE = NOT AUTHORIZED
AEGIS PRODUCTION ADAPTER = NOT AUTHORIZED
```

Start with:

1. `IDR/IDR-V1.3-Trust-Chain-Closure-Master-Spec.md`
2. `IDR/IDR-V1.3-round5-reaudit-report-2026-07-29.md`
3. `IDR/source-design/IDR-V1.3-original-design.txt`
4. `IDR/docs/audit/IDR_V1_3_ROUND6_AUDIT_MANIFEST.md`
5. `VALIDATION-SUMMARY.md`
6. `idr-v13-round6-validation-output.txt`

Before review:

```bash
unzip -t IDR-V1.3-trust-chain-closure-round6-2026-07-29.zip
cd IDR-V1.3-trust-chain-closure-round6-2026-07-29
shasum -a 256 -c SHA256SUMS
```

The outer `.zip.sha256` file authenticates the archive as delivered.
`SHA256SUMS` authenticates every regular file inside the extracted package.
The archive intentionally contains no symlinks, path-traversal entries, build
outputs, dependency install directories, Git metadata, local databases,
runtime state or credentials.
