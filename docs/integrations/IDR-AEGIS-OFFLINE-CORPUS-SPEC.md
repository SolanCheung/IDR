# IDR × Aegis Life Offline Corpus Specification V1

Status: **Validation Only / No Production Authority**

Date: 2026-08-04

## Purpose

This specification defines the first measurable validation stage for the
frozen IDR V1.3 Aegis adapter. It permits bounded, privacy-reviewed offline
replay and aggregate comparison reporting. It does not authorize passive
production capture, live mirroring, dispatch, product state mutation, or
production use of an IDR decision.

The corpus runner is an evaluation tool outside the IDR V1.3 Round 14 frozen
source surface. IDR V1.4 Minimal remains Specification Only.

## Processing boundary

```text
Versioned offline corpus
        |
        v
Corpus metadata validation
        |
        v
Per-case privacy and authority-injection gate
        |
        v
Strict ShadowReplayEnvelopeV1 parsing
        |
        v
Frozen IDR V1.3 assess_shadow
        |
        v
Digest-only case result + aggregate metrics
```

The runner MUST process cases in memory, MUST NOT copy replay envelopes into
its report, and MUST emit the static boundary:

```text
effect_authority = none
dispatch = forbidden
production_state_mutation = forbidden
```

## Corpus object

`ShadowReplayCorpusV1` contains:

- `schema_version`: exactly `1`;
- `corpus_id`: bounded ASCII token;
- `privacy_review`: approved review reference and explicit absence flags;
- `cases`: between 1 and 1,000 unique cases.

Each `ShadowReplayCaseV1` contains:

- unique `case_id`;
- `source_classification`: `synthetic`, `audit_fixture`, or
  `redacted_export`;
- pseudonymous `source_ref`;
- expected outcome: non-blocking `assessed`, `blocked`, or a stable protocol
  rejection code;
- optional reviewed divergence classification;
- one structured replay envelope retained only for evaluation.

The initial committed corpus MUST be synthetic. A `redacted_export` requires a
separate data-handling approval before it may be added; this document does not
grant that approval.

## Privacy gate

Evaluation MUST fail at corpus level unless the privacy review is approved and
declares all of the following:

- no real user data;
- no raw conversation text;
- no credentials or secrets;
- no Human Model payload;
- no personality or psychological inference;
- no production authority object.

Every envelope string MUST be a bounded pseudonymous ASCII token. Free-form
text, email-like values, query strings, and multiline values are rejected.
Keys for raw content, prompts, messages, Human Model data, personality data,
embeddings, vectors, graph memory, secrets, credentials, Actions,
Authorizations, Execution Permits, dispatch, providers, or production state
are rejected before deserialization. Strict envelope parsing rejects all other
unknown fields.

Privacy failure is evaluation data only. It MUST NOT be retried, repaired, or
forwarded to another product component by the runner.

## Expected labels and divergence review

Expected outcome labels are test or human-review assertions, not IDR inputs.
They MUST NOT influence the IDR assessment. A returned assessment with blocked
action posture is a distinct `blocked` disposition and MUST NOT be counted as
an allow. The runner compares labels only after assessment to count:

- false allow: expected block or rejection, IDR produced a non-blocking
  assessment;
- false reject: expected non-blocking assessment, IDR blocked or rejected;
- wrong rejection code: expected and actual rejection codes differ.
- wrong blocking disposition: expected block but received protocol rejection,
  or the reverse.

Every host/IDR field divergence SHOULD carry one of the following reviewed
classifications:

- `host_defect`;
- `idr_defect`;
- `mapping_defect`;
- `policy_difference`;
- `insufficient_evidence`.

Missing reviews are counted as unresolved. A review attached to an exact,
uncompared, or rejected outcome is counted as orphaned and is not treated as a
valid classification.

## Required report metrics

`ShadowBatchReportV1` MUST report:

- total, parsed, evaluated, and rejected envelopes;
- privacy, parse, and observer failures;
- non-blocking assessed, blocked-assessment, and protocol-rejected outcomes;
- expected matches, false allows, false rejects, and wrong rejection codes;
- host comparisons, exact matches, divergences, and agreement basis points;
- coordination-mode, action-posture, and run-state divergence counts;
- divergence classification counts and unresolved reviews;
- confirmation-required count;
- stable rejection-code distribution;
- duplicate request-digest occurrences;
- source-classification counts;
- unknown-field and authority-field injection attempts;
- observer latency p50, p95, and p99 from the offline runner;
- corpus digest and static no-authority safety boundary.

Latency and match rate are observations, not release thresholds. The synthetic
smoke corpus deliberately contains one controlled host divergence and therefore
MUST NOT be presented as representative product accuracy.

## Synthetic boundary matrix

The reference host maintains a compact, typed 40-case matrix that expands into
the privacy-gated corpus format. It covers:

- `low`, `medium`, `high`, and `critical` impact levels;
- Fast Path eligible, ambiguous, incomplete, irreversible, authority-blocked,
  and policy-blocked profiles;
- decision-required, not-required, and conservative-unknown profiles;
- response-only, response-then-act, confirmation, parallel read-only,
  long-running acknowledgement, and progress-stream coordination;
- missing response, actionless confirmation/authorization, unsafe parallel,
  actionless stream, high-impact action without authorization, fact-impact
  mismatch, and Fast Path/decision-runtime conflict rejections.

The frozen selector exposes seven coordination enum values but has no branch
that returns `act_then_respond`. The matrix MUST report that value as
unobserved, not claim artificial IDR coverage. A reviewed host projection may
exercise the corresponding divergence path without changing the IDR result.

The specification-only host mapping profile now covers this value through the
independent source strategy `action_before_response`; see
[`IDR-AEGIS-HOST-PROJECTION-SEMANTIC-MAPPING-SPEC.md`](IDR-AEGIS-HOST-PROJECTION-SEMANTIC-MAPPING-SPEC.md).
This is synthetic mapping coverage, not observed IDR or Aegis product coverage.

The current deterministic matrix expectation is 20 non-blocking assessments,
6 blocked assessments, and 14 `INVALID_CONTRACT` rejections. All 40 labels are
expected to match, all request digests are expected to be unique, and its
domain-separated corpus digest is:

```text
sha256:d87ea7e9758e589566f27666d66d935116cab94226ed7ecfbc6280494d443038
```

These numbers prove deterministic branch mechanics only. They are not an
accuracy, safety, fairness, or production-readiness score.

## Advancement rule

This phase may be considered complete only when:

1. the corpus and request digests are stable;
2. all safety, privacy, parsing, metric, and frozen-adapter tests pass;
3. no V1.3 production source is changed;
4. every observed divergence is classified or explicitly unresolved;
5. the report contains no raw replay envelope;
6. no product dispatch or state-write interface is linked.

Passive production capture remains a separate Gate B decision. It requires a
new privacy, retention, deletion, resource-isolation, and failure-independence
review and is not authorized by completion of this corpus phase.

## Explicit exclusions

- no Setoka;
- no personality inference;
- no vector database or graph memory;
- no bitemporal semantic retrieval;
- no Human Model feature or write;
- no database migration;
- no live traffic;
- no production code or production enablement.
