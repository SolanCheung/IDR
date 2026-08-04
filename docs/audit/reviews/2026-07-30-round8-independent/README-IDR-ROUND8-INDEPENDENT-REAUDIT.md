# IDR V1.3 Round 8 independent re-audit bundle

Contents:

- `IDR-V1.3-round8-independent-reaudit-report-2026-07-30.md`: full independent review.
- `idr-v13-round8-integrity-output.txt`: ZIP, path, symlink and SHA256SUMS verification.
- `idr-v13-round8-independent-test-output.txt`: independently executed TypeScript and Python results.
- `idr-v13-round8-static-audit-checks.txt`: source-level invariant evidence.

The review environment did not provide Cargo/Rust/PostgreSQL, so Rust and PostgreSQL results in the submitted package were not independently re-executed. TypeScript and Python were independently executed.

Release decision remains fail-closed.
