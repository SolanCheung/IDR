#!/usr/bin/env node

import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(fileURLToPath(import.meta.url));
const REF_TOKEN = /^[A-Za-z0-9_.:-]{1,512}$/;
const VERSION_TOKEN = /^[A-Za-z0-9_.:-]{1,128}$/;
const DIGEST = /^sha256:[a-f0-9]{64}$/;
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const ZERO_DIGEST = `sha256:${"0".repeat(64)}`;

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
const ACTION_POSTURES = new Set(["none", "blocked", "planned", "waiting_authorization"]);
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
const DERIVATION_CLASSES = new Set([
  "deterministic",
  "governed_model",
  "human_reviewed",
  "mixed",
]);
const LIFECYCLE_STATES = new Set(["draft", "sealed", "superseded", "revoked", "expired"]);

const FAMILY_BINDINGS = Object.freeze({
  semantic_roles: "semantic_classifier",
  fast_path_facts: "command_policy_classifier",
  decision_facts: "decision_context_classifier",
  coordination_facts: "coordination_planner",
  host_projection: "aegis_host_decision_runtime",
});

const EXPECTED_REPLAY_LEAVES = [
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
  "authority_context",
  "authorization",
  "credential",
  "credentials",
  "dispatch",
  "dispatch_nonce",
  "execution_permit",
  "provider_payload",
  "production_state",
]);
const PROHIBITED_FIELDS = new Set([
  "api_key",
  "content",
  "conversation",
  "conversation_text",
  "embedding",
  "error_text",
  "goal_text",
  "graph_memory",
  "human_model",
  "message",
  "message_text",
  "output_text",
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

function deny(code) {
  throw new GateError(code);
}

async function load(relativePath) {
  const value = JSON.parse(await readFile(join(root, relativePath), "utf8"));
  assertCanonicalValue(value);
  return value;
}

function clone(value) {
  return structuredClone(value);
}

function object(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
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
  if (object(value)) {
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

function scanForbidden(value) {
  if (Array.isArray(value)) {
    for (const entry of value) scanForbidden(entry);
    return;
  }
  if (!object(value)) return;
  for (const [key, entry] of Object.entries(value)) {
    if (AUTHORITY_FIELDS.has(key)) deny("AUTHORITY_FIELD_INJECTION");
    if (PROHIBITED_FIELDS.has(key)) deny("PROHIBITED_DATA_FIELD");
    scanForbidden(entry);
  }
}

function exactShape(value, requiredKeys) {
  if (!object(value)) deny("INVALID_SHAPE");
  const required = new Set(requiredKeys);
  for (const key of Object.keys(value)) {
    if (!required.has(key)) deny("UNKNOWN_FIELD");
  }
  for (const key of required) {
    if (!Object.hasOwn(value, key)) deny("MISSING_FIELD");
  }
}

function assertRef(value) {
  if (typeof value !== "string" || !REF_TOKEN.test(value)) deny("INVALID_REFERENCE");
}

function assertVersion(value) {
  if (typeof value !== "string" || !VERSION_TOKEN.test(value)) deny("INVALID_VERSION");
}

function assertDigest(value) {
  if (typeof value !== "string" || !DIGEST.test(value)) deny("INVALID_DIGEST");
}

function assertUuid(value) {
  if (typeof value !== "string" || !UUID.test(value)) deny("INVALID_UUID");
}

function assertPositiveInteger(value) {
  if (!Number.isSafeInteger(value) || value <= 0) deny("INVALID_LOGICAL_TIME");
}

function assertBoolean(value) {
  if (typeof value !== "boolean") deny("INVALID_BOOLEAN");
}

function assertEnum(value, allowed, code = "INVALID_ENUM") {
  if (!allowed.has(value)) deny(code);
}

function validateLifecycle(lifecycle) {
  exactShape(lifecycle, [
    "status",
    "valid_from_logical_time",
    "valid_until_logical_time",
    "supersedes_observation_digest",
    "revocation_ref",
  ]);
  assertEnum(lifecycle.status, LIFECYCLE_STATES, "INVALID_LIFECYCLE_STATE");
  assertPositiveInteger(lifecycle.valid_from_logical_time);
  assertPositiveInteger(lifecycle.valid_until_logical_time);
  if (lifecycle.valid_from_logical_time >= lifecycle.valid_until_logical_time) {
    deny("INVALID_VALIDITY_WINDOW");
  }
  if (lifecycle.supersedes_observation_digest !== null) {
    assertDigest(lifecycle.supersedes_observation_digest);
  }
  if (lifecycle.revocation_ref !== null) assertRef(lifecycle.revocation_ref);
  if (lifecycle.status !== "sealed") deny("OBSERVATION_NOT_SEALED");
  if (lifecycle.supersedes_observation_digest !== null || lifecycle.revocation_ref !== null) {
    deny("SEALED_STATE_CONTRADICTION");
  }
}

function validateReceipt(receipt, lifecycle) {
  exactShape(receipt, [
    "event_id",
    "run_id",
    "turn_id",
    "input_receipt_id",
    "source_actor",
    "actor_ref",
    "content_ref",
    "content_digest",
    "correlation_ref",
    "logical_time",
    "schema_version",
    "receipt_digest",
  ]);
  for (const key of ["event_id", "run_id", "turn_id", "input_receipt_id"]) {
    assertUuid(receipt[key]);
  }
  if (new Set([receipt.event_id, receipt.run_id, receipt.turn_id, receipt.input_receipt_id]).size !== 4) {
    deny("IDENTITY_COLLISION");
  }
  assertEnum(receipt.source_actor, SOURCE_ACTORS);
  assertRef(receipt.actor_ref);
  assertRef(receipt.content_ref);
  assertDigest(receipt.content_digest);
  assertRef(receipt.correlation_ref);
  assertPositiveInteger(receipt.logical_time);
  if (receipt.schema_version !== 1) deny("UNSUPPORTED_SCHEMA_VERSION");
  assertDigest(receipt.receipt_digest);
  if (
    receipt.logical_time < lifecycle.valid_from_logical_time ||
    receipt.logical_time >= lifecycle.valid_until_logical_time
  ) {
    deny("RECEIPT_OUTSIDE_VALIDITY");
  }
  const actual = digest(
    "idr:aegis:structured-observation-receipt:v1",
    receipt,
    "receipt_digest",
  );
  if (actual !== receipt.receipt_digest) deny("RECEIPT_DIGEST_MISMATCH");
}

function validateProducer(producer, expectedKind) {
  exactShape(producer, [
    "producer_ref",
    "producer_kind",
    "producer_version",
    "policy_ref",
    "derivation_class",
    "idr_output_used",
  ]);
  assertRef(producer.producer_ref);
  if (producer.producer_kind !== expectedKind) deny("PRODUCER_KIND_MISMATCH");
  assertVersion(producer.producer_version);
  assertRef(producer.policy_ref);
  assertEnum(producer.derivation_class, DERIVATION_CLASSES, "INVALID_DERIVATION_CLASS");
  assertBoolean(producer.idr_output_used);
  if (producer.idr_output_used) deny("IDR_CIRCULAR_DERIVATION");
}

function validateSemanticRoleFacts(facts) {
  exactShape(facts, ["primary_semantic_role", "semantic_roles"]);
  assertEnum(facts.primary_semantic_role, SEMANTIC_ROLES);
  if (!Array.isArray(facts.semantic_roles) || facts.semantic_roles.length === 0 || facts.semantic_roles.length > 16) {
    deny("INVALID_SEMANTIC_ROLES");
  }
  for (const role of facts.semantic_roles) assertEnum(role, SEMANTIC_ROLES);
  if (new Set(facts.semantic_roles).size !== facts.semantic_roles.length) {
    deny("INVALID_SEMANTIC_ROLES");
  }
  if (!facts.semantic_roles.includes(facts.primary_semantic_role)) {
    deny("PRIMARY_ROLE_OMITTED");
  }
}

function validateFastPathFacts(facts) {
  exactShape(facts, [
    "deterministic_command_match",
    "required_parameters_complete",
    "ambiguity_present",
    "authority_context_valid",
    "policy_allows_request",
    "impact_level",
    "reversible",
    "depends_on_human_model",
  ]);
  for (const key of [
    "deterministic_command_match",
    "required_parameters_complete",
    "ambiguity_present",
    "authority_context_valid",
    "policy_allows_request",
    "reversible",
    "depends_on_human_model",
  ]) {
    assertBoolean(facts[key]);
  }
  assertEnum(facts.impact_level, IMPACT_LEVELS);
}

function validateDecisionFacts(facts) {
  exactShape(facts, [
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
  ]);
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
    assertBoolean(facts[key]);
  }
  assertEnum(facts.impact_level, IMPACT_LEVELS);
}

function validateCoordinationFacts(facts) {
  exactShape(facts, [
    "response_planned",
    "action_planned",
    "explicit_confirmation_requested",
    "authorization_required",
    "action_is_read_only",
    "long_running",
    "stream_progress",
    "safe_to_parallelize",
  ]);
  for (const key of Object.keys(facts)) assertBoolean(facts[key]);
}

function validateHostProjection(facts) {
  exactShape(facts, ["coordination_mode", "action_posture", "next_run_state"]);
  assertEnum(facts.coordination_mode, COORDINATION_MODES);
  assertEnum(facts.action_posture, ACTION_POSTURES);
  assertEnum(facts.next_run_state, RUN_STATES);
}

const FACT_VALIDATORS = Object.freeze({
  semantic_roles: validateSemanticRoleFacts,
  fast_path_facts: validateFastPathFacts,
  decision_facts: validateDecisionFacts,
  coordination_facts: validateCoordinationFacts,
  host_projection: validateHostProjection,
});

function validateAssertion(name, assertion, receipt, lifecycle) {
  exactShape(assertion, [
    "schema_version",
    "family",
    "producer",
    "source_receipt_digest",
    "produced_at_logical_time",
    "facts",
    "assertion_digest",
  ]);
  if (assertion.schema_version !== 1) deny("UNSUPPORTED_SCHEMA_VERSION");
  if (assertion.family !== name) deny("ASSERTION_FAMILY_MISMATCH");
  validateProducer(assertion.producer, FAMILY_BINDINGS[name]);
  assertDigest(assertion.source_receipt_digest);
  if (assertion.source_receipt_digest !== receipt.receipt_digest) {
    deny("ASSERTION_RECEIPT_MISMATCH");
  }
  assertPositiveInteger(assertion.produced_at_logical_time);
  if (assertion.produced_at_logical_time < receipt.logical_time) {
    deny("ASSERTION_PRECEDES_RECEIPT");
  }
  if (
    assertion.produced_at_logical_time < lifecycle.valid_from_logical_time ||
    assertion.produced_at_logical_time >= lifecycle.valid_until_logical_time
  ) {
    deny("ASSERTION_OUTSIDE_VALIDITY");
  }
  FACT_VALIDATORS[name](assertion.facts);
  assertDigest(assertion.assertion_digest);
  const actual = digest(
    `idr:aegis:structured-observation-assertion:v1:${name}`,
    assertion,
    "assertion_digest",
  );
  if (actual !== assertion.assertion_digest) deny("ASSERTION_DIGEST_MISMATCH");
}

function collectLeafPaths(value, prefix = "", output = []) {
  if (Array.isArray(value) || value === null || typeof value !== "object") {
    output.push(prefix);
    return output;
  }
  for (const [key, entry] of Object.entries(value)) {
    collectLeafPaths(entry, prefix ? `${prefix}.${key}` : key, output);
  }
  return output;
}

function projectObservation(observation) {
  const receipt = observation.receipt;
  const assertions = observation.assertions;
  const semantic = assertions.semantic_roles.facts;
  return {
    observation_ref: observation.observation_ref,
    assessment_request: {
      input: {
        event_id: receipt.event_id,
        run_id: receipt.run_id,
        turn_id: receipt.turn_id,
        source_actor: receipt.source_actor,
        actor_ref: receipt.actor_ref,
        primary_semantic_role: semantic.primary_semantic_role,
        semantic_roles: clone(semantic.semantic_roles),
        content_ref: receipt.content_ref,
        content_digest: receipt.content_digest,
        correlation_ref: receipt.correlation_ref,
        logical_time: receipt.logical_time,
        schema_version: receipt.schema_version,
      },
      fast_path_facts: clone(assertions.fast_path_facts.facts),
      decision_facts: clone(assertions.decision_facts.facts),
      coordination_facts: clone(assertions.coordination_facts.facts),
    },
    host_projection: clone(assertions.host_projection.facts),
  };
}

function validateObservation(observation, projectionLogicalTime) {
  assertCanonicalValue(observation);
  scanForbidden(observation);
  exactShape(observation, [
    "schema_id",
    "schema_version",
    "observation_ref",
    "source_system_ref",
    "lifecycle",
    "receipt",
    "assertions",
    "observation_digest",
  ]);
  if (observation.schema_id !== "idr.aegis.structured-observation.v1") {
    deny("SCHEMA_ID_MISMATCH");
  }
  if (observation.schema_version !== 1) deny("UNSUPPORTED_SCHEMA_VERSION");
  assertRef(observation.observation_ref);
  assertRef(observation.source_system_ref);
  validateLifecycle(observation.lifecycle);
  validateReceipt(observation.receipt, observation.lifecycle);

  exactShape(observation.assertions, Object.keys(FAMILY_BINDINGS));
  for (const family of Object.keys(FAMILY_BINDINGS)) {
    validateAssertion(
      family,
      observation.assertions[family],
      observation.receipt,
      observation.lifecycle,
    );
  }

  assertDigest(observation.observation_digest);
  const actualObservationDigest = digest(
    "idr:aegis:structured-observation:v1",
    observation,
    "observation_digest",
  );
  if (actualObservationDigest !== observation.observation_digest) {
    deny("OBSERVATION_DIGEST_MISMATCH");
  }

  assertPositiveInteger(projectionLogicalTime);
  if (
    projectionLogicalTime < observation.lifecycle.valid_from_logical_time ||
    projectionLogicalTime >= observation.lifecycle.valid_until_logical_time
  ) {
    deny("OBSERVATION_NOT_CURRENT");
  }

  const projection = projectObservation(observation);
  const leaves = collectLeafPaths(projection).sort();
  if (JSON.stringify(leaves) !== JSON.stringify(EXPECTED_REPLAY_LEAVES)) {
    deny("PROJECTION_ALLOWLIST_MISMATCH");
  }
  const projectionDigest = digest(
    "idr:aegis:structured-observation-projection:v1",
    { projection, projection_digest: ZERO_DIGEST },
    "projection_digest",
  );
  return {
    receipt_digest: observation.receipt.receipt_digest,
    assertion_digests: Object.fromEntries(
      Object.entries(observation.assertions).map(([name, assertion]) => [
        name,
        assertion.assertion_digest,
      ]),
    ),
    observation_digest: observation.observation_digest,
    projection_digest: projectionDigest,
    projected_leaf_count: leaves.length,
    projection,
  };
}

function sealObservation(input) {
  const observation = clone(input);
  observation.receipt.receipt_digest = digest(
    "idr:aegis:structured-observation-receipt:v1",
    observation.receipt,
    "receipt_digest",
  );
  for (const [family, assertion] of Object.entries(observation.assertions)) {
    assertion.source_receipt_digest = observation.receipt.receipt_digest;
    assertion.assertion_digest = digest(
      `idr:aegis:structured-observation-assertion:v1:${family}`,
      assertion,
      "assertion_digest",
    );
  }
  observation.observation_digest = digest(
    "idr:aegis:structured-observation:v1",
    observation,
    "observation_digest",
  );
  return observation;
}

function applyMutation(golden, testCase, defaultProjectionTime) {
  const value = clone(golden);
  let projectionTime = defaultProjectionTime;
  const assertion = value.assertions.fast_path_facts;
  switch (testCase.mutation) {
    case "TOP_UNKNOWN_FIELD":
      value.unreviewed = true;
      break;
    case "RAW_TEXT_IN_RECEIPT":
      value.receipt.raw_text = "forbidden";
      break;
    case "AUTHORITY_FIELD_INJECTION":
      value.assertions.authorization = { status: "approved" };
      break;
    case "RECEIPT_DIGEST_SUBSTITUTION":
      value.receipt.receipt_digest = ZERO_DIGEST;
      break;
    case "RECEIPT_SUBSTITUTION_RESEALED":
      value.receipt.content_digest = `sha256:${"1".repeat(64)}`;
      value.receipt.receipt_digest = digest(
        "idr:aegis:structured-observation-receipt:v1",
        value.receipt,
        "receipt_digest",
      );
      break;
    case "ASSERTION_RECEIPT_MISMATCH":
      assertion.source_receipt_digest = ZERO_DIGEST;
      break;
    case "ASSERTION_DIGEST_SUBSTITUTION":
      assertion.assertion_digest = ZERO_DIGEST;
      break;
    case "OBSERVATION_DIGEST_SUBSTITUTION":
      value.observation_digest = ZERO_DIGEST;
      break;
    case "IDR_OUTPUT_USED":
      value.assertions.host_projection.producer.idr_output_used = true;
      break;
    case "PRODUCER_KIND_MISMATCH":
      assertion.producer.producer_kind = "coordination_planner";
      break;
    case "ASSERTION_FAMILY_MISMATCH":
      assertion.family = "decision_facts";
      break;
    case "PRIMARY_ROLE_OMITTED":
      value.assertions.semantic_roles.facts.semantic_roles = ["command"];
      break;
    case "DUPLICATE_SEMANTIC_ROLE":
      value.assertions.semantic_roles.facts.semantic_roles = ["intent", "intent"];
      break;
    case "ASSERTION_PRECEDES_RECEIPT":
      assertion.produced_at_logical_time = value.receipt.logical_time - 1;
      break;
    case "MISSING_HOST_PROJECTION":
      delete value.assertions.host_projection;
      break;
    case "LIFECYCLE_SUPERSEDED":
      value.lifecycle.status = "superseded";
      value.lifecycle.supersedes_observation_digest = `sha256:${"2".repeat(64)}`;
      break;
    case "PROJECTION_EXPIRED":
      projectionTime = value.lifecycle.valid_until_logical_time;
      break;
    case "UUID_COLLISION":
      value.receipt.turn_id = value.receipt.run_id;
      break;
    case "INVALID_CONTENT_REFERENCE":
      value.receipt.content_ref = "https://example.invalid/raw";
      break;
    case "EXTRA_FACT":
      assertion.facts.model_confidence = 99;
      break;
    case "PRODUCER_POLICY_DRIFT":
      assertion.producer.policy_ref = "policy:fast-path:v2";
      break;
    default:
      throw new Error(`unknown mutation ${testCase.mutation}`);
  }
  return { value, projectionTime };
}

const manifest = await load("manifest.json");
await load("idr-aegis-structured-observation-v1.schema.json");
const unsealedGolden = await load(manifest.golden.path);

if (process.argv.includes("--print-digests")) {
  const sealed = sealObservation(unsealedGolden);
  const result = validateObservation(sealed, manifest.projection_logical_time);
  process.stdout.write(`${JSON.stringify({ sealed, result }, null, 2)}\n`);
  process.exit(0);
}

const golden = unsealedGolden;
const result = validateObservation(golden, manifest.projection_logical_time);
if (
  result.receipt_digest !== manifest.golden.expected_receipt_digest ||
  result.observation_digest !== manifest.golden.expected_observation_digest ||
  result.projection_digest !== manifest.golden.expected_projection_digest ||
  JSON.stringify(result.assertion_digests) !==
    JSON.stringify(manifest.golden.expected_assertion_digests) ||
  result.projected_leaf_count !== 42
) {
  throw new Error("GOLDEN_VECTOR_MISMATCH");
}

const passed = [manifest.golden.id];
for (const testCase of manifest.negative_cases) {
  const { value, projectionTime } = applyMutation(
    golden,
    testCase,
    manifest.projection_logical_time,
  );
  let code = "NOT_REJECTED";
  try {
    validateObservation(value, projectionTime);
  } catch (error) {
    code = error instanceof GateError ? error.code : String(error);
  }
  if (code !== testCase.expected_code) {
    throw new Error(`${testCase.id}:EXPECTED_${testCase.expected_code}:ACTUAL_${code}`);
  }
  passed.push(testCase.id);
}

process.stdout.write(
  `${JSON.stringify(
    {
      pack_id: manifest.pack_id,
      status: manifest.status,
      projected_leaf_count: result.projected_leaf_count,
      passed: passed.length,
      failed: 0,
      cases: passed,
      producer_implementation: "NOT_AUTHORIZED",
      representative_exporter: "BLOCKED",
      real_data_access: "NOT_AUTHORIZED",
    },
    null,
    2,
  )}\n`,
);
