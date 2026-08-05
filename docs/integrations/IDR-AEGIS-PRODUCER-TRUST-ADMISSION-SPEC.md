# IDR × Aegis Life Producer Trust Admission Specification V1

Status: **Specification Only / All Producers Not Implemented**

Date: 2026-08-05

## Purpose

This specification defines who may produce each fact family inside a future
`AegisStructuredObservationV1`, which inputs that producer may consume, which
proofs are required before its output can be trusted, and which conditions
must block implementation admission.

It does not implement, approve, or deploy a producer. It does not authorize
real-data access, raw-content retention, corpus export, passive capture, or a
production integration.

## Boundary

The 42-leaf Shadow projection has two ownership classes:

```text
Ingress receipt owner                         Fact producer owners
11 receipt and lineage leaves                31 fact leaves
        |                                            |
        v                                            v
IngressReceiptV1                    five receipt-bound assertions
        |                                            |
        +--------------------+-----------------------+
                             v
                AegisStructuredObservationV1
                             |
                             | separate intake approval required
                             v
                 Representative Corpus Intake
```

No fact producer owns an event, run, turn, actor, content, correlation, or
logical-time identity. No ingress receipt owner may invent semantic,
fast-path, decision, coordination, or host-projection facts.

The ingress owner's exact source and logical-time contract is specified by
[`IDR-AEGIS-INGRESS-RECEIPT-TRUST-SPEC.md`](IDR-AEGIS-INGRESS-RECEIPT-TRUST-SPEC.md).
Fact-producer admission requires its receipt-source binding, but that binding
and owner remain unimplemented.

## Trust is not schema validity

These statements are intentionally distinct:

```text
producer profile is schema-valid
    != producer implementation exists
    != producer implementation is authoritative
    != observation is admissible
    != export is authorized
```

The conformance pack proves only that the responsibility registry is internally
consistent. A future producer must pass every implementation gate using proof
bound to an exact implementation tree, policy revision, configuration/model
artifact, target observation schema, and source-review pin.

## AegisProducerTrustRegistryV1

The registry contains exactly:

```text
schema_id
schema_version
registry_ref
target_observation_schema
source_review_pin
trust_policy
lifecycle
producers
implementation_gate
registry_digest
```

Unknown fields are rejected. Strings are NFC. Digests use lowercase SHA-256.
Git object identifiers use lowercase hexadecimal. JSON numbers are
non-negative safe integers.

The registry is immutable. Any change to ownership, inputs, evidence
requirements, source pin, policy, producer state, or implementation gate
creates a new registry and digest.

### Target observation binding

`target_observation_schema` binds:

- exact `schema_id` and `schema_version`;
- repository-relative schema path;
- SHA-256 digest of the schema artifact;
- the required assertion receipt-binding mode.

A producer approved against one schema digest is not approved against another.
Schema compatibility inference is forbidden at the trust boundary.

### Source review pin

`source_review_pin` binds the repository, reviewed commit, reviewed tree,
review document, and review scope. The reviewed scope is tracked source and
documentation only. It explicitly excludes working-tree changes, untracked
files, databases, journals, persistence snapshots, credentials, network
services, and user data.

Admission against an unpinned or different source tree requires a new source
review. Similar paths or unchanged type names are not sufficient.

### Trust policy

`trust_policy` binds a policy reference, policy version, and domain-separated
policy digest. Changing either value invalidates the registry and all producer
admission decisions derived from it.

Policy digest domain:

```text
idr:aegis:producer-trust-policy:v1
```

## ProducerTrustProfileV1

Exactly one profile is required for each family:

| Family | Producer kind | Owner boundary | Fact leaves |
| --- | --- | --- | ---: |
| `semantic_roles` | `semantic_classifier` | `semantic_ingress` | 2 |
| `fast_path_facts` | `command_policy_classifier` | `command_policy` | 8 |
| `decision_facts` | `decision_context_classifier` | `decision_context` | 10 |
| `coordination_facts` | `coordination_planner` | `coordination_planning` | 8 |
| `host_projection` | `aegis_host_decision_runtime` | `host_decision_runtime` | 3 |

The profiles must cover all 31 fact paths exactly once. Missing, extra,
duplicate, or cross-family ownership is denied.

Each profile contains:

```text
family
producer_kind
owner_boundary
responsibility
input_policy
derivation_policy
required_proofs
trust_state
profile_digest
```

### Responsibility

`responsibility` binds:

- the exact owned fact paths;
- `required_output_mode = receipt_bound_assertion`;
- `receipt_binding = exact_receipt_digest`;
- `failure_mode = deny_assertion`;
- `independence_rule = no_idr_or_downstream_feedback`.

Fail-open defaults, partial assertions, inferred missing facts, and best-effort
normalization are forbidden. A producer that cannot produce every owned fact
for the exact receipt must produce no assertion.

### Input policy

Each family has an exact allowlist of typed input classes. Anything not listed
is denied. The shared prohibited classes include:

- `idr_assessment` and `idr_decision`;
- `shadow_comparison_result` and `evaluation_label`;
- `chosen_action`, `execution_result`, and `goal_outcome`;
- `journal_record`, `persistence_snapshot`, and `trace_diagnostic`;
- provider responses, arbitrary JSON, and offline-corpus feedback.

This prevents reverse-inference and circular evaluation.

Only the `semantic_roles` and `fast_path_facts` producers may be considered for
ephemeral access to `ephemeral_user_input`. That access must remain inside the
product ingress boundary, must not appear in the observation, and must never be
persisted, logged, exported, cached, or copied to model telemetry. All other
producers have `raw_content_access = none`.

Every profile requires:

```text
raw_content_persistence = forbidden
unstructured_journal_read = forbidden
arbitrary_json_input = forbidden
```

### Derivation policy

The profile enumerates allowed derivation classes. `governed_model` or `mixed`
requires immutable model/configuration artifacts, evaluation evidence, bounded
failure behavior, and a reviewed policy revision. A natural-language prompt,
confidence value, or model name alone is not an implementation identity.

Human review may evaluate a candidate implementation, but a human-created fact
cannot silently replace a runtime producer unless `human_reviewed` is an
explicitly admitted derivation class and its exact workflow is digest-bound.

Every producer must prove `idr_output_used = false` and must operate without
Shadow comparison results or later actions/outcomes.

### Required proofs

All profiles require evidence for:

1. exact implementation and build artifact identity;
2. exact policy and configuration/model identity;
3. receipt lineage and family-specific fact coverage;
4. prohibited-input non-use and no circular derivation;
5. fail-closed behavior and malformed-input rejection;
6. deterministic replay or governed-model reproducibility bounds;
7. security, privacy, failure isolation, revocation, and rollback;
8. cross-language serialization and digest conformance.

Evidence references alone are insufficient. A future admission decision must
verify each proof against trusted current time and the exact digests it names.

## Family responsibility matrix

### Semantic roles

Owns:

```text
semantic_roles.primary_semantic_role
semantic_roles.semantic_roles
```

Allowed typed inputs are the ingress receipt, ephemeral user input, locale
context, and the exact semantic taxonomy revision. It must not treat a planned
action, IDR assessment, or goal outcome as evidence of the user's semantic
role.

### Fast-path facts

Owns the exact eight `fast_path_facts.*` leaves. Allowed typed inputs are the
ingress receipt, ephemeral user input, authority context, command registry,
parameter-validation result, and tenant policy context. Matching a command
does not prove authority or policy permission; each Boolean remains separately
owned and tested.

### Decision facts

Owns the exact ten `decision_facts.*` leaves. Allowed typed inputs are the
ingress receipt, the sealed semantic-role assertion, structured decision
context, verified evidence-conflict summary, and tenant policy context. It may
not consume the IDR result, chosen action, execution result, or goal outcome.

### Coordination facts

Owns the exact eight `coordination_facts.*` leaves. Allowed typed inputs are the
ingress receipt, sealed semantic and decision assertions, structured
coordination context, authority context, and tenant policy context. It may not
infer facts from a dispatch, provider response, or UI completion state.

### Host projection

Owns the exact three `host_projection.*` leaves. Allowed typed inputs are the
ingress receipt, the four upstream sealed assertions, and typed Aegis host
runtime state. It must exist before IDR assessment and remain independent of
the Shadow comparator. Constructing it from an IDR decision is circular and is
always denied.

## Trust state model

A future profile may move only through immutable decisions:

```text
NOT_IMPLEMENTED
      |
      v
IMPLEMENTATION_REVIEW_READY
      |
      v
ADMITTED --> SUSPENDED --> ADMITTED
   |             |
   +-------------+--> REVOKED
```

- `NOT_IMPLEMENTED`: no implementation identity or proof packet exists;
- `IMPLEMENTATION_REVIEW_READY`: a complete, pinned proof packet exists but is
  not trusted;
- `ADMITTED`: all gates passed for exact bound artifacts and validity window;
- `SUSPENDED`: use is temporarily denied pending a bounded review;
- `REVOKED`: the bound implementation may never produce trusted assertions.

The Phase 7 registry requires all five profiles to be `NOT_IMPLEMENTED`.
Changing a fixture to `ADMITTED` without a separately specified admission
decision and proof contract is rejected.

## Implementation gate

The registry carries one fail-closed gate with these exact checks:

```text
responsibility_contract_locked
source_owner_approved
implementation_registered
implementation_tree_pinned
policy_and_model_artifacts_pinned
trusted_logical_clock_verified
receipt_lineage_verified
exact_fact_coverage_verified
no_circular_derivation_verified
failure_isolation_verified
security_privacy_review_passed
revocation_rollback_tested
cross_language_conformance_passed
```

Only `responsibility_contract_locked` is satisfied in this phase. Every other
check is unsatisfied. One unsatisfied, expired, mismatched, suspended, or
revoked check makes the decision `BLOCKED`.

Implementation admission is still not export approval. Even after all producer
checks pass, corpus intake, consent/lawful basis, export approval, retention,
deletion, labeling, and dedicated-sink controls remain separate gates.

Gate digest domain:

```text
idr:aegis:producer-implementation-gate:v1
```

## Hash binding

Each producer profile uses:

```text
idr:aegis:producer-trust-profile:v1:<family>
```

The digest covers every profile field except `profile_digest`.

The registry uses:

```text
idr:aegis:producer-trust-registry:v1
```

It covers schema identity, registry identity, target schema binding, source
pin, trust policy including its digest, lifecycle, all profiles including their
digests, and the implementation gate including its digest. Only
`registry_digest` is excluded.

Canonical JSON and the digest framing rules are identical to the structured
observation pack: UTF-8 canonical JSON with a NUL byte between the ASCII domain
and the encoded object.

## Responsibility boundary

- Aegis Rust may eventually implement typed receipt and fact producers after a
  separate implementation approval. It must fail closed and seal producer
  identity, policy, and receipt lineage.
- TypeScript may validate and transport admitted producer metadata. It may not
  produce facts, widen input allowlists, or recompute IDR decisions.
- Python may test producer quality against approved synthetic or admitted
  corpora. It may not turn evaluation output into source facts or admission.
- IDR Rust remains the sole semantic assessment kernel and is not modified by
  this specification.

## Current result

```text
PRODUCER_RESPONSIBILITY_CONTRACT = SPECIFIED
PRODUCER_TRUST_REGISTRY = SPECIFIED
AUTHORITATIVE_FACT_PRODUCERS = RESPONSIBILITIES_SPECIFIED_NOT_IMPLEMENTED
PRODUCER_IMPLEMENTATION_GATE = BLOCKED
AEGIS_STRUCTURED_OBSERVATION_PRODUCER = NOT_IMPLEMENTED
REPRESENTATIVE_EXPORTER = BLOCKED
REAL_AEGIS_DATA_ACCESS = NOT_AUTHORIZED
```

## Explicit exclusions

- no Aegis or IDR V1.3 production-code change;
- no producer, model call, prompt, exporter, passive capture, or live mirror;
- no database, migration, retention worker, or deletion worker;
- no real user data or untracked Aegis source;
- no new dependency;
- no Setoka, personality inference, Human Model feature, vector database,
  graph memory, or bitemporal semantic retrieval.
