# IDR Version Roadmap

Status date: 2026-08-04

## Version state

| Version | State | Production authority |
| --- | --- | --- |
| IDR V1.3 | **Frozen Audit Baseline** — Round 14 final | **Disabled** |
| IDR V1.4 Minimal | **Specification Only** | **Not authorized** |

## IDR V1.3 — frozen baseline

Round 14 is the final IDR V1.3 audit baseline.

```text
Archive: IDR-V1.3-trust-chain-closure-round14-2026-07-31.zip
SHA-256: 4c9a3a106a8cd9c61073d6a1642bfe3a40c37fce294e4e1dd86759b797b2dc42
Status: Frozen Audit Baseline / Production Disabled
```

The following V1.3 surfaces are frozen:

- Rust protocol, runtime, store, and tool code;
- TypeScript interaction client and tests;
- Python evaluation and conformance code;
- PostgreSQL schemas, migrations, roles, functions, and tests;
- Aegis runtime and IDR adapter code;
- dependency manifests and lockfiles;
- cross-language production contracts and generated artifacts.

No feature, refactor, dependency update, migration, or runtime correction is
allowed to alter this baseline in place. A critical correction requires an
explicit new version or emergency erratum, a new immutable package, and a new
audit scope.

Frozen does not mean production-approved. These latches remain unchanged:

```text
IDR_TRUST_CHAIN_CLOSURE_COMPLETE = NO
PRODUCTION TRUST ROOT = BLOCKED
PRODUCTION AUTHORIZATION = NOT AUTHORIZED
PRODUCTION EXECUTION = NOT AUTHORIZED
HUMAN MODEL LONG-TERM WRITE = NOT AUTHORIZED
AEGIS PRODUCTION ADAPTER = NOT AUTHORIZED
```

## IDR V1.4 Minimal — specification and conformance phase

The authorized deliverables are the normative
`IDR-V1.4-EVIDENCE-GOVERNANCE-SPEC.md` and neutral, non-production conformance
artifacts under `contracts/evidence-governance/v1/`. They define and test:

```text
CanonicalFactVersionV1
→ EvidenceQualificationDecisionV1
→ AdmittedEvidenceSnapshotV1
→ Decision Contract Binding
→ Pre-dispatch Revalidation
```

The specification includes exact object boundaries, state enums, field and
hash constraints, freshness, lineage, provenance, supersession, remediation,
Decision/Snapshot binding, dispatch-time validation, invalidation propagation,
and Rust/TypeScript/Python responsibility boundaries.

The initial conformance pack adds strict JSON wire shapes, one
domain-separated Decision evidence-binding golden vector, and negative cases
for snapshot substitution, warning removal, unknown fields, `DENY` admission,
prohibited personality inference, and stale pre-dispatch `PASS`. It does not
issue, persist, qualify, seal, bind, revalidate, or dispatch production state.

Aegis Life's Round 14 `idr-aegis-adapter` remains byte-frozen. A separate
`idr-aegis-shadow-validator` evaluation crate may consume the frozen adapter
to produce observe-only comparison records. It is outside the production path
and exposes no production feature or effect interface.

### Explicitly excluded

- Setoka;
- personality inference or broad psychological profiling;
- vector databases;
- graph memory;
- bitemporal semantic retrieval;
- Human Model features;
- database migrations;
- production code or production enablement.

### Gates before any implementation proposal

No implementation work may begin merely because the specification exists. A
separate authorization and audit scope must first resolve:

1. trusted clock and policy authority;
2. source proof formats and issuer governance;
3. predicate schemas, materiality, and freshness budgets;
4. corroboration independence and human-review authority;
5. completion of canonical cross-language golden vectors and the full negative
   fixture matrix beyond the initial neutral pack;
6. persistence, atomic invalidation propagation, recovery, and privacy design;
7. migration and backward-compatibility strategy;
8. a proof that the frozen V1.3 baseline remains byte-for-byte unchanged.
9. a later-version dependency boundary that can expose offline assessment
   without inheriting the frozen runtime's non-optional `sqlx` dependency.

Until those gates are separately approved:

```text
V1.4 Minimal = Specification Only
IMPLEMENTATION = NOT AUTHORIZED
DATABASE MIGRATION = NOT AUTHORIZED
PRODUCTION ENABLEMENT = NOT AUTHORIZED
```
