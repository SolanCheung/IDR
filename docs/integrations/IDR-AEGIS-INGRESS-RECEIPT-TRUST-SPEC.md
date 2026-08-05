# IDR × Aegis Life Ingress Receipt Trust Specification V1

Status: **Specification Only / Receipt Source Not Implemented**

Date: 2026-08-05

## Purpose

This specification defines the exact source ownership and generation semantics
required for a future Aegis `IngressReceiptV1`. It covers event, run, turn,
receipt, actor, content, correlation, schema-version, digest, and logical-time
values before any fact producer or IDR assessment runs.

It does not implement a receipt source, approve an Aegis owner, read real data,
or authorize an exporter. Existing Journal timestamps, trace records,
process-local dispatch-context examples, lifecycle ticks, and caller-supplied
UUIDs remain non-authoritative.

## Boundary

```text
authenticated ingress + typed host context + exact input bytes
                         |
                         v
            approved ingress receipt owner
                         |
          +--------------+--------------+
          |              |              |
          v              v              v
   identity rules   content binding   logical clock
          |              |              |
          +--------------+--------------+
                         v
                 IngressReceiptV1
                         |
                         | external ReceiptSourceBindingV1
                         v
           five receipt-bound fact assertions
```

Receipt creation is upstream of every fact producer. A fact producer may
consume a sealed receipt but may not create, repair, normalize, or replace its
identity, actor, content, correlation, or logical-time fields.

## Trust statement

These claims are distinct:

```text
receipt schema is valid
    != receipt source exists
    != receipt source is approved
    != receipt is bound to an approved source contract
    != observation is admissible
    != export is authorized
```

The source contract described here is an immutable verification profile. A
future runtime receipt must be accompanied by a separately sealed
`ReceiptSourceBindingV1` before any source assertion can be trusted.

## AegisIngressReceiptSourceContractV1

The contract contains exactly:

```text
schema_id
schema_version
contract_ref
target_receipt_schema
source_review_pin
lifecycle
owner
identity_semantics
actor_semantics
content_semantics
correlation_semantics
logical_time_semantics
receipt_digest_semantics
receipt_source_binding
implementation_gate
contract_digest
```

Unknown fields are rejected. All strings are NFC. All numbers are non-negative
safe integers. Digests use lowercase SHA-256 and Git object identifiers use
lowercase hexadecimal.

The contract is immutable. Any owner, source pin, namespace, generation rule,
clock epoch, digest preimage, failure rule, or implementation-gate change
creates a new contract digest.

## Target receipt binding

`target_receipt_schema` binds the exact structured-observation schema path and
SHA-256 digest, receipt definition name, schema version, and receipt digest
domain. Compatibility inference is forbidden: a source admitted for one schema
digest is not admitted for another.

The target receipt contains:

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

## Source review pin

The contract is pinned to Aegis Life commit
`381d14fb1dccfeae691286a39579535ef59f7b4e` and tree
`da76b01653bb194e4f192e5768b32a38fd3b2173`.

The reviewed scope is tracked source and documentation only. Working-tree
changes, untracked files, databases, journals, persistence snapshots,
credentials, network services, and user data are excluded. A different source
tree requires a new review and contract.

## Owner boundary

The proposed owner boundary is a typed product ingress controller, represented
by:

```text
owner_kind = product_ingress_controller
owner_ref = component:aegis-ingress-controller
trust_state = NOT_IMPLEMENTED
```

The CLI, Journal, state store, trace layer, IDR adapter, Shadow validator,
exporter, model, and provider are not receipt owners. Delegating one field to
an unapproved caller is forbidden.

The owner must create the receipt atomically before publishing the accepted
ingress event. A partial receipt must never enter a queue, Journal, fact
producer, or observation.

## Identity semantics

Four UUID fields use separate namespaces and must be mutually distinct:

| Field | Source | Cardinality | Reuse |
| --- | --- | --- | --- |
| `event_id` | cryptographically secure UUID v4 generated for the accepted ingress event | one per event | never |
| `run_id` | typed host run context created before ingress | one per host run | only within that run |
| `turn_id` | typed interaction-turn context created before ingress | one per turn | only within that turn |
| `input_receipt_id` | cryptographically secure UUID v4 generated for the sealed receipt | one per receipt | never |

The run identifier cannot be derived from goal text, a queue item, lifecycle
state, or a later dispatch. The turn identifier cannot be reconstructed from a
message index, Journal order, or wall time. Missing typed run or turn context
causes `REJECT_RECEIPT`.

All UUIDs must be canonical lowercase. Collision detection runs before event
publication. Collision, namespace reuse, or generation failure causes
`REJECT_RECEIPT`; retry may allocate a new value before anything is published.
Published identities are immutable and cannot be repaired in place.

## Actor semantics

`source_actor` is selected from the frozen IDR V1.3 taxonomy using an
authenticated typed origin classification. It is not copied from a free-form
caller string or inferred from content.

`actor_ref` is a tenant-scoped pseudonymous reference produced before receipt
sealing. Email, phone, account name, device identifier, credential, network
address, filesystem path, or other direct identifier is forbidden. Failure to
resolve an admitted pseudonym causes `REJECT_RECEIPT`.

Pseudonym resolution and rotation policy are separate governance concerns. A
receipt may bind a pseudonym but does not grant permission to resolve it.

## Content semantics

`content_ref` is a fresh opaque reference in the `aegis:content` namespace. It
is not a URL, path, query, excerpt, prompt, or database locator.

`content_digest` is SHA-256 over the exact UTF-8 bytes accepted at the ingress
controller boundary before downstream parsing, normalization, model calls, or
goal construction. No Unicode normalization, whitespace rewriting, newline
conversion, decoding/re-encoding, redaction, or canonical JSON transformation
is applied to the digest preimage.

The digest proves byte equality only. It does not make content admissible and
does not authorize raw-content retention. The source contract requires:

```text
raw_content_in_receipt = forbidden
raw_content_persistence = not_authorized
raw_content_export = forbidden
raw_content_telemetry = forbidden
```

If the controller cannot hash the exact accepted bytes, it must reject the
receipt. Hashing a later Goal, Journal description, provider payload, or
redacted copy is forbidden.

## Correlation semantics

`correlation_ref` is a non-semantic grouping reference in the
`aegis:correlation` namespace. It comes from an authenticated typed upstream
context. If no group exists, the ingress owner creates a fresh per-input
reference.

Reuse is allowed only when the upstream context explicitly declares that two
inputs belong to the same group. Correlation must not be derived from goal
text, semantic similarity, timestamps, actor identity, chosen action, or IDR
output. It carries no authority and cannot substitute for run or turn identity.

## Logical-time semantics

`logical_time` comes from an externally injected, source-system-scoped,
monotonic logical clock. The clock contract binds:

```text
clock_ref = clock:aegis-ingress-logical
epoch_ref = epoch:aegis-ingress-v1
sequence_rule = strictly_increasing_per_source_system
allocation_stage = before_event_publish
rollback_rule = reject_and_require_new_approved_epoch
restart_rule = restore_verified_anchor_or_reject
exhaustion_rule = reject_and_require_new_approved_epoch
```

Wall time, Journal `Utc::now()`, trace ticks, process-local counters without a
verified restart anchor, and lifecycle order are not authoritative clocks.

Clock allocation and identity/content sealing must be one atomic acceptance
operation. A reserved logical time that never publishes may be skipped, but a
published value may never be reused. Rollback, duplicate, out-of-order,
unverifiable restart, or safe-integer exhaustion causes `REJECT_RECEIPT`.

This is the ingress ordering clock. It is not the trusted current time required
for export approval, retention expiry, revocation checks, or corpus
verification. The separate source-export approval specification now defines
that clock evidence shape and verification semantics, but no production clock
or verifier implements them; the gate is `SPECIFIED_NOT_IMPLEMENTED`.

## Receipt digest semantics

The receipt digest domain remains:

```text
idr:aegis:structured-observation-receipt:v1
```

The digest covers all eleven receipt fields except `receipt_digest`, using
canonical JSON, UTF-8 encoding, and one NUL byte between the ASCII domain and
the encoded object. It does not cover raw content.

Changing an identity, actor, reference, content digest, correlation, logical
time, or schema version invalidates the receipt digest.

## ReceiptSourceBindingV1 requirement

The current receipt shape does not embed its source-contract or clock-epoch
digest. Source admission therefore requires a separate immutable binding with:

```text
schema_id
schema_version
binding_ref
receipt_digest
source_contract_digest
source_review_tree
owner_ref
clock_ref
epoch_ref
bound_at_logical_time
binding_digest
```

Binding digest domain:

```text
idr:aegis:receipt-source-binding:v1
```

The binding must be created by the approved receipt owner in the same atomic
acceptance operation and verified before any fact assertion is accepted. A
missing, mismatched, expired, superseded, suspended, or revoked contract
binding causes rejection.

This phase specifies the binding shape only. It does not add the binding to the
structured observation, implement storage, or claim that an admitted binding
exists. A future implementation proposal must decide how the observation and
binding travel together without weakening the frozen 42-leaf projection.

## Implementation gate

The contract contains these exact checks:

```text
receipt_source_contract_locked
source_owner_approved
receipt_implementation_registered
ingress_path_wired
cryptographic_uuid_generation_verified
run_context_binding_verified
turn_context_binding_verified
actor_pseudonymization_verified
exact_content_digest_verified
correlation_semantics_verified
monotonic_clock_injection_verified
rollback_restart_recovery_verified
collision_and_atomicity_tested
receipt_source_binding_implemented
security_privacy_review_passed
cross_language_conformance_passed
```

Only `receipt_source_contract_locked` is satisfied in this phase. All other
checks are unsatisfied. Any unsatisfied, expired, mismatched, suspended, or
revoked check keeps the decision `BLOCKED`.

Implementation admission would authorize only a new source review. It would
not approve fact producers, observation admission, data export, retention,
labels, or an offline sink.

## Hash binding

Implementation-gate digest domain:

```text
idr:aegis:ingress-receipt-implementation-gate:v1
```

Source-contract digest domain:

```text
idr:aegis:ingress-receipt-source-contract:v1
```

The contract digest covers every field, including the target-schema digest,
source pin, owner state, all semantics, binding requirement, and gate digest.
Only `contract_digest` is excluded.

## Responsibility boundary

- Aegis Rust may eventually own atomic receipt creation, typed context input,
  exact-byte hashing, clock allocation, source binding, and digest sealing
  after a separate implementation approval.
- TypeScript may validate or transport a future admitted receipt and source
  binding. It may not generate identities, repair time, hash transformed
  content, or infer actor/correlation semantics.
- Python may test synthetic or admitted receipt corpora. It may not source
  production input, reassign identity, or treat evaluation time as ingress
  logical time.
- IDR Rust consumes the resulting structured facts and remains unchanged.

## Current result

```text
INGRESS_RECEIPT_SOURCE_CONTRACT = SPECIFIED
EXACT_INGRESS_IDENTITY_AND_TIME_SEMANTICS = SPECIFIED_NOT_IMPLEMENTED
INGRESS_RECEIPT_SOURCE_OWNER = NOT_APPROVED
INGRESS_RECEIPT_IMPLEMENTATION_GATE = BLOCKED
RECEIPT_SOURCE_BINDING = SPECIFIED_NOT_IMPLEMENTED
TRUSTED_EXPORT_AND_VERIFICATION_CLOCK = SPECIFIED_NOT_IMPLEMENTED
REPRESENTATIVE_EXPORTER = BLOCKED
REAL_AEGIS_DATA_ACCESS = NOT_AUTHORIZED
```

## Explicit exclusions

- no Aegis or IDR V1.3 production-code change;
- no receipt producer, clock service, exporter, passive capture, or live mirror;
- no database, Journal, trace, persistence, or real-user-data read;
- no migration or new dependency;
- no Setoka, personality inference, Human Model feature, vector database,
  graph memory, or bitemporal semantic retrieval.
