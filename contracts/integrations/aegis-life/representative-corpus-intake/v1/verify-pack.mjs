#!/usr/bin/env node

import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(fileURLToPath(import.meta.url));
const MAX_CASES = 1_000;
const TOKEN = /^[A-Za-z0-9_.:/-]{1,512}$/;
const DIGEST = /^sha256:[a-f0-9]{64}$/;
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const IMPACT_LEVELS = new Set(["low", "medium", "high", "critical"]);
const COORDINATION_MODES = new Set([
  "respond_only",
  "act_then_respond",
  "respond_then_act",
  "respond_then_confirm_then_act",
  "parallel",
  "acknowledge_then_run",
  "stream_progress_then_final",
]);
const ACTION_POSTURES = new Set([
  "none",
  "blocked",
  "planned",
  "waiting_authorization",
]);
const RUN_STATES = new Set([
  "pending",
  "running",
  "waiting_input",
  "waiting_authorization",
  "waiting_dependency",
  "succeeded",
  "rejected",
  "failed",
  "cancelled",
  "timed_out",
  "invalidated",
]);
const SOURCE_ACTORS = new Set([
  "user",
  "host",
  "agent",
  "tool",
  "external_system",
  "human_approver",
  "scheduler",
  "policy_engine",
  "device_environment",
]);
const SEMANTIC_ROLES = new Set([
  "objective",
  "intent",
  "command",
  "fact",
  "observation",
  "proposal",
  "authorization",
  "policy",
  "result",
  "feedback",
  "time_trigger",
  "security_signal",
]);
const DIVERGENCE_CLASSIFICATIONS = new Set([
  "host_defect",
  "idr_defect",
  "mapping_defect",
  "policy_difference",
  "insufficient_evidence",
]);

const ALLOWED_REPLAY_LEAVES = [
  "observation_ref",
  "assessment_request.input.event_id",
  "assessment_request.input.run_id",
  "assessment_request.input.turn_id",
  "assessment_request.input.source_actor",
  "assessment_request.input.actor_ref",
  "assessment_request.input.primary_semantic_role",
  "assessment_request.input.semantic_roles",
  "assessment_request.input.content_ref",
  "assessment_request.input.content_digest",
  "assessment_request.input.correlation_ref",
  "assessment_request.input.logical_time",
  "assessment_request.input.schema_version",
  "assessment_request.fast_path_facts.deterministic_command_match",
  "assessment_request.fast_path_facts.required_parameters_complete",
  "assessment_request.fast_path_facts.ambiguity_present",
  "assessment_request.fast_path_facts.authority_context_valid",
  "assessment_request.fast_path_facts.policy_allows_request",
  "assessment_request.fast_path_facts.impact_level",
  "assessment_request.fast_path_facts.reversible",
  "assessment_request.fast_path_facts.depends_on_human_model",
  "assessment_request.decision_facts.multiple_viable_options",
  "assessment_request.decision_facts.material_tradeoffs",
  "assessment_request.decision_facts.evidence_conflict",
  "assessment_request.decision_facts.impact_level",
  "assessment_request.decision_facts.irreversible_result",
  "assessment_request.decision_facts.affects_long_term_goal",
  "assessment_request.decision_facts.host_user_interest_conflict",
  "assessment_request.decision_facts.simple_lookup",
  "assessment_request.decision_facts.unique_legal_operation",
  "assessment_request.decision_facts.user_choice_already_explicit",
  "assessment_request.coordination_facts.response_planned",
  "assessment_request.coordination_facts.action_planned",
  "assessment_request.coordination_facts.explicit_confirmation_requested",
  "assessment_request.coordination_facts.authorization_required",
  "assessment_request.coordination_facts.action_is_read_only",
  "assessment_request.coordination_facts.long_running",
  "assessment_request.coordination_facts.stream_progress",
  "assessment_request.coordination_facts.safe_to_parallelize",
  "host_projection.coordination_mode",
  "host_projection.action_posture",
  "host_projection.next_run_state",
].sort();

const AUTHORITY_FIELDS = new Set([
  "action",
  "action_contract",
  "authorization",
  "credential",
  "credentials",
  "dispatch",
  "dispatch_allowed",
  "dispatch_nonce",
  "execution_permit",
  "provider_payload",
  "production_state",
  "production_state_mutation",
]);
const PROHIBITED_DATA_FIELDS = new Set([
  "api_key",
  "content",
  "conversation",
  "conversation_text",
  "embedding",
  "graph_memory",
  "human_model",
  "message_text",
  "password",
  "personality",
  "prompt",
  "psychological_profile",
  "raw_text",
  "secret",
  "token",
  "vector",
]);

class GateError extends Error {
  constructor(code) {
    super(code);
    this.code = code;
  }
}

async function load(path) {
  const value = JSON.parse(await readFile(join(root, path), "utf8"));
  assertCanonicalValue(value);
  return value;
}

function deny(code) {
  throw new GateError(code);
}

function assertCanonicalValue(value) {
  if (value === null || typeof value === "boolean") return;
  if (typeof value === "string") {
    if (value !== value.normalize("NFC")) deny("NON_NFC_STRING");
    return;
  }
  if (typeof value === "number") {
    if (!Number.isSafeInteger(value) || value < 0) deny("UNSAFE_NUMBER");
    return;
  }
  if (Array.isArray(value)) {
    for (const entry of value) assertCanonicalValue(entry);
    return;
  }
  if (typeof value === "object") {
    for (const [key, entry] of Object.entries(value)) {
      assertCanonicalValue(key);
      assertCanonicalValue(entry);
    }
    return;
  }
  deny("UNSUPPORTED_JSON_TYPE");
}

function canonicalJson(value) {
  if (value === null || typeof value !== "object") return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
  return `{${Object.keys(value)
    .sort()
    .map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`)
    .join(",")}}`;
}

function digest(domain, value, digestField) {
  const preimage = { ...value };
  delete preimage[digestField];
  const hash = createHash("sha256");
  hash.update(domain, "ascii");
  hash.update(Buffer.from([0]));
  hash.update(canonicalJson(preimage), "utf8");
  return `sha256:${hash.digest("hex")}`;
}

function object(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function exactShape(value, allowed, required = allowed) {
  if (!object(value)) deny("INVALID_SHAPE");
  for (const key of Object.keys(value)) {
    if (!allowed.has(key)) deny("UNKNOWN_FIELD");
  }
  for (const key of required) {
    if (!Object.hasOwn(value, key)) deny("MISSING_FIELD");
  }
}

function assertToken(value) {
  if (typeof value !== "string" || !TOKEN.test(value)) deny("INVALID_TOKEN");
}

function assertDigest(value) {
  if (typeof value !== "string" || !DIGEST.test(value)) deny("INVALID_DIGEST");
}

function assertBoolean(value) {
  if (typeof value !== "boolean") deny("INVALID_BOOLEAN");
}

function assertInteger(value, { positive = false } = {}) {
  if (!Number.isSafeInteger(value) || value < (positive ? 1 : 0)) deny("INVALID_INTEGER");
}

function assertEnum(value, allowed) {
  if (typeof value !== "string" || !allowed.has(value)) deny("INVALID_ENUM");
}

function assertTokenTree(value) {
  if (typeof value === "string") {
    assertToken(value);
    return;
  }
  if (Array.isArray(value)) {
    for (const entry of value) assertTokenTree(entry);
    return;
  }
  if (object(value)) {
    for (const entry of Object.values(value)) assertTokenTree(entry);
  }
}

function scanProhibitedFields(value) {
  if (Array.isArray(value)) {
    for (const entry of value) scanProhibitedFields(entry);
    return;
  }
  if (!object(value)) return;
  for (const [key, entry] of Object.entries(value)) {
    if (AUTHORITY_FIELDS.has(key)) deny("AUTHORITY_FIELD_INJECTION");
    if (PROHIBITED_DATA_FIELDS.has(key)) deny("PROHIBITED_DATA_FIELD");
    scanProhibitedFields(entry);
  }
}

function collectLeaves(value, prefix = "", leaves = []) {
  if (Array.isArray(value)) {
    leaves.push(prefix);
    return leaves;
  }
  if (object(value)) {
    for (const [key, entry] of Object.entries(value)) {
      collectLeaves(entry, prefix ? `${prefix}.${key}` : key, leaves);
    }
    return leaves;
  }
  leaves.push(prefix);
  return leaves;
}

function assertReplayEnvelope(value) {
  scanProhibitedFields(value);
  assertTokenTree(value);
  exactShape(
    value,
    new Set(["observation_ref", "assessment_request", "host_projection"]),
    new Set(["observation_ref", "assessment_request"]),
  );
  assertToken(value.observation_ref);

  const request = value.assessment_request;
  exactShape(
    request,
    new Set(["input", "fast_path_facts", "decision_facts", "coordination_facts"]),
  );
  const input = request.input;
  exactShape(
    input,
    new Set([
      "event_id",
      "run_id",
      "turn_id",
      "source_actor",
      "actor_ref",
      "primary_semantic_role",
      "semantic_roles",
      "content_ref",
      "content_digest",
      "correlation_ref",
      "logical_time",
      "schema_version",
    ]),
  );
  if (!UUID.test(input.event_id) || !UUID.test(input.run_id) || !UUID.test(input.turn_id)) {
    deny("INVALID_UUID");
  }
  assertEnum(input.source_actor, SOURCE_ACTORS);
  assertToken(input.actor_ref);
  assertEnum(input.primary_semantic_role, SEMANTIC_ROLES);
  if (!Array.isArray(input.semantic_roles) || input.semantic_roles.length === 0) {
    deny("INVALID_SEMANTIC_ROLES");
  }
  const roles = new Set();
  for (const role of input.semantic_roles) {
    assertEnum(role, SEMANTIC_ROLES);
    if (roles.has(role)) deny("INVALID_SEMANTIC_ROLES");
    roles.add(role);
  }
  if (!roles.has(input.primary_semantic_role)) deny("INVALID_SEMANTIC_ROLES");
  assertToken(input.content_ref);
  assertDigest(input.content_digest);
  assertToken(input.correlation_ref);
  assertInteger(input.logical_time, { positive: true });
  if (input.schema_version !== 1) deny("UNSUPPORTED_REPLAY_SCHEMA");

  const fast = request.fast_path_facts;
  exactShape(
    fast,
    new Set([
      "deterministic_command_match",
      "required_parameters_complete",
      "ambiguity_present",
      "authority_context_valid",
      "policy_allows_request",
      "impact_level",
      "reversible",
      "depends_on_human_model",
    ]),
  );
  for (const key of [
    "deterministic_command_match",
    "required_parameters_complete",
    "ambiguity_present",
    "authority_context_valid",
    "policy_allows_request",
    "reversible",
    "depends_on_human_model",
  ]) {
    assertBoolean(fast[key]);
  }
  assertEnum(fast.impact_level, IMPACT_LEVELS);

  const decision = request.decision_facts;
  exactShape(
    decision,
    new Set([
      "multiple_viable_options",
      "material_tradeoffs",
      "evidence_conflict",
      "impact_level",
      "irreversible_result",
      "affects_long_term_goal",
      "host_user_interest_conflict",
      "simple_lookup",
      "unique_legal_operation",
      "user_choice_already_explicit",
    ]),
  );
  for (const key of [
    "multiple_viable_options",
    "material_tradeoffs",
    "evidence_conflict",
    "irreversible_result",
    "affects_long_term_goal",
    "host_user_interest_conflict",
    "simple_lookup",
    "unique_legal_operation",
    "user_choice_already_explicit",
  ]) {
    assertBoolean(decision[key]);
  }
  assertEnum(decision.impact_level, IMPACT_LEVELS);

  const coordination = request.coordination_facts;
  exactShape(
    coordination,
    new Set([
      "response_planned",
      "action_planned",
      "explicit_confirmation_requested",
      "authorization_required",
      "action_is_read_only",
      "long_running",
      "stream_progress",
      "safe_to_parallelize",
    ]),
  );
  for (const key of Object.keys(coordination)) assertBoolean(coordination[key]);

  if (value.host_projection !== undefined) {
    exactShape(
      value.host_projection,
      new Set(["coordination_mode", "action_posture", "next_run_state"]),
    );
    assertEnum(value.host_projection.coordination_mode, COORDINATION_MODES);
    assertEnum(value.host_projection.action_posture, ACTION_POSTURES);
    assertEnum(value.host_projection.next_run_state, RUN_STATES);
  }

  const leaves = collectLeaves(value).sort();
  if (leaves.some((path) => !ALLOWED_REPLAY_LEAVES.includes(path))) {
    deny("ALLOWLIST_DRIFT");
  }
}

function assertExpectedOutcome(value) {
  if (!object(value) || typeof value.status !== "string") deny("INVALID_LABEL");
  if (value.status === "assessed" || value.status === "blocked") {
    exactShape(value, new Set(["status"]));
    return;
  }
  if (value.status === "rejected") {
    exactShape(value, new Set(["status", "reason_code"]));
    assertToken(value.reason_code);
    return;
  }
  deny("INVALID_LABEL");
}

function assertDivergenceReview(value) {
  exactShape(value, new Set(["classification", "review_ref"]));
  assertEnum(value.classification, DIVERGENCE_CLASSIFICATIONS);
  assertToken(value.review_ref);
}

function validateBundle(bundle, verificationTimeMs) {
  exactShape(
    bundle,
    new Set([
      "schema_id",
      "schema_version",
      "intake_id",
      "purpose",
      "source_classification",
      "source_system_ref",
      "source_export_digest",
      "exported_at_ms",
      "approval",
      "retention",
      "redaction",
      "candidates",
      "label_set",
      "bundle_digest",
    ]),
  );
  if (bundle.schema_id !== "idr.aegis.representative-corpus-intake.v1") {
    deny("UNSUPPORTED_SCHEMA");
  }
  if (bundle.schema_version !== 1) deny("UNSUPPORTED_SCHEMA");
  assertToken(bundle.intake_id);
  if (bundle.purpose !== "offline_shadow_evaluation") deny("INVALID_PURPOSE");
  assertEnum(bundle.source_classification, new Set(["synthetic_control", "redacted_export"]));
  assertToken(bundle.source_system_ref);
  assertDigest(bundle.source_export_digest);
  assertInteger(bundle.exported_at_ms, { positive: true });
  assertInteger(verificationTimeMs, { positive: true });
  if (bundle.exported_at_ms > verificationTimeMs) deny("INVALID_TIME_ORDER");

  const approval = bundle.approval;
  exactShape(
    approval,
    new Set([
      "status",
      "review_ref",
      "valid_from_ms",
      "valid_until_ms",
      "allows_representative_export",
      "allows_raw_conversation",
      "allows_credentials",
      "allows_human_model",
      "allows_personality_inference",
    ]),
  );
  assertToken(approval.review_ref);
  assertInteger(approval.valid_from_ms, { positive: true });
  assertInteger(approval.valid_until_ms, { positive: true });
  for (const key of [
    "allows_representative_export",
    "allows_raw_conversation",
    "allows_credentials",
    "allows_human_model",
    "allows_personality_inference",
  ]) {
    assertBoolean(approval[key]);
  }
  if (approval.valid_from_ms >= approval.valid_until_ms) deny("INVALID_TIME_ORDER");
  if (
    approval.status !== "approved" ||
    !approval.allows_representative_export ||
    approval.allows_raw_conversation ||
    approval.allows_credentials ||
    approval.allows_human_model ||
    approval.allows_personality_inference
  ) {
    deny("APPROVAL_NOT_VALID");
  }
  if (
    bundle.exported_at_ms < approval.valid_from_ms ||
    bundle.exported_at_ms >= approval.valid_until_ms ||
    verificationTimeMs < approval.valid_from_ms ||
    verificationTimeMs >= approval.valid_until_ms
  ) {
    deny("APPROVAL_EXPIRED");
  }

  const retention = bundle.retention;
  exactShape(
    retention,
    new Set([
      "policy_ref",
      "delete_after_ms",
      "deletion_receipt_required",
      "raw_source_retained",
    ]),
  );
  assertToken(retention.policy_ref);
  assertInteger(retention.delete_after_ms, { positive: true });
  assertBoolean(retention.deletion_receipt_required);
  assertBoolean(retention.raw_source_retained);
  if (retention.delete_after_ms <= bundle.exported_at_ms) deny("INVALID_TIME_ORDER");
  if (retention.delete_after_ms <= verificationTimeMs) deny("RETENTION_EXPIRED");
  if (!retention.deletion_receipt_required || retention.raw_source_retained) {
    deny("RETENTION_POLICY_VIOLATION");
  }

  const redaction = bundle.redaction;
  exactShape(
    redaction,
    new Set(["profile_id", "status", "review_ref", "allowlisted_paths", "findings"]),
  );
  assertToken(redaction.profile_id);
  assertToken(redaction.review_ref);
  assertInteger(redaction.findings);
  if (redaction.status !== "passed" || redaction.findings !== 0) {
    deny("REDACTION_NOT_PASSED");
  }
  if (!Array.isArray(redaction.allowlisted_paths)) deny("ALLOWLIST_DRIFT");
  const policyPaths = [...redaction.allowlisted_paths].sort();
  if (
    policyPaths.length !== ALLOWED_REPLAY_LEAVES.length ||
    policyPaths.some((path, index) => path !== ALLOWED_REPLAY_LEAVES[index])
  ) {
    deny("ALLOWLIST_DRIFT");
  }

  if (!Array.isArray(bundle.candidates) || bundle.candidates.length === 0) {
    deny("LABEL_COVERAGE_MISMATCH");
  }
  if (bundle.candidates.length > MAX_CASES) deny("CORPUS_LIMIT_EXCEEDED");
  const candidatesById = new Map();
  const lineageDigests = new Set();
  for (const candidate of bundle.candidates) {
    exactShape(
      candidate,
      new Set([
        "candidate_id",
        "lineage_digest",
        "captured_at_ms",
        "replay_envelope",
        "candidate_digest",
      ]),
    );
    assertToken(candidate.candidate_id);
    assertDigest(candidate.lineage_digest);
    assertInteger(candidate.captured_at_ms, { positive: true });
    assertDigest(candidate.candidate_digest);
    if (candidate.captured_at_ms > bundle.exported_at_ms) deny("INVALID_TIME_ORDER");
    if (candidatesById.has(candidate.candidate_id)) deny("DUPLICATE_CANDIDATE_ID");
    if (lineageDigests.has(candidate.lineage_digest)) deny("DUPLICATE_LINEAGE");
    candidatesById.set(candidate.candidate_id, candidate);
    lineageDigests.add(candidate.lineage_digest);
    assertReplayEnvelope(candidate.replay_envelope);
    const actualCandidateDigest = digest(
      "idr:aegis:representative-candidate:v1",
      candidate,
      "candidate_digest",
    );
    if (actualCandidateDigest !== candidate.candidate_digest) {
      deny("CANDIDATE_DIGEST_MISMATCH");
    }
  }

  const labelSet = bundle.label_set;
  exactShape(
    labelSet,
    new Set(["schema_version", "intake_id", "review_ref", "reviewed_at_ms", "labels"]),
  );
  if (labelSet.schema_version !== 1 || labelSet.intake_id !== bundle.intake_id) {
    deny("LABEL_COVERAGE_MISMATCH");
  }
  assertToken(labelSet.review_ref);
  assertInteger(labelSet.reviewed_at_ms, { positive: true });
  if (
    labelSet.reviewed_at_ms < bundle.exported_at_ms ||
    labelSet.reviewed_at_ms > verificationTimeMs
  ) {
    deny("INVALID_TIME_ORDER");
  }
  if (!Array.isArray(labelSet.labels)) deny("LABEL_COVERAGE_MISMATCH");
  const labeledIds = new Set();
  for (const label of labelSet.labels) {
    exactShape(
      label,
      new Set([
        "candidate_id",
        "candidate_digest",
        "expected_outcome",
        "divergence_review",
      ]),
      new Set(["candidate_id", "candidate_digest", "expected_outcome"]),
    );
    assertToken(label.candidate_id);
    assertDigest(label.candidate_digest);
    assertExpectedOutcome(label.expected_outcome);
    if (label.divergence_review !== undefined) {
      assertDivergenceReview(label.divergence_review);
    }
    if (labeledIds.has(label.candidate_id)) deny("LABEL_COVERAGE_MISMATCH");
    labeledIds.add(label.candidate_id);
    const candidate = candidatesById.get(label.candidate_id);
    if (!candidate) deny("LABEL_COVERAGE_MISMATCH");
    if (candidate.candidate_digest !== label.candidate_digest) {
      deny("LABEL_DIGEST_MISMATCH");
    }
  }
  if (
    labeledIds.size !== candidatesById.size ||
    [...candidatesById.keys()].some((candidateId) => !labeledIds.has(candidateId))
  ) {
    deny("LABEL_COVERAGE_MISMATCH");
  }

  assertDigest(bundle.bundle_digest);
  const actualBundleDigest = digest(
    "idr:aegis:representative-intake-bundle:v1",
    bundle,
    "bundle_digest",
  );
  if (actualBundleDigest !== bundle.bundle_digest) deny("BUNDLE_DIGEST_MISMATCH");

  return {
    candidate_count: bundle.candidates.length,
    candidate_digests: bundle.candidates.map((candidate) => candidate.candidate_digest),
    bundle_digest: bundle.bundle_digest,
  };
}

const manifest = await load("manifest.json");
await load("idr-aegis-representative-intake-v1.schema.json");
const passed = [];

const goldenCase = manifest.valid_cases[0];
const golden = await load(goldenCase.path);
const goldenResult = validateBundle(golden, manifest.verification_time_ms);
if (goldenResult.bundle_digest !== goldenCase.expected_bundle_digest) {
  throw new Error(`GOLDEN_BUNDLE_DIGEST:${goldenResult.bundle_digest}`);
}
if (
  goldenResult.candidate_digests.length !== goldenCase.expected_candidate_digests.length ||
  goldenResult.candidate_digests.some(
    (candidateDigest, index) => candidateDigest !== goldenCase.expected_candidate_digests[index],
  )
) {
  throw new Error("GOLDEN_CANDIDATE_DIGEST");
}
passed.push(goldenCase.id);

for (const testCase of manifest.invalid_cases) {
  const value = await load(testCase.path);
  let actualCode = "NOT_REJECTED";
  try {
    validateBundle(value, manifest.verification_time_ms);
  } catch (error) {
    actualCode = error instanceof GateError ? error.code : String(error);
  }
  if (actualCode !== testCase.expected_code) {
    throw new Error(`${testCase.id}:EXPECTED_${testCase.expected_code}:ACTUAL_${actualCode}`);
  }
  passed.push(testCase.id);
}

process.stdout.write(
  `${JSON.stringify(
    {
      pack_id: manifest.pack_id,
      status: manifest.status,
      verification_time_ms: manifest.verification_time_ms,
      passed: passed.length,
      failed: 0,
      cases: passed,
    },
    null,
    2,
  )}\n`,
);
