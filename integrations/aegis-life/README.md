# Aegis Life reference integration

Aegis Life consumes IDR through its `crates/idr-aegis-adapter` crate.

The adapter is deliberately thin:

- it maps Aegis production references and authority data into
  `HostAuthorityCandidateV1`; a candidate is not an IDR Authority Context or
  grant;
- it exposes a narrow `assess_shadow` façade and does not re-export raw
  protocol, runtime, or store crates;
- it does not duplicate IDR guards or grant authority;
- it allows Aegis Life to act as a shadow-mode reference host.
- it does not depend on `idr-store`.

The adapter now has explicit Cargo features:

- default `shadow-mode` enables only the legacy shadow assessment surface;
- `aegis-production-adapter` is compile-blocked until the IDR production
  release latch and independent audit gates pass.

The adapter is not a production execution admission boundary. IDR remains
blocked for production trust-root use until the open requirements in
`docs/audit/IDR_V1_3_REMEDIATION.md` are closed and independently re-audited.

## Separate shadow validator

The Round 14 adapter files remain byte-for-byte frozen. New evaluation behavior
is isolated in Aegis Life's `crates/idr-aegis-shadow-validator` crate rather
than added to the adapter.

The validator may:

- replay an `InteractionAssessmentRequestV1` through the frozen façade;
- bind the replay input to a deterministic request digest;
- compare an existing Aegis projection with the IDR assessment;
- capture an IDR rejection as evaluation data;
- emit an append-only record declaring `effect_authority = none`,
  `dispatch = forbidden`, and `production_state_mutation = forbidden`.

It has no production feature, database connection API, provider, Action,
Execution Permit, dispatch carrier, or state-write interface, and performs no
database or network call. Because V1.3 is frozen, its runtime's non-optional
`sqlx` dependency remains present transitively in the validation binary;
dependency decoupling requires a later IDR version. Shadow output cannot
authorize or alter the Aegis product path.

The phased corpus, metric, privacy, failure-isolation, and advancement gates
are defined in
`docs/integrations/IDR-AEGIS-SHADOW-VALIDATION-PLAN.md`. The concrete offline
corpus object and report contract are defined in
`docs/integrations/IDR-AEGIS-OFFLINE-CORPUS-SPEC.md`. The separate intake,
redaction, retention, deletion, lineage, and human-label binding boundary is
defined in
`docs/integrations/IDR-AEGIS-REPRESENTATIVE-CORPUS-INTAKE-SPEC.md`, with
synthetic conformance artifacts under
`contracts/integrations/aegis-life/representative-corpus-intake/v1/`.

The tracked Aegis source-to-corpus review is documented in
`docs/integrations/IDR-AEGIS-SOURCE-DATA-FLOW-REVIEW.md`. It covers every
allowlisted replay leaf and pins the reviewed Aegis commit and source-file
digests. The result is `REPRESENTATIVE_EXPORTER = BLOCKED`: current ingress
metadata is incomplete, raw text coexists in Journal/continuity surfaces, and
the required approval, trusted-time, retention, deletion-receipt, labeling,
and dedicated-sink controls do not exist.

The specification-only source object and pure mapping are defined in
`docs/integrations/IDR-AEGIS-STRUCTURED-OBSERVATION-SPEC.md`, with synthetic
conformance artifacts under
`contracts/integrations/aegis-life/structured-observation/v1/`. This resolves a
wire-design gap only. It does not add an Aegis producer or change the blocked
export decision.

The five producer responsibility profiles and their fail-closed implementation
gate are defined in
`docs/integrations/IDR-AEGIS-PRODUCER-TRUST-ADMISSION-SPEC.md`, with synthetic
conformance artifacts under
`contracts/integrations/aegis-life/producer-trust-admission/v1/`. All producer
states remain `NOT_IMPLEMENTED`; this is not implementation or admission.

The upstream receipt-owner and source semantics are defined in
`docs/integrations/IDR-AEGIS-INGRESS-RECEIPT-TRUST-SPEC.md`, with synthetic
conformance artifacts under
`contracts/integrations/aegis-life/ingress-receipt-trust/v1/`. It does not
implement the ingress owner, monotonic clock, or receipt-source binding.

The independent Host Decision Snapshot and exact 7/4/11 semantic mappings are
defined in
`docs/integrations/IDR-AEGIS-HOST-PROJECTION-SEMANTIC-MAPPING-SPEC.md`, with
synthetic artifacts under
`contracts/integrations/aegis-life/host-projection-mapping/v1/`. The product
snapshot owner and mapper remain unimplemented.
