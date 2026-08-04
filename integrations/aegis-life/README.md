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
`docs/integrations/IDR-AEGIS-OFFLINE-CORPUS-SPEC.md`.
