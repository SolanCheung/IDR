# IDR × Aegis Life Structured Observation Specification V1

Status: **Specification Only / No Producer or Export Authorization**

Date: 2026-08-05

## Purpose

This specification defines the smallest product-owned, raw-content-free source
object from which a future Aegis Life integration could construct one
`ShadowReplayEnvelopeV1` without reverse-inference from an action, outcome, or
IDR assessment.

The object is named `AegisStructuredObservationV1`. It closes a schema-design
gap found by the source data-flow review. It does not implement a producer,
capture path, database, exporter, retention operator, deletion mechanism, or
real-data permission.

## Boundary

```text
Aegis ingress receipt metadata
        |
        +--> semantic-role assertion
        +--> fast-path assertion
        +--> decision-facts assertion
        +--> coordination-facts assertion
        `--> independent host-projection assertion
                    |
                    v
          AegisStructuredObservationV1
                    |
                    | pure, allowlisted projection only
                    v
             ShadowReplayEnvelopeV1
                    |
                    v
        Representative Corpus Intake Gate
```

The structured observation is upstream of corpus intake. It is neither an
intake approval nor an admitted corpus record. The existing approval,
redaction, lineage, human-label, retention, deletion, and bundle-digest gates
still apply after projection.

## State model

The source owner may reason about these immutable states:

```text
DRAFT -> SEALED -> SUPERSEDED
                 -> REVOKED
                 -> EXPIRED
```

- `draft`: incomplete and never eligible for projection;
- `sealed`: complete, digest-valid, and within its logical validity window;
- `superseded`: replaced by a successor observation;
- `revoked`: withdrawn by a source-owner revocation record;
- `expired`: outside its logical validity window.

Only `sealed` is structurally eligible for a pure Shadow projection. Eligibility
does not authorize export. State mutation in place is forbidden; supersession
or revocation must produce a new source-owner record and retain the prior
digest for audit.

## AegisStructuredObservationV1

The top-level object contains exactly:

```text
schema_id
schema_version
observation_ref
source_system_ref
lifecycle
receipt
assertions
observation_digest
```

Unknown fields are rejected. All JSON numbers are non-negative safe integers.
All strings are NFC. Identifiers are bounded pseudonymous ASCII tokens. UUIDs
are canonical lowercase. Digests use lowercase SHA-256.

### LifecycleV1

`lifecycle` binds:

- `status`;
- `valid_from_logical_time`;
- `valid_until_logical_time`;
- optional `supersedes_observation_digest`;
- optional `revocation_ref`.

For a `sealed` observation, both optional fields must be absent, validity must
contain the receipt logical time and every assertion production time, and the
external projection logical time must be within the half-open interval
`[valid_from_logical_time, valid_until_logical_time)`.

`superseded` requires a successor relationship outside this object;
`revoked` requires an external revocation record. Neither may be normalized
back to `sealed` by an exporter.

### IngressReceiptV1

The receipt contains exactly:

```text
event_id
run_id
turn_id
input_receipt_id
source_actor
actor_ref
content_ref
content_digest
correlation_ref
logical_time
schema_version
receipt_digest
```

The receipt carries no raw input. `content_ref` is a pseudonymous reference,
not a path, URL, query, or content excerpt. `content_digest` binds the content
at the Aegis controller boundary but does not make that content admissible.

Receipt digest domain:

```text
idr:aegis:structured-observation-receipt:v1
```

The digest covers every receipt field except `receipt_digest` using canonical
JSON and a NUL byte between the ASCII domain and the encoded object.

## Fact assertions

`assertions` must contain exactly five named assertions:

```text
semantic_roles
fast_path_facts
decision_facts
coordination_facts
host_projection
```

Every assertion contains:

```text
schema_version = 1
family
producer
source_receipt_digest
produced_at_logical_time
facts
assertion_digest
```

`source_receipt_digest` must equal the exact sealed receipt digest. Assertion
production time must not precede receipt logical time and must remain inside
the observation validity window.

### ProducerBindingV1

Each assertion producer binds:

- `producer_ref`: pseudonymous owner reference;
- `producer_kind`: exact producer class for the family;
- `producer_version`: immutable implementation/configuration version;
- `policy_ref`: exact policy revision governing the assertion;
- `derivation_class`: `deterministic`, `governed_model`, `human_reviewed`, or
  `mixed`;
- `idr_output_used`: always `false`.

Required family-to-producer mapping:

| Family | Required `producer_kind` |
| --- | --- |
| `semantic_roles` | `semantic_classifier` |
| `fast_path_facts` | `command_policy_classifier` |
| `decision_facts` | `decision_context_classifier` |
| `coordination_facts` | `coordination_planner` |
| `host_projection` | `aegis_host_decision_runtime` |

The five producers may share an organizational owner, but each assertion must
still carry its exact family-specific version and policy reference. Natural
language, model confidence, or a chosen action is not a substitute for the
required facts.

`idr_output_used = false` prevents circular evaluation. No assertion,
especially `host_projection`, may be reconstructed from an IDR assessment or
from a Shadow comparison result.

### Assertion digest

Each assertion uses the domain:

```text
idr:aegis:structured-observation-assertion:v1:<family>
```

The digest covers every assertion field except `assertion_digest`. Changing a
producer, policy, derivation class, receipt binding, logical time, or fact
invalidates the assertion.

## Exact fact payloads

The fact payloads reuse the frozen IDR V1.3 enum values and Boolean leaves
without adding interpretation:

- `semantic_roles`: `primary_semantic_role` and unique `semantic_roles`, with
  the primary role present in the set;
- `fast_path_facts`: the exact eight frozen leaves;
- `decision_facts`: the exact ten frozen leaves;
- `coordination_facts`: the exact eight frozen leaves;
- `host_projection`: `coordination_mode`, `action_posture`, and
  `next_run_state`.

`depends_on_human_model` remains a Boolean dependency flag. It cannot carry a
Human Model value or authorize personality, psychological, relationship, or
identity inference.

## Observation digest

The sealed observation digest uses:

```text
idr:aegis:structured-observation:v1
```

It covers lifecycle, receipt including its digest, all assertions including
their digests, source system, observation reference, and schema identity. Only
`observation_digest` is excluded.

An assertion substitution, warning-free rewrite, producer-version change,
policy change, receipt substitution, state change, or validity change invalidates
the observation digest.

## Pure projection to ShadowReplayEnvelopeV1

Projection is a field copy, not an assessment or inference:

| Shadow target | Observation source |
| --- | --- |
| `observation_ref` | top-level `observation_ref` |
| input IDs, actor, content, correlation, time, version | `receipt` |
| semantic roles | `assertions.semantic_roles.facts` |
| fast-path facts | `assertions.fast_path_facts.facts` |
| decision facts | `assertions.decision_facts.facts` |
| coordination facts | `assertions.coordination_facts.facts` |
| host projection | `assertions.host_projection.facts` |

The receipt-only `input_receipt_id` is lineage metadata and is not copied into
the frozen Shadow request. It remains transitively bound through the receipt,
assertion, observation, and later candidate-lineage digests.

Projection must reject:

- any lifecycle state other than `sealed`;
- projection outside the logical validity window;
- any receipt, assertion, or observation digest mismatch;
- producer-kind/family mismatch;
- `idr_output_used = true`;
- a missing or extra assertion family;
- any unknown, raw-content, authority, dispatch, provider, Human Model,
  personality, vector, or graph-memory field.

The conformance pack computes a domain-separated projection digest for the
golden vector using:

```text
idr:aegis:structured-observation-projection:v1
```

That evaluation digest is not a production IDR contract field.

## Prohibited content

The object must never contain:

- raw input, prompt, conversation, message, output, error, or goal text;
- email addresses, query strings, URLs, filesystem paths, credentials, secrets,
  tokens, or provider payloads;
- Actions, Authorizations, Authority Contexts, Execution Permits, dispatch
  nonces, production state, or effect results;
- Human Model payloads, personality or psychological inference;
- embeddings, vectors, graph memory, or semantic retrieval payloads.

The object is not a convenient redacted view of Journal JSON. It must be
constructed from typed source-owner outputs before any raw or arbitrary JSON
surface is selected for an offline sink.

## Responsibility boundary

- Aegis Rust may eventually own receipt creation, typed fact-producer outputs,
  digest sealing, lifecycle, and the pure projection, but no implementation is
  authorized by this document.
- TypeScript may validate or transport the structured observation after a
  future implementation gate; it may not invent facts or recompute IDR
  decisions.
- Python may evaluate admitted projected corpora; it may not source production
  data, change facts, weaken digest checks, or admit an invalid observation.
- IDR Rust remains the only semantic assessment kernel and is not changed by
  this integration schema.

## What this specification resolves

This phase resolves only the design shape of:

```text
VERSIONED_STRUCTURED_SOURCE_SCHEMA = SPECIFIED
PURE_SHADOW_FIELD_MAPPING = SPECIFIED
FACT_PRODUCER_BINDING_SHAPE = SPECIFIED
```

It does not establish that any producer exists or is authoritative.

## Remaining blockers

```text
AEGIS_STRUCTURED_OBSERVATION_PRODUCER = NOT IMPLEMENTED
AUTHORITATIVE_FACT_PRODUCERS = NOT APPROVED
SOURCE_SPECIFIC_EXPORT_APPROVAL = UNRESOLVED
CONSENT_OR_LAWFUL_BASIS = UNRESOLVED
TRUSTED_EXPORT_AND_VERIFICATION_CLOCK = UNRESOLVED
PRODUCT_ALLOWLIST_AND_REDACTION_PROOF = UNRESOLVED
RETENTION_EXECUTOR = UNRESOLVED
VERIFIABLE_DELETION_RECEIPT = UNRESOLVED
INDEPENDENT_HUMAN_LABEL_WORKFLOW = UNRESOLVED
DEDICATED_OFFLINE_SINK = UNRESOLVED
SECURITY_AND_PRIVACY_REVIEW = UNRESOLVED
REPRESENTATIVE_EXPORTER = BLOCKED
REAL_AEGIS_DATA_ACCESS = NOT AUTHORIZED
```

## Explicit exclusions

- no Aegis or IDR V1.3 production code;
- no exporter, passive capture, or live mirror;
- no database access or migration;
- no real user data;
- no new dependency;
- no Setoka, personality inference, Human Model feature, vector database,
  graph memory, or bitemporal semantic retrieval.
