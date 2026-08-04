import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import {
  buildExactActionAuthorizationCandidate,
  parseCanonicalInputEvent,
  parseInteractionAssessment,
} from "../src/index.ts";

const fixture = JSON.parse(
  readFileSync(
    new URL(
      "../../../contracts/human-centered/v1/fixtures/supplier-confirmation.json",
      import.meta.url,
    ),
    "utf8",
  ),
) as Record<string, unknown>;

test("product boundary accepts the shared canonical multi-role input", () => {
  const input = parseCanonicalInputEvent(fixture.input);
  assert.equal(input.source_actor, "user");
  assert.equal(input.primary_semantic_role, "intent");
  assert.deepEqual(input.semantic_roles, ["intent", "command"]);
});

test("product boundary normalizes UUIDs to canonical lowercase", () => {
  const source = fixture.input as Record<string, unknown>;
  const input = parseCanonicalInputEvent({
    ...source,
    event_id: String(source.event_id).toUpperCase(),
    run_id: String(source.run_id).toUpperCase(),
    turn_id: String(source.turn_id).toUpperCase(),
  });
  assert.equal(input.event_id, String(source.event_id).toLowerCase());
  assert.equal(input.run_id, String(source.run_id).toLowerCase());
  assert.equal(input.turn_id, String(source.turn_id).toLowerCase());
});

test("product boundary rejects a primary semantic role omitted from roles", () => {
  const input = {
    ...(fixture.input as Record<string, unknown>),
    semantic_roles: ["command"],
  };
  assert.throws(
    () => parseCanonicalInputEvent(input),
    /contain primary_semantic_role/,
  );
});

test("product boundary rejects an unverified content digest", () => {
  const input = {
    ...(fixture.input as Record<string, unknown>),
    content_digest: "not-a-digest",
  };
  assert.throws(
    () => parseCanonicalInputEvent(input),
    /content_digest must use the sha256/,
  );
});

test("product boundary rejects actor-role impersonation", () => {
  const input = {
    ...(fixture.input as Record<string, unknown>),
    source_actor: "tool",
    actor_ref: "tool:search",
    primary_semantic_role: "authorization",
    semantic_roles: ["authorization"],
  };
  assert.throws(
    () => parseCanonicalInputEvent(input),
    /allowed for source_actor/,
  );
});

test("product boundary rejects unknown canonical input fields", () => {
  assert.throws(
    () =>
      parseCanonicalInputEvent({
        ...(fixture.input as Record<string, unknown>),
        untrusted_extension: true,
      }),
    /unknown fields/,
  );
});

test("product boundary parses but does not recompute Rust decisions", () => {
  const expected = fixture.expected as Record<string, unknown>;
  const assessment = parseInteractionAssessment({
    schema_version: 1,
    fast_path: {
      outcome: expected.fast_path_outcome,
      reason_codes: ["ambiguity_present"],
      rule_version: 1,
    },
    decision_necessity: {
      outcome: expected.decision_necessity,
      enters_decision_runtime: true,
      reason_codes: ["multiple_viable_options"],
      rule_version: 1,
    },
    coordination_mode: expected.coordination_mode,
    action_posture: expected.action_posture,
    next_run_state: expected.next_run_state,
  });
  assert.equal(assessment.coordination_mode, "respond_then_confirm_then_act");
  assert.equal(assessment.next_run_state, "waiting_authorization");
});

test("assessment parser rejects contradictory cross-field states", () => {
  assert.throws(
    () =>
      parseInteractionAssessment({
        schema_version: 1,
        fast_path: {
          outcome: "allow_fast_path",
          reason_codes: ["all_conditions_satisfied"],
          rule_version: 1,
        },
        decision_necessity: {
          outcome: "required",
          enters_decision_runtime: true,
          reason_codes: ["material_tradeoffs"],
          rule_version: 1,
        },
        coordination_mode: "respond_then_act",
        action_posture: "planned",
        next_run_state: "running",
      }),
    /contradictory guard decisions/,
  );
});

test("blocked assessment is rejected rather than marked succeeded", () => {
  const blocked = {
    schema_version: 1,
    fast_path: {
      outcome: "block",
      reason_codes: ["policy_denied"],
      rule_version: 1,
    },
    decision_necessity: {
      outcome: "required",
      enters_decision_runtime: true,
      reason_codes: ["material_tradeoffs"],
      rule_version: 1,
    },
    coordination_mode: "respond_only",
    action_posture: "blocked",
    next_run_state: "rejected",
  };
  assert.equal(parseInteractionAssessment(blocked).next_run_state, "rejected");
  assert.throws(
    () =>
      parseInteractionAssessment({
        ...blocked,
        next_run_state: "succeeded",
      }),
    /invalid terminal posture/,
  );
});

test("authorization candidate is bound to exact action revision and digests", () => {
  const digest = "a".repeat(64);
  const candidate = buildExactActionAuthorizationCandidate({
    actionRef: {
      kind: "action",
      contract_id: "33333333-3333-4333-8333-333333333333",
      revision: 4,
      record_digest: digest,
    },
    parameterDigest: "b".repeat(64),
    decision: "approve",
  });
  assert.equal(candidate.action_ref.revision, 4);
  assert.equal(candidate.parameter_digest, "b".repeat(64));

  assert.throws(
    () =>
      buildExactActionAuthorizationCandidate({
        actionRef: {
          kind: "action",
          contract_id: "33333333-3333-4333-8333-333333333333",
          revision: 4,
          record_digest: digest,
        },
        parameterDigest: "not-a-digest",
        decision: "approve",
      }),
    /SHA-256/,
  );

  assert.throws(
    () =>
      buildExactActionAuthorizationCandidate({
        actionRef: {
          kind: "action",
          contract_id: "33333333-3333-4333-8333-333333333333",
          revision: 4,
          record_digest: digest,
        },
        parameterDigest: "b".repeat(64),
        decision: "bypass" as "approve",
      }),
    /decision is unsupported/,
  );
});
