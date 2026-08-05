# IDR × Aegis Life Source Data-Flow Review V1

Status: **Reviewed / Representative Exporter Blocked**

Date: 2026-08-05

## Decision

The reviewed Aegis Life source does not contain an admissible production source
for `ShadowReplayEnvelopeV1`. A production or representative-data exporter is
therefore **BLOCKED**.

The tracked source does expose a small ingress metadata candidate:
`input_receipt_id`, the literal source class `user`, input length, and later a
goal correlation carrying the same receipt identifier. That candidate is not a
qualified evidence source. It has no approved export purpose, content digest,
turn/run identity, actor identity, IDR semantic facts, host projection, trusted
export clock, retention executor, or deletion receipt.

No Aegis database was opened, no journal or persistence file was read, and no
real user record was accessed. This review authorizes synthetic controls only.

## Evidence basis

The review is pinned to the tracked Aegis Life source at:

```text
Repository: Aegis Life
Commit: 381d14fb1dccfeae691286a39579535ef59f7b4e
Tree: da76b01653bb194e4f192e5768b32a38fd3b2173
Scope: git-tracked source and documentation only
Excluded: working-tree modifications, untracked files, databases, journals,
          persistence snapshots, credentials, network services, and user data
```

The machine-readable evidence inventory, file digests, all 42 allowlisted leaf
paths, and unresolved gates are recorded in
`contracts/integrations/aegis-life/representative-corpus-intake/v1/aegis-source-readiness-review-v1.json`.

## Observed data flow

```text
CLI --submit-input <free-form text>
        |
        v
Kernel::submit_input(String)
        |
        v
SystemEvent::UserInput { content }             PROHIBITED RAW SOURCE
        |
        +--> Journal "user input received"
        |      source = "user"
        |      input_len
        |      input_receipt_id                 CANDIDATE METADATA ONLY
        |
        +--> Goal::new(content)
               |
               +--> identity / will / planning
               |
               +--> goal-commit JournalEntry
               |      description includes goal text
               |      data may include input_receipt_id
               |
               +--> LifecycleSnapshot
               |      current_goal_description
               |
               `--> KernelPersistenceState
                      goal descriptions and memory records

Separate, not wired to the product ingress above:

InputReceipt + DispatchContextV1              TYPE-ONLY / PROCESS-LOCAL
TraceEventRecord                              CALLER-PROVIDED / TYPE-ONLY
ProductionAuditExportV1                       UNRELATED OPERATIONS EXPORT
ShadowReplayEnvelopeV1                        PREBUILT OR SYNTHETIC INPUT ONLY
```

There is no tracked bridge from `SystemEvent::UserInput` or the ingress Journal
record to an `InteractionAssessmentRequestV1`. The frozen adapter accepts an
already constructed request and assesses it; it does not source or derive that
request from Aegis product events.

## Source-surface findings

### 1. Product ingress and Journal metadata

`crates/kernel/src/runtime.rs:1607-1609` publishes the complete input string as
`SystemEvent::UserInput`. At `runtime.rs:1940-1963`, the routing path generates a
new UUID and writes only `source`, `input_len`, and `input_receipt_id` to the
initial Journal entry before constructing a `Goal` from the raw content.

The receipt UUID is a useful future correlation candidate, but the current
record is not versioned as an export source and has no input digest or trusted
logical time. `input_len` is not on the IDR allowlist and must never be copied
into a replay candidate.

At `runtime.rs:1160-1191`, a later goal-commit Journal record may carry the same
`input_receipt_id`, but its human-readable description includes the full goal
description. The JSONL Journal therefore cannot be exported wholesale. Any
future source projector must select typed leaves before data leaves the Aegis
controller boundary and must fail closed on unrecognized entry shapes.

### 2. Raw and continuity state

`crates/core-protocol/src/types.rs:42-105` defines product events containing raw
user input and output strings. `crates/state-store/src/lib.rs:161-190` stores a
`current_goal_description`, while `crates/kernel/src/persistence.rs:14-64`
persists goal descriptions, terminal/queue state, and memory records.

These surfaces contain or may contain prohibited free-form content. They are
not export candidates. A future exporter must not deserialize them and then
attempt best-effort redaction; it must consume a separate, already structured,
allowlisted projection.

### 3. Journal persistence

`crates/core-protocol/src/services.rs:116-127` permits free-form Journal
descriptions and arbitrary JSON data. `crates/journal/src/lib.rs:44-91` appends
entries and obtains wall time from `chrono::Utc::now()`. The JSONL persistence
implementation at `crates/journal/src/persistence.rs:50-145` provides save,
load, and append, but no bounded retention, selective deletion, compaction, or
deletion-receipt interface.

Consequently, Journal timestamps are observations, not the externally injected
trusted current time required by the intake gate, and Journal file deletion
cannot be treated as a verified per-candidate deletion receipt.

### 4. Dispatch-context protocol

`crates/core-protocol/src/dispatch_context.rs:1-4` explicitly states that the
receipt-rooted correlation model is not wired into kernel execution, adapters,
authorization, journaling, or replay. Its `InputReceipt` has a receipt UUID,
logical order, input digest, origin class, and schema version
(`dispatch_context.rs:51-102`), but those fields are not created by the reviewed
product ingress.

The repository in `crates/state-store/src/dispatch_context.rs:57-88` is
process-local and performs no external I/O. These types are useful design
precedent only. Reusing their names or sample values cannot establish lineage
to an actual Aegis input.

### 5. Trace model

`crates/core-protocol/src/trace.rs:322-405` contains caller-provided event IDs,
ticks, ordering, optional references, and privacy classification. The fields
are not bound to `Kernel::submit_input`, and the default privacy classification
is `InternalOnly`. Trace diagnostics are explicitly non-authoritative.

Trace values therefore cannot supply IDR event identity, logical time, actor,
semantic facts, or host projection without a new approved source owner and an
exact binding to the ingress receipt.

### 6. Existing production audit export

`crates/state-store/src/production_operations/audit_export.rs:8-68` exports
bounded operational lease, claim, dispatch, migration, and semantic-family
metadata. Its `operator_authorization_ref` is checked only for non-emptiness.
It neither exports input assessment facts nor proves the representative offline
export approval required by the intake specification.

The production runbook places copy and retention responsibility on the
operator. That operational policy is not a candidate-level retention executor
or deletion-receipt mechanism. The existing audit export must not be expanded
or repurposed under this phase.

### 7. IDR adapter and shadow validator

`crates/idr-aegis-adapter/src/lib.rs:75-97` maps references or assesses an
already constructed request. It does not construct IDR facts from product
state. `crates/idr-aegis-shadow-validator/src/lib.rs:25-45` accepts a prebuilt
replay envelope, while lines 75-103 make its no-authority/no-dispatch/no-state-
mutation boundary static.

This is correct for synthetic offline validation but does not supply a
representative corpus source.

## Complete allowlisted-field mapping

The following table summarizes the complete leaf mapping. The companion JSON
enumerates every leaf separately and the verifier rejects omissions or
duplicates.

| IDR target | Reviewed Aegis source | Status | Reason |
| --- | --- | --- | --- |
| `observation_ref` | ingress `input_receipt_id` | Candidate only | Stable pseudonymous reference semantics and source admission are not defined |
| `input.event_id` | ingress `input_receipt_id` | Candidate only | A receipt UUID is not yet an IDR event identity |
| `input.source_actor` | literal Journal `source = user` | Candidate only | Only one coarse source literal exists; no source-owner contract |
| `input.correlation_ref` | receipt repeated in goal-commit data | Candidate only | Join exists in selected records, but source record contains raw description |
| `input.run_id`, `input.turn_id` | none | Not found | No product-ingress run/turn binding |
| `input.actor_ref` | none | Not found | No pseudonymous actor identity bound to receipt |
| `input.primary_semantic_role`, `input.semantic_roles` | none | Not found | Aegis does not emit the IDR role taxonomy at ingress |
| `input.content_ref`, `input.content_digest` | none on product path | Not found | Dispatch-context digest is type-only and not wired |
| `input.logical_time`, `input.schema_version` | none on product path | Not found | Journal wall time and lifecycle tick are not bound trusted sources |
| all 8 `fast_path_facts` leaves | none | Not found | Planner/Will outcomes cannot be reverse-inferred as IDR facts |
| all 10 `decision_facts` leaves | none | Not found | No authoritative fact producer or evidence binding |
| all 8 `coordination_facts` leaves | none | Not found | No exact IDR coordination-fact projection |
| all 3 `host_projection` leaves | only shadow fixtures/prebuilt input | Synthetic only | No independently existing Aegis projection with matching semantics |

No leaf is `ADMITTED`. Similar names, UUID shape compatibility, or deterministic
derivability are insufficient without exact source semantics and lineage.

## Authorization, provenance, retention, and deletion

| Gate | Finding | Result |
| --- | --- | --- |
| Source owner and schema | `AegisStructuredObservationV1` and five producer responsibility profiles are specified, but no Aegis source owner or producer implements them | Specified, not implemented |
| Export approval | No verified approval object allowing representative offline export | Blocked |
| Consent or lawful basis | Coordination consent types are unrelated to corpus export | Blocked |
| Provenance and lineage | No product receipt-to-IDR-candidate lineage digest | Blocked |
| Allowlist and redaction | No fail-closed product-side projector; raw text coexists in source records | Blocked |
| Trusted time | Wall-clock calls exist, but no externally injected trusted export/verification clock | Blocked |
| Retention | Journal and continuity persistence have no candidate-scoped retention executor | Blocked |
| Deletion | Generic in-memory removal and file overwrite are not deletion receipts | Blocked |
| Human labels | No independent reviewer workflow bound to candidate digest | Blocked |
| Dedicated sink | No approved offline evaluation sink or access-control boundary | Blocked |

## Required design before exporter implementation

An exporter implementation proposal remains unauthorized until all of the
following are separately approved:

1. A versioned product-ingress observation object containing only structured,
   allowlisted leaves and a stable receipt binding.
2. An authoritative producer for each IDR fact family; facts may not be inferred
   backward from the action chosen, goal outcome, or IDR assessment.
3. Exact run, turn, event, actor, content-reference, content-digest, correlation,
   schema-version, and logical-time semantics.
4. An independently existing Aegis host projection with a reviewed mapping to
   the three IDR projection enums.
5. A source-specific approval proof, purpose, actor/tenant scope, and revocation
   check performed at an externally supplied trusted current time.
6. A product-side fail-closed allowlist projector and redaction proof that run
   before data enters a dedicated offline sink.
7. Domain-separated source-export, lineage, candidate, and bundle digests.
8. A retention executor and verifiable deletion receipt covering the raw source,
   projected export, labels, evaluation bundle, backups, and failure remnants.
9. Independent human-label workflow bound to the exact candidate digest.
10. A new security/privacy audit proving failure isolation, least privilege,
    bounded volume, secret exclusion, and no product-path dependency.

Follow-on specification work defines item 1's object shape and the pure 42-leaf
mapping in
[`IDR-AEGIS-STRUCTURED-OBSERVATION-SPEC.md`](IDR-AEGIS-STRUCTURED-OBSERVATION-SPEC.md).
Item 2's five producer responsibilities, typed-input allowlists, proof
requirements, and fail-closed implementation gate are defined in
[`IDR-AEGIS-PRODUCER-TRUST-ADMISSION-SPEC.md`](IDR-AEGIS-PRODUCER-TRUST-ADMISSION-SPEC.md).
Item 3's identity, actor, content, correlation, digest, ingress logical-time,
and receipt-source binding semantics are defined in
[`IDR-AEGIS-INGRESS-RECEIPT-TRUST-SPEC.md`](IDR-AEGIS-INGRESS-RECEIPT-TRUST-SPEC.md).
All five producer profiles and the receipt source remain `NOT_IMPLEMENTED`.
No reviewed Aegis component implements these requirements, so this source
review's exporter and real-data decisions remain unchanged.

Satisfying this list would authorize a new implementation review, not the
export itself. It must not modify the frozen IDR V1.3 adapter or silently turn
the current shadow validator into a product component.

## Final state

```text
SOURCE_DATA_FLOW_REVIEW = COMPLETE
VERSIONED_STRUCTURED_SOURCE_SCHEMA = SPECIFIED_NOT_IMPLEMENTED
AUTHORITATIVE_FACT_PRODUCERS = RESPONSIBILITIES_SPECIFIED_NOT_IMPLEMENTED
EXACT_INGRESS_IDENTITY_AND_TIME_SEMANTICS = SPECIFIED_NOT_IMPLEMENTED
REPRESENTATIVE_EXPORTER = BLOCKED
REAL_AEGIS_DATA_ACCESS = NOT AUTHORIZED
PASSIVE_CAPTURE = NOT AUTHORIZED
LIVE_MIRROR = NOT AUTHORIZED
AUTHORIZED_CORPUS_SOURCE = SYNTHETIC_CONTROL_ONLY
AEGIS_PRODUCTION_CODE_CHANGED = NO
IDR_V1_3_PRODUCTION_CODE_CHANGED = NO
```
