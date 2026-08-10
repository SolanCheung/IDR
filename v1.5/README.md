# IDR V1.5 Embeddable Runtime

Status: **Embeddable MVP / Reference Implementation**. This is not production ready.

V1.5 is a standalone workspace. It does not change the root V1.3 Cargo workspace,
PostgreSQL trust chain, protocol, runtime, store, or interaction client.

## Runtime boundary

```text
Host Agent
  -> ResolveRequestV1
  -> idr-core
       -> DecisionContractV1
       OR HostModelRequestV1
       OR UnresolvedResultV1 (fail closed)
  -> host's existing LLM (when requested)
  -> HostModelResultV1
  -> continue_resolve
  -> DecisionContractV1
  -> host action
  -> OutcomeFeedbackV1
  -> feedback
  -> Human Model update + EvaluationRecordV1
```

IDR owns intent, context, evidence, scoped Human Model assertions, and decision
resolution. The host owns the model, agent loop, tools, API keys, and business
actions. IDR never identifies or configures a model vendor.

## Rust public API

`IdrCore` exposes:

- `resolve(ResolveRequestV1)`
- `continue_resolve(ContinueResolveRequestV1)`
- `resolve_with_provider(request, &HostModelProvider)`
- `feedback(OutcomeFeedbackV1)`
- `observe(ObservationV1)`
- `query_human_model(QueryHumanModelRequestV1)`
- read-only evaluation and evidence accessors

The core has no database, model SDK, tool execution, reservation, permit,
dispatch, receipt-signing, or governance dependency. Its in-memory state is a
reference store; durable storage is a future host adapter boundary.

## Host-owned model modes

In-process Rust hosts implement `HostModelProvider`. `resolve_with_provider`
only calls the provider if normal resolution returns
`model_inference_required`, and it validates the structured result before
continuing. Recommended actions and alternatives must be declared in the
request's `supported_actions`; IDR never substitutes an arbitrary fallback.
When model inference is disabled, requests that require it return a structured
`unresolved` result instead of an unusable model request.

Language-neutral hosts use:

- `POST /v1/resolve`
- `POST /v1/resolve/continue`
- `POST /v1/feedback`
- `GET /v1/human-model/{subject_ref}`

The optional `idr-http` crate contains no decision or learning rules.

## Human Model V1 rules

- Only preference, constraint, expertise, goal, workflow, decision pattern,
  and interaction pattern assertions are allowed.
- Personality and psychological profiling observations are rejected.
- Explicit evidence activates immediately with high confidence.
- A single implicit signal remains a candidate. Three consistent signals are
  required for activation.
- Assertions only apply when every assertion scope field matches the request.
- Current request constraints block a different historical preferred action
  unless a future structured compatibility proof explicitly allows it.
- Inactive, contradicted, superseded, and expired assertions do not apply.
- Contradictions lower prior confidence, close prior valid time, preserve
  history, and record `supersedes`/`contradicts` relations.
- Inferred evidence cannot supersede an explicit assertion.
- Feedback must match the original recommendation, exact scope, and SHA-256
  decision digest before it can update the subject's model or evaluation
  records. Each decision consumes at most one feedback payload: identical
  replay returns the cached result, while conflicting replay fails closed. It
  never changes global policy or model weights online.

## Evaluation

`research/evaluation` reads `EvaluationRecordV1` JSONL without retaining raw
conversation and calculates:

- intent correction rate
- decision override rate
- re-clarification rate
- outcome success rate
- host model invocation rate

## Verification

```bash
cargo test --manifest-path v1.5/Cargo.toml
cargo clippy --manifest-path v1.5/Cargo.toml --all-targets -- -D warnings

cd v1.5/packages/idr-client
npm test
npm run typecheck

python -m unittest discover -s v1.5/research/evaluation/tests -v
```
