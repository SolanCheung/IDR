import assert from "node:assert/strict";
import test from "node:test";
import {
  IdrClient,
  IdrUnresolvedError,
  resolveWithHostModel,
  SCHEMA_VERSION_V1,
  type DecisionContractV1,
  type FetchLike,
  type HostModelResultV1,
  type ResolveRequestV1,
} from "../src/index.ts";

const input: ResolveRequestV1 = {
  schema_version: SCHEMA_VERSION_V1,
  request_id: "client-1",
  subject_ref: "user-1",
  input: { message: "deploy" },
  intent_candidates: [],
  context: {
    scope: { domain: "software_development", risk: "low", reversible: true },
    current_constraints: [],
    data: {},
  },
  human_model_refs: [],
  human_model_snapshot: [],
  evidence: [],
  host_capabilities: { model_inference: true, supported_actions: ["deploy"] },
};

const decision: DecisionContractV1 = {
  schema_version: SCHEMA_VERSION_V1,
  decision_id: "decision-1",
  decision_digest: "a".repeat(64),
  request_id: input.request_id,
  resolved_intent: "deploy",
  intent_confidence: 0.9,
  recommended_action: { action: "deploy", parameters: {} },
  alternatives: [],
  constraints: [],
  ambiguities: [],
  human_model_basis: [],
  evidence_basis: [],
  reason_codes: ["host_model_result_validated"],
  decision_confidence: 0.9,
  model_usage: "host_delegated",
};

test("delegates model inference to the host exactly once", async () => {
  const paths: string[] = [];
  const fakeFetch: FetchLike = async (url, init) => {
    const path = new URL(url.toString()).pathname;
    paths.push(path);
    assert.equal(init?.method, "POST");
    if (path === "/v1/resolve") {
      return Response.json({
        type: "model_inference_required",
        model_request: {
          request_id: input.request_id,
          purpose: "intent_resolution",
          structured_context: {},
          expected_response_schema: {},
        },
      });
    }
    return Response.json({ type: "decision", decision });
  };
  const client = new IdrClient("https://idr.test/", fakeFetch);
  let modelCalls = 0;
  const resolved = await resolveWithHostModel(client, input, {
    async infer(request): Promise<HostModelResultV1> {
      modelCalls += 1;
      return {
        request_id: request.request_id,
        resolved_intent: "deploy",
        recommended_action: { action: "deploy", parameters: {} },
        alternatives: [],
        constraints: [],
        ambiguities: [],
        confidence: 0.9,
      };
    },
  });
  assert.equal(modelCalls, 1);
  assert.deepEqual(paths, ["/v1/resolve", "/v1/resolve/continue"]);
  assert.equal(resolved.decision_id, "decision-1");
});

test("does not call host model when IDR already has a decision", async () => {
  const client = new IdrClient("https://idr.test", async () =>
    Response.json({ type: "decision", decision }),
  );
  let modelCalls = 0;
  const resolved = await resolveWithHostModel(client, input, {
    async infer(): Promise<HostModelResultV1> {
      modelCalls += 1;
      throw new Error("unexpected model call");
    },
  });
  assert.equal(modelCalls, 0);
  assert.equal(resolved.decision_id, "decision-1");
});

test("does not call host model for a fail-closed unresolved result", async () => {
  const client = new IdrClient("https://idr.test", async () =>
    Response.json({
      type: "unresolved",
      unresolved: {
        request_id: input.request_id,
        resolved_intent: null,
        reason: "model_inference_unavailable",
        reason_codes: ["host_model_inference_disabled"],
      },
    }),
  );
  let modelCalls = 0;
  await assert.rejects(
    resolveWithHostModel(client, input, {
      async infer(): Promise<HostModelResultV1> {
        modelCalls += 1;
        throw new Error("unexpected model call");
      },
    }),
    IdrUnresolvedError,
  );
  assert.equal(modelCalls, 0);
});

test("feedback uses the standalone endpoint", async () => {
  let path = "";
  const client = new IdrClient("https://idr.test", async (url) => {
    path = new URL(url.toString()).pathname;
    return Response.json({
      evidence: [],
      assertions_updated: [],
      evaluation_record: {
        decision_id: "decision-1",
        intent_corrected: false,
        decision_overridden: false,
        clarification_required: false,
        model_inference_used: false,
        human_model_assertion_refs: [],
        evidence_refs: [],
        outcome_status: "success",
        resolved_at: "2026-01-01T00:00:00Z",
      },
    });
  });
  await client.feedback({
    decision_id: "decision-1",
    decision_digest: decision.decision_digest,
    recommended_action: { action: "deploy", parameters: {} },
    actual_action: { action: "deploy", parameters: {} },
    user_response: "accepted",
    outcome: { status: "success", details: {} },
    correction: null,
    scope: { domain: "software_development" },
    observed_at: "2026-01-01T00:00:00Z",
  });
  assert.equal(path, "/v1/feedback");
});
