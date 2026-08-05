# IDR × Aegis Life Shadow Validation Plan

Status: **Validation Only / No Production Authority**  
Date: 2026-08-05

## Objective

Use Aegis Life as a reference host to collect evidence about IDR assessment
behavior without allowing IDR to execute an effect, mutate production state,
block the primary product path, or become a production trust root.

IDR V1.3 Round 14 remains frozen and Production Disabled. IDR V1.4 Minimal
remains Specification Only.

## Isolation boundary

```text
Aegis exported assessment candidate
        |
        v
Frozen idr-aegis-adapter::assess_shadow
        |
        v
Separate idr-aegis-shadow-validator
        |
        +--> append-only comparison record
        |
        `--> no Action / no Permit / no dispatch / no product write
```

Every record MUST declare:

```text
effect_authority = none
dispatch = forbidden
production_state_mutation = forbidden
```

The validator exposes no database connection, network provider, production
credential, dispatch carrier, Action issuer, Authorization issuer, or
Execution Permit type, and performs no database or network call. The frozen
`idr-runtime` Cargo manifest has a non-optional transitive `sqlx` dependency;
removing it is a later-version dependency-boundary task, not a V1.3 edit. The
primary Aegis result never depends on validator success.

## Current phase: Offline Replay

The versioned corpus format, privacy gate, label semantics, and metric contract
are defined in
[`IDR-AEGIS-OFFLINE-CORPUS-SPEC.md`](IDR-AEGIS-OFFLINE-CORPUS-SPEC.md).
The reference host implementation lives in the separate Aegis Life
`idr-aegis-shadow-validator` crate; it is not part of the frozen V1.3 runtime.

Authorized inputs:

- explicitly exported JSON replay envelopes;
- synthetic and audit fixtures;
- an optional host projection limited to coordination mode, action posture,
  and next run state.

Authorized outputs:

- deterministic request digest;
- IDR assessment or stable rejection code;
- exact-match or field-level divergence result;
- static observe-only safety declaration.

Raw conversation text, credentials, secrets, provider payloads, long-term Human
Model data, broad personality inferences, and production authorization objects
MUST NOT enter the replay envelope.

## Required measurements

For an evaluation corpus, report:

1. total envelopes, parsed envelopes, and rejected envelopes;
2. exact-match rate;
3. divergence counts for coordination mode, action posture, and run state;
4. stable rejection-code distribution;
5. request-digest duplicate rate;
6. corpus source and synthetic/real classification;
7. privacy-redaction failures;
8. any attempted unknown-field or authority-field injection;
9. validator latency p50/p95/p99 measured by the external evaluation runner;
10. confirmation that no product dispatch or state-write interface was linked.

No target accuracy threshold is declared before a representative corpus
exists. A high match rate alone is not evidence of correctness; every
divergence requires classification as host defect, IDR defect, mapping defect,
policy difference, or insufficient evidence.

## Failure behavior

- Invalid replay JSON: reject the observation; do not affect Aegis.
- Invalid IDR contract: emit a shadow rejection record; do not affect Aegis.
- Validator crash or timeout: record evaluation infrastructure failure outside
  the product path; do not retry an external effect.
- Host/IDR divergence: record only; do not select a winner automatically.
- Unknown field or attempted authority injection: reject fail-closed.

## Advancement gates

### Gate A — Offline corpus

- deterministic replay is stable;
- all safety-boundary tests pass;
- a versioned, privacy-reviewed corpus exists;
- divergences have human-reviewed classifications;
- the frozen adapter still matches its Round 14 hashes.

The synthetic smoke corpus, 40-case boundary matrix, and batch evaluator
satisfy the mechanics of Gate A. The matrix explicitly reports
`act_then_respond` as unobserved because the frozen selector has no returning
branch for that enum value. These artifacts do not establish representative
accuracy. Gate A remains open until a separately approved, representative
corpus exists and its divergences are reviewed.

Any future representative candidate must first satisfy
[`IDR-AEGIS-REPRESENTATIVE-CORPUS-INTAKE-SPEC.md`](IDR-AEGIS-REPRESENTATIVE-CORPUS-INTAKE-SPEC.md),
including externally authorized export, exact structured-facts allowlisting,
retention/deletion controls, lineage and candidate digests, and independent
human labels bound to those digests. The current intake fixture is synthetic
and grants no access to Aegis production data.

The tracked-source review in
[`IDR-AEGIS-SOURCE-DATA-FLOW-REVIEW.md`](IDR-AEGIS-SOURCE-DATA-FLOW-REVIEW.md)
is complete. It found only candidate ingress correlation metadata and no
complete source for the IDR input, fact, or host-projection contract. The
representative exporter therefore remains `BLOCKED`; completing source
reconnaissance does not close Gate A.

### Gate B — Passive capture

Requires a separate review. Capture MUST remain asynchronous and non-blocking,
store only allowlisted fields/digests, use a dedicated evaluation sink, and
have retention/deletion controls. No production runtime change is authorized
by this plan.

### Gate C — Live mirror

Requires an independent security review, resource/latency budgets, sampling,
kill switch, privacy controls, and proof that mirror failure cannot influence
the primary path. Live mirror still has no dispatch authority.

### Production consideration

Out of scope. It requires a separately approved IDR version, completion of the
V1.4 conformance matrix, trusted clock and policy authority decisions, source
proof governance, persistence and recovery design, migration plan, and two
independent audits.

## Explicit non-goals

- no Setoka;
- no personality or psychological inference;
- no vector database or graph memory;
- no bitemporal semantic retrieval;
- no Human Model write;
- no database migration;
- no production enablement;
- no reinterpretation of IDR `BLOCK`, `DENY`, or warnings.
