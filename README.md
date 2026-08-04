# The Human-Centered Intent & Decision Runtime

An independent runtime for turning multi-source input into auditable intent,
decision, response, action, execution, outcome, and human-model contracts.

The official short name is **IDR**.

## Version status

| Version | Status | Boundary |
| --- | --- | --- |
| IDR V1.3 | **Frozen Audit Baseline / Production Disabled** | Round 14 is the final V1.3 audit baseline. Its Rust, TypeScript, Python, PostgreSQL, and Aegis runtime code is frozen. |
| IDR V1.4 Minimal | **Specification Only** | Defines evidence qualification, admitted snapshots, Decision binding, and pre-dispatch revalidation. No production implementation is authorized. |

The frozen V1.3 baseline is the Round 14 archive identified by SHA-256
`4c9a3a106a8cd9c61073d6a1642bfe3a40c37fce294e4e1dd86759b797b2dc42`.
Freezing an audit baseline does not authorize production: all V1.3 production
latches remain blocked. Any required V1.3 runtime correction MUST reopen the
audit under a new, explicit version or emergency erratum; it MUST NOT silently
change the frozen baseline.

The target production trust boundary is implemented in Rust. Round 11 through
Round 14 audit evidence is preserved under `docs/audit/` and
`docs/trust-chain-closure/`. The Round 13 runtime remediation fixes the
TEMP-table shadowing finding:
owner-executed PostgreSQL functions have an explicit trusted search path,
Runtime can execute only the constant-work v8 HMAC opener, and Production
startup rejects TEMP-capable or extra-role-capable login identities. Runtime
still has zero authority base-table DML; every accepted view write remains
bound to the Rust Orchestrator's database transaction attestation.
Production startup is deliberately hard blocked until the remaining release
gates and two independent audits pass.
TypeScript owns product integration, while Python is restricted to offline
research and evaluation.

Round 14 changed no semantic authority code. It made the final audit ZIP
clean-room replayable through one shared Aegis path resolver and a post-package
validation entry. It is now the frozen V1.3 audit baseline, not a production
release.

See `docs/trust-chain-closure/ROUND14-REAUDIT-RESPONSE.md` for the current
response to the Round 13 package audit. See
`docs/architecture/IDR-V1.4-EVIDENCE-GOVERNANCE-SPEC.md` for the V1.4 Minimal
specification and `docs/architecture/ROADMAP.md` for the version roadmap.

## Repository policy

This repository preserves IDR V1.3 as a frozen, production-disabled audit
baseline while V1.4 Minimal remains specification-only. A commit in this
repository is not evidence of production authorization. Generated build
outputs, local credentials, caches, and packaged audit archives are excluded
from version control; reproducible audit evidence and validation reports remain
under `docs/`.

## Validation-only surfaces

The V1.4 Minimal neutral conformance artifacts live under
`contracts/evidence-governance/v1/`. They provide strict wire-shape schemas, a
domain-separated Decision/Snapshot binding golden vector, and negative cases
for substitution, warning removal, unknown fields, `DENY` admission,
prohibited personality inference, and stale pre-dispatch `PASS` reuse. These
artifacts are specification tests, not production implementation.

Aegis Life validates the frozen V1.3 adapter through a separate
`idr-aegis-shadow-validator` crate. The validator can replay and compare
assessments but has no production authority, dispatch, provider, database, or
state-write interface. It performs no database or network call; the frozen
`idr-runtime` manifest still contributes a transitive `sqlx` dependency. See
`integrations/aegis-life/README.md`.

## Repository layout

- `crates/idr-protocol`: Candidate/Proof wire boundary, Ed25519 Trust Root
  verification and constrained RFC 8785 canonical encoding. The older API is
  available only through `legacy-shadow-api`.
- `crates/idr-runtime`: the single concrete command-driven authoritative
  issuance, PostgreSQL transaction and state-transition boundary.
- `crates/idr-store`: reexports the PostgreSQL authority and provides a
  `dev-file-store` that cannot compile together with `production`.
- `packages/interaction-client`: TypeScript transport and authorization client.
- `research/evaluation`: Python golden-fixture evaluation.
- `contracts`: language-neutral fixtures.
- `integrations/aegis-life`: integration contract documentation.

## Aegis Life relationship

Aegis Life is the first reference host, not the owner of this runtime. Its
`idr-aegis-adapter` crate depends on these Rust crates through explicit path
dependencies. It converts Aegis authority data only into an untrusted host
candidate; it cannot mint an IDR Authority Context or grant.

## Validation

```bash
cargo test --workspace --offline
IDR_TEST_DATABASE_URL=postgresql:///postgres cargo test -p idr-store \
  --no-default-features --features shadow-mode \
  --test postgres_trust_chain --test postgres_vertical_slice --offline
cargo clippy --workspace --all-targets --offline -- -D warnings
cargo clippy -p idr-store --no-default-features --features production \
  --offline -- -D warnings

cd packages/interaction-client
npm ci --offline
npm test
npm run typecheck

cd ../../research/evaluation
PYTHONPATH=src python3 -m unittest discover -s tests -v
```

## License

All rights are reserved until an explicit distribution license is selected.
See `LICENSE`.
