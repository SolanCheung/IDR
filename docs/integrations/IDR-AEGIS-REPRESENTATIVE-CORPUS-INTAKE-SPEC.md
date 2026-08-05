# IDR × Aegis Life Representative Corpus Intake Specification V1

Status: **Specification Only / No Data Export Authorization**

Date: 2026-08-05

## Purpose

This specification defines the governance boundary that must exist before any
representative Aegis Life sample may enter IDR offline shadow evaluation. It
does not authorize reading production records, exporting user data, passive
capture, live mirroring, or changing the Aegis product path.

The initial conformance fixture is synthetic. Support for a
`redacted_export` classification defines validation behavior only; it is not
approval to create such an export.

## Closed intake chain

```text
Explicit Intake Approval
        |
        v
Structured-Facts Allowlist + Redaction Result
        |
        v
Candidate Lineage + Candidate Digest
        |
        v
Independent Human Label bound to Candidate Digest
        |
        v
Retention and Deletion Gate at trusted current time
        |
        v
Eligible for offline ShadowReplayCorpusV1 admission
```

No step may be inferred from another step. Passing this intake chain means
only that a candidate is structurally eligible for offline evaluation.

## RepresentativeCorpusIntakeBundleV1

The bundle contains:

- fixed schema identifier and version;
- pseudonymous intake, source-system, and purpose references;
- source classification: `synthetic_control` or `redacted_export`;
- digest of the source export before candidate transformation;
- explicit export time;
- intake approval object;
- retention and deletion policy object;
- redaction result and exact allowlist revision;
- one to 1,000 candidate records;
- a complete human-review label set;
- a domain-separated bundle digest.

All numbers are non-negative JSON safe integers. All identifiers are bounded
NFC ASCII tokens. Unknown fields are rejected.

## Intake approval

Approval must:

- be `approved`;
- explicitly allow representative offline export;
- be valid both when the bundle was exported and at verification time;
- carry a review reference;
- explicitly disallow raw conversation, credentials, Human Model payloads,
  and personality inference.

The conformance verifier receives `verification_time_ms` from its fixture
manifest. A real implementation must inject trusted current time from outside
the bundle. Bundle-provided time is never authoritative.

## Structured-facts allowlist

Only the following replay leaves are admissible:

```text
observation_ref
assessment_request.input.event_id
assessment_request.input.run_id
assessment_request.input.turn_id
assessment_request.input.source_actor
assessment_request.input.actor_ref
assessment_request.input.primary_semantic_role
assessment_request.input.semantic_roles
assessment_request.input.content_ref
assessment_request.input.content_digest
assessment_request.input.correlation_ref
assessment_request.input.logical_time
assessment_request.input.schema_version
assessment_request.fast_path_facts.deterministic_command_match
assessment_request.fast_path_facts.required_parameters_complete
assessment_request.fast_path_facts.ambiguity_present
assessment_request.fast_path_facts.authority_context_valid
assessment_request.fast_path_facts.policy_allows_request
assessment_request.fast_path_facts.impact_level
assessment_request.fast_path_facts.reversible
assessment_request.fast_path_facts.depends_on_human_model
assessment_request.decision_facts.multiple_viable_options
assessment_request.decision_facts.material_tradeoffs
assessment_request.decision_facts.evidence_conflict
assessment_request.decision_facts.impact_level
assessment_request.decision_facts.irreversible_result
assessment_request.decision_facts.affects_long_term_goal
assessment_request.decision_facts.host_user_interest_conflict
assessment_request.decision_facts.simple_lookup
assessment_request.decision_facts.unique_legal_operation
assessment_request.decision_facts.user_choice_already_explicit
assessment_request.coordination_facts.response_planned
assessment_request.coordination_facts.action_planned
assessment_request.coordination_facts.explicit_confirmation_requested
assessment_request.coordination_facts.authorization_required
assessment_request.coordination_facts.action_is_read_only
assessment_request.coordination_facts.long_running
assessment_request.coordination_facts.stream_progress
assessment_request.coordination_facts.safe_to_parallelize
host_projection.coordination_mode
host_projection.action_posture
host_projection.next_run_state
```

`depends_on_human_model` is a Boolean dependency flag from the frozen V1.3
contract; it is not permission to include Human Model content. No Human Model
value or inference is admissible.

Every string inside a replay envelope must be a bounded pseudonymous ASCII
token. Free-form text, email-like values, query strings, and multiline values
are rejected.

## Prohibited fields

The gate rejects raw content, messages, prompts, conversations, credentials,
secrets, Human Model content, personality or psychological profiles,
embeddings, vectors, graph memory, provider payloads, Actions,
Authorizations, Execution Permits, dispatch data, or production state.

Authority-field injection is reported separately from other prohibited-data
fields. Both are fail-closed.

## Candidate lineage and digest

Each candidate has:

- unique pseudonymous `candidate_id`;
- unique `lineage_digest` bound to its source transformation lineage;
- capture time not later than export time;
- one strict `ShadowReplayEnvelopeV1`;
- `candidate_digest`.

The candidate digest is lowercase SHA-256 over canonical JSON excluding the
digest field, using the domain:

```text
idr:aegis:representative-candidate:v1
```

Changing lineage, capture time, observation facts, or host projection
invalidates the digest.

## Human review label binding

Every candidate must have exactly one label, and no extra label is allowed. A
label binds both `candidate_id` and the exact `candidate_digest` before adding:

- expected disposition: `assessed`, `blocked`, or stable protocol rejection;
- optional reviewed divergence classification and review reference.

Expected labels never enter IDR assessment. Candidate substitution after human
review invalidates both the label binding and bundle digest.

## Retention and deletion

The retention object must:

- carry a versioned policy reference;
- set `delete_after_ms` later than export and verification time;
- require a deletion receipt;
- declare that the raw source is not retained by the evaluation bundle.

Expiry is a hard denial. The verifier does not extend retention, delete source
systems, or manufacture a deletion receipt.

## Bundle digest

The final lowercase SHA-256 digest covers approval, retention, redaction,
candidates, candidate digests, and labels using the domain:

```text
idr:aegis:representative-intake-bundle:v1
```

It excludes only `bundle_digest` itself.

## Required rejection codes

- `UNKNOWN_FIELD`
- `APPROVAL_NOT_VALID`
- `APPROVAL_EXPIRED`
- `RETENTION_EXPIRED`
- `RETENTION_POLICY_VIOLATION`
- `PROHIBITED_DATA_FIELD`
- `AUTHORITY_FIELD_INJECTION`
- `REDACTION_NOT_PASSED`
- `ALLOWLIST_DRIFT`
- `CANDIDATE_DIGEST_MISMATCH`
- `LABEL_DIGEST_MISMATCH`
- `LABEL_COVERAGE_MISMATCH`
- `DUPLICATE_CANDIDATE_ID`
- `DUPLICATE_LINEAGE`
- `INVALID_TIME_ORDER`
- `BUNDLE_DIGEST_MISMATCH`

## Role boundaries

- Rust remains the only semantic assessment kernel. This specification does
  not alter it.
- TypeScript or another product-boundary implementation may construct and
  validate export bundles, but may not recompute IDR decisions.
- Python may measure admitted-corpus behavior and evaluation quality, but may
  not weaken admission, privacy, digest, or retention decisions.
- Aegis Life remains the data controller for any future export and must provide
  its own authorization, deletion execution, and audit evidence.

## Current limitation and source-review result

The source-specific data-flow review is recorded in
[`IDR-AEGIS-SOURCE-DATA-FLOW-REVIEW.md`](IDR-AEGIS-SOURCE-DATA-FLOW-REVIEW.md).
It found no complete or admitted product source for the 42 allowlisted replay
leaves. Four ingress metadata leaves have candidate-only analogues; all other
required facts or projections are absent from the product path or exist only in
synthetic/prebuilt shadow inputs.

No production exporter exists because the versioned structured source is not
implemented and the authoritative fact producers, export approval, trusted
clock, product-side allowlist projection, retention operator, deletion receipt,
independent label workflow, and dedicated sink remain unresolved. Only
synthetic controls may be used with this contract.

The wire shape for the first item is now specified by
[`IDR-AEGIS-STRUCTURED-OBSERVATION-SPEC.md`](IDR-AEGIS-STRUCTURED-OBSERVATION-SPEC.md),
but no Aegis producer implements it. “Specified” is not “admitted”: every
downstream intake gate in this document remains mandatory.

## Explicit exclusions

- no real Aegis data export;
- no passive capture or live mirror;
- no raw conversation storage;
- no Setoka, personality inference, or Human Model feature;
- no vector database, graph memory, or semantic retrieval;
- no database migration;
- no production decision, dispatch, or state mutation;
- no change to IDR V1.3 production source.
