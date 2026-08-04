# IDR V1.3 Round 6 audit manifest

Audit candidate date: 2026-07-29

System: The Human-Centered Intent & Decision Runtime (IDR)

Release status: `PRODUCTION TRUST ROOT = BLOCKED`

## Governing evidence

The audit package must contain these immutable inputs at the IDR repository
root:

- `IDR-V1.3-Trust-Chain-Closure-Master-Spec.md`
- `IDR-V1.3-round5-reaudit-report-2026-07-29.md`
- `source-design/IDR-V1.3-original-design.txt`

Their expected SHA-256 digests are:

```text
e5a650f0280a6a887678a3856f993ac2fa12355333f7d3bd96ba5e3c287ecd1f  IDR-V1.3-Trust-Chain-Closure-Master-Spec.md
64f169e76ea6629ccc22fd9b319c9e3e9c4f5bea4d21ae5828760c844b982357  IDR-V1.3-round5-reaudit-report-2026-07-29.md
394a75929a8605e2b158c1b4533b497c61152f6f5f8d8c2325a385908c1b6432  source-design/IDR-V1.3-original-design.txt
```

## Required source scope

Review the complete `IDR/` and `Aegis-Life/` trees in the package, including:

- Rust manifests, lockfiles, source, migrations, tests and trybuild stderr;
- generated JSON Schema, golden fixture, TypeScript and Python bindings;
- TypeScript package, tests and offline vendor artifacts;
- Python evaluator and conformance tests;
- Trust-Chain Closure architecture, proof, state-machine, threat, migration,
  API-visibility, test and risk documents;
- Aegis Life `idr-aegis-adapter`, its feature gates, integration tests and all
  host code that could obtain or manufacture IDR authority.

Build outputs, `node_modules`, caches, local databases, runtime state, secrets,
`.git` metadata and previous audit bundles are intentionally excluded.

## Required validation evidence

`VALIDATION-SUMMARY.md` states the executed checks and gate conclusion.
`idr-v13-round6-validation-output.txt` is the unedited combined stdout/stderr
from `tools/run_round6_validation.sh`. The script must end with:

```text
ALL_VALIDATION_STEPS=PASS
```

Expected-failure compile gates are a required success condition:

- `idr-store` with `production + dev-file-store`;
- Aegis `idr-aegis-adapter` with `aegis-production-adapter`.

`SHA256SUMS` covers every packaged regular file except itself. Audit intake
must first reject path traversal and symlink entries, then verify every digest.

## Independent review decision

This package is a Round 6 audit candidate, not a production release. Reviewers
must independently test the code and report every Master Spec gate. No local
test result may change the release latch. Production remains blocked until all
remaining release blockers are implemented and two consecutive independent
clean reviews find no trust-boundary P0.

Required gate labels:

```text
IDR_TRUST_CHAIN_CLOSURE_COMPLETE
PRODUCTION_TRUST_ROOT
PRODUCTION_AUTHORIZATION
PRODUCTION_EXECUTION
HUMAN_MODEL_LONG_TERM_WRITE
AEGIS_PRODUCTION_ADAPTER
```
