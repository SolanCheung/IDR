# IDR V1.3 Trust-Chain Closure Round 14 re-audit package

This 2026-07-31 archive closes only the Round 13 package-replay defect. It does
not contain the separately planned V1.4 architecture delta.

```text
IDR_TRUST_CHAIN_CLOSURE_COMPLETE = NO
PRODUCTION TRUST ROOT = BLOCKED
PRODUCTION AUTHORIZATION = NOT AUTHORIZED
PRODUCTION EXECUTION = NOT AUTHORIZED
HUMAN MODEL LONG-TERM WRITE = NOT AUTHORIZED
AEGIS PRODUCTION ADAPTER = NOT AUTHORIZED
```

Review order:

1. `IDR/docs/audit/reviews/2026-07-31-round13-independent/README.md`
2. `IDR/docs/trust-chain-closure/ROUND14-REAUDIT-RESPONSE.md`
3. `AUDIT-MANIFEST.md`
4. `VALIDATION-SUMMARY.md`
5. `IDR/tools/resolve_aegis_root.sh`
6. `IDR/tools/run_round14_validation.sh`
7. `IDR/tools/validate_round14_archive.sh`

The final-archive validation sequence is:

```bash
archive=IDR-V1.3-trust-chain-closure-round14-2026-07-31.zip
verify_dir="$(mktemp -d)"
unzip -q "$archive" \
  '*/IDR/tools/validate_round14_archive.sh' \
  -d "$verify_dir"
verifier="$(find "$verify_dir" -name validate_round14_archive.sh -print -quit)"
bash "$verifier" "$archive"
```

The verifier creates a second empty temporary directory, validates the outer
and inner hashes, extracts the whole archive and executes the packaged
validation entry against the packaged `Aegis-Life`. Toolchains, PostgreSQL and
offline dependency caches are explicit external prerequisites.

The outer `.zip.sha256` must contain the ZIP basename only; absolute paths and
relative path components are rejected so the same pair can be replayed after
upload or relocation.
