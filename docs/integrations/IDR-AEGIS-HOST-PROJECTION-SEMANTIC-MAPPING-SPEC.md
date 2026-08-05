# IDR × Aegis Life Host Projection Semantic Mapping Specification V1

Status: **Specification Only / Host Projection Source Not Implemented**

Date: 2026-08-05

## Purpose

This specification defines an independently produced Aegis host-decision
snapshot and a total, deterministic mapping from its three source enums to the
three frozen IDR V1.3 Host Projection enums:

```text
host_coordination_strategy -> coordination_mode
host_action_state          -> action_posture
host_run_phase             -> next_run_state
```

It closes only the semantic-design portion of the Host Projection gate. It
does not implement a product source, modify the Aegis runtime, approve a host
owner, admit a projection, or authorize real-data export.

## Current evidence

The reviewed Aegis source contains Host Projection values only in synthetic
fixtures and prebuilt Shadow replay inputs. The Shadow validator compares those
values with IDR output but does not produce an independent product result.

Those fixtures remain evaluation controls, not production provenance. An IDR
assessment, Shadow divergence, chosen action, execution outcome, Journal entry,
or synthetic matrix must never become the source of a Host Projection.

## Boundary

```text
sealed ingress receipt + typed host planning/runtime state
                         |
                         v
        approved Aegis host-decision owner
                         |
                         v
            AegisHostDecisionSnapshotV1
                         |
               pure enum mapping only
                         |
                         v
                  HostProjectionV1
                         |
                         v
                  IDR Shadow comparison
```

The source snapshot must be sealed before the IDR assessment starts. The
projection may later be compared with IDR, but comparison output cannot flow
back into the source snapshot or mapping.

## AegisHostProjectionMappingProfileV1

The immutable profile contains exactly:

```text
schema_id
schema_version
mapping_ref
target_projection_schema
source_review_pin
lifecycle
owner
source_snapshot_contract
coordination_mapping
action_mapping
run_state_mapping
consistency_rules
independence_policy
implementation_gate
profile_digest
```

Unknown fields are rejected. Strings are NFC, numbers are non-negative safe
integers, and digests use lowercase SHA-256.

Any change to the source owner, source or target enum, mapping entry,
consistency rule, independence rule, source pin, target schema digest, or gate
creates a new profile and digest.

## Source owner

The proposed source owner is:

```text
owner_kind = aegis_host_decision_runtime
owner_ref = component:aegis-host-decision-runtime
trust_state = NOT_IMPLEMENTED
```

The owner must consume only the sealed ingress receipt, its verified
`ReceiptSourceBindingV1`, the four upstream sealed fact assertions, and typed
Aegis host planning/runtime state. It may not consume IDR output, Shadow
results, evaluation labels, chosen actions, execution results, or goal outcomes.

The existing IDR adapter and Shadow validator are explicitly prohibited owners.

## AegisHostDecisionSnapshotV1

The source snapshot contains exactly:

```text
schema_id
schema_version
snapshot_ref
receipt_digest
receipt_source_binding_digest
captured_at_logical_time
owner_ref
owner_version
policy_ref
host_coordination_strategy
host_action_state
host_run_phase
idr_output_used
shadow_result_used
snapshot_digest
```

`receipt_digest` and `receipt_source_binding_digest` bind the exact ingress
event and approved source contract. `captured_at_logical_time` must not precede
the receipt and must precede the start of IDR assessment.

Owner version and policy reference are immutable implementation identifiers.
Natural-language prompts, model names, or caller labels are insufficient.

Both circularity fields must be `false`:

```text
idr_output_used = false
shadow_result_used = false
```

Snapshot digest domain:

```text
idr:aegis:host-decision-snapshot:v1
```

The digest covers every snapshot field except `snapshot_digest` using
canonical JSON and a NUL byte between the ASCII domain and encoded object.

## Coordination mapping

The source and target vocabularies are intentionally different:

| Aegis source strategy | IDR `coordination_mode` |
| --- | --- |
| `response_only` | `respond_only` |
| `action_before_response` | `act_then_respond` |
| `response_before_action` | `respond_then_act` |
| `response_then_authorization_gate_then_action` | `respond_then_confirm_then_act` |
| `parallel_response_and_action` | `parallel` |
| `acknowledge_before_background_run` | `acknowledge_then_run` |
| `stream_progress_before_final` | `stream_progress_then_final` |

Every source and target value appears exactly once. Fallbacks, string
normalization, fuzzy matching, and unknown-value coercion are forbidden.

## Action mapping

| Aegis source action state | IDR `action_posture` |
| --- | --- |
| `no_action` | `none` |
| `action_blocked` | `blocked` |
| `action_ready` | `planned` |
| `action_waiting_authorization` | `waiting_authorization` |

`action_ready` means the host independently has a complete action plan; it does
not grant execution authority. `action_waiting_authorization` requires an
authorization gate but carries no Authorization or Execution Permit.

## Run-state mapping

| Aegis source run phase | IDR `next_run_state` |
| --- | --- |
| `queued` | `pending` |
| `executing` | `running` |
| `waiting_user_input` | `waiting_input` |
| `waiting_action_authorization` | `waiting_authorization` |
| `waiting_external_dependency` | `waiting_dependency` |
| `completed` | `succeeded` |
| `declined` | `rejected` |
| `execution_failed` | `failed` |
| `user_cancelled` | `cancelled` |
| `deadline_exceeded` | `timed_out` |
| `superseded_or_stale` | `invalidated` |

This is a pure mapping of an already existing host phase. The mapper does not
run the action, inspect a provider receipt, or decide whether execution
succeeded.

## Cross-field consistency

Before mapping, the snapshot must satisfy all of these rules:

1. `no_action` requires `response_only`;
2. a non-`response_only` strategy requires an action state other than
   `no_action`;
3. `action_waiting_authorization` requires
   `waiting_action_authorization`;
4. `waiting_action_authorization` requires
   `action_waiting_authorization`;
5. `action_blocked` requires `declined`;
6. `declined` requires `action_blocked`.

These rules prevent the mapper from hiding contradictory states. All other
combinations are mapped independently; downstream IDR assessment may still
disagree and record a Shadow divergence.

Invalid, missing, unknown, contradictory, or unbound snapshots are rejected.
The mapper never repairs them.

## Independence policy

The profile permanently forbids:

```text
idr_assessment
idr_coordination_mode
idr_action_posture
idr_next_run_state
shadow_comparison_result
shadow_divergence
evaluation_label
chosen_action
execution_result
goal_outcome
journal_record
synthetic_fixture_result
```

The snapshot must be captured before IDR assessment and the pure mapping must
complete without network, provider, database, Journal, model, or IDR calls.

The `host_projection` assertion later binds the snapshot digest, receipt digest,
owner version, and policy through its producer metadata and assertion digest.
This phase specifies that requirement but does not change the structured
observation schema or implement the binding.

## Projection algorithm

For a valid sealed snapshot:

1. verify the target schema digest and mapping-profile digest;
2. verify receipt and ReceiptSourceBinding digests externally;
3. verify snapshot digest, owner identity, policy revision, and logical order;
4. reject prohibited/circular inputs and inconsistent tuples;
5. perform exact lookup in each of the three mapping tables;
6. emit only `coordination_mode`, `action_posture`, and `next_run_state`;
7. bind the result into the host-projection assertion before IDR evaluation.

The algorithm has no default branch. A source enum added in a later Aegis
version must fail closed until a new mapping profile is approved.

## Conformance coverage

The synthetic vector set must cover:

```text
7 / 7 source coordination strategies
7 / 7 target coordination modes
4 / 4 source action states
4 / 4 target action postures
11 / 11 source run phases
11 / 11 target run states
```

Coverage proves table totality only. It does not prove that the Aegis product
can produce any vector.

## Implementation gate

The profile contains these exact checks:

```text
host_mapping_profile_locked
host_source_owner_approved
host_snapshot_implementation_registered
pre_idr_capture_order_verified
receipt_source_binding_verified
source_enum_semantics_reviewed
coordination_mapping_coverage_verified
action_mapping_coverage_verified
run_state_mapping_coverage_verified
cross_field_consistency_tested
no_circular_derivation_verified
deterministic_mapper_verified
failure_isolation_verified
security_privacy_review_passed
cross_language_conformance_passed
```

Only `host_mapping_profile_locked` is satisfied in this phase. Every other
check is unsatisfied and the decision remains `BLOCKED`.

## Hash binding

Mapping-profile digest domain:

```text
idr:aegis:host-projection-mapping-profile:v1
```

Implementation-gate digest domain:

```text
idr:aegis:host-projection-implementation-gate:v1
```

The profile digest covers every profile field including source/target bindings,
all mapping entries, consistency and independence rules, and the gate digest.
Only `profile_digest` is excluded.

## Responsibility boundary

- Aegis Rust may eventually own the typed source snapshot, consistency checks,
  pure mapper, and assertion binding after separate implementation approval.
- TypeScript may validate or transport an admitted snapshot/projection. It may
  not infer, repair, or recompute source states.
- Python may evaluate mapping and divergence coverage using synthetic or
  admitted corpora. It may not become a source owner or feed evaluation output
  back into the projection.
- IDR Rust remains the only assessment kernel and is unchanged.

## Current result

```text
HOST_DECISION_SNAPSHOT_SCHEMA = SPECIFIED
HOST_PROJECTION_SEMANTIC_MAPPING = SPECIFIED_NOT_IMPLEMENTED
HOST_PROJECTION_SOURCE_OWNER = NOT_APPROVED
HOST_PROJECTION_IMPLEMENTATION_GATE = BLOCKED
HOST_PROJECTION_PRODUCT_SOURCE = NOT_IMPLEMENTED
REPRESENTATIVE_EXPORTER = BLOCKED
REAL_AEGIS_DATA_ACCESS = NOT_AUTHORIZED
```

## Explicit exclusions

- no Aegis or IDR V1.3 production-code change;
- no host snapshot producer, runtime mapper, exporter, capture, or live mirror;
- no authority, dispatch, provider, execution, database, or migration change;
- no real user data or untracked Aegis source;
- no new dependency;
- no Setoka, personality inference, Human Model feature, vector database,
  graph memory, or bitemporal semantic retrieval.
