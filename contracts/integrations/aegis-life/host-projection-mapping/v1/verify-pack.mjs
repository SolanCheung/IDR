#!/usr/bin/env node

import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(fileURLToPath(import.meta.url));
const repositoryRoot = resolve(root, "../../../../..");
const DIGEST = /^sha256:[0-9a-f]{64}$/;
const REF_TOKEN = /^[a-z0-9][a-z0-9._:/-]{2,179}$/;
const GIT_SHA = /^[0-9a-f]{40}$/;
const SNAPSHOT_DOMAIN = "idr:aegis:host-decision-snapshot:v1";
const GATE_DOMAIN = "idr:aegis:host-projection-implementation-gate:v1";
const PROFILE_DOMAIN = "idr:aegis:host-projection-mapping-profile:v1";

const PROHIBITED_FIELDS = new Set([
  "action",
  "authorization",
  "conversation",
  "execution_permit",
  "goal_text",
  "human_model",
  "message_text",
  "output_text",
  "personality",
  "prompt",
  "provider_payload",
  "raw_text",
  "secret",
  "token",
  "vector",
]);

const COORDINATION_MAPPING = [
  { source: "response_only", target: "respond_only" },
  { source: "action_before_response", target: "act_then_respond" },
  { source: "response_before_action", target: "respond_then_act" },
  {
    source: "response_then_authorization_gate_then_action",
    target: "respond_then_confirm_then_act",
  },
  { source: "parallel_response_and_action", target: "parallel" },
  { source: "acknowledge_before_background_run", target: "acknowledge_then_run" },
  { source: "stream_progress_before_final", target: "stream_progress_then_final" },
];

const ACTION_MAPPING = [
  { source: "no_action", target: "none" },
  { source: "action_blocked", target: "blocked" },
  { source: "action_ready", target: "planned" },
  { source: "action_waiting_authorization", target: "waiting_authorization" },
];

const RUN_MAPPING = [
  { source: "queued", target: "pending" },
  { source: "executing", target: "running" },
  { source: "waiting_user_input", target: "waiting_input" },
  { source: "waiting_action_authorization", target: "waiting_authorization" },
  { source: "waiting_external_dependency", target: "waiting_dependency" },
  { source: "completed", target: "succeeded" },
  { source: "declined", target: "rejected" },
  { source: "execution_failed", target: "failed" },
  { source: "user_cancelled", target: "cancelled" },
  { source: "deadline_exceeded", target: "timed_out" },
  { source: "superseded_or_stale", target: "invalidated" },
];

const CONSISTENCY_RULES = [
  "no_action_requires_response_only",
  "non_response_only_requires_action",
  "waiting_action_requires_waiting_run",
  "waiting_run_requires_waiting_action",
  "blocked_action_requires_declined_run",
  "declined_run_requires_blocked_action",
];

const PROHIBITED_INPUTS = [
  "idr_assessment",
  "idr_coordination_mode",
  "idr_action_posture",
  "idr_next_run_state",
  "shadow_comparison_result",
  "shadow_divergence",
  "evaluation_label",
  "chosen_action",
  "execution_result",
  "goal_outcome",
  "journal_record",
  "synthetic_fixture_result",
];

const GATE_CHECKS = [
  "host_mapping_profile_locked",
  "host_source_owner_approved",
  "host_snapshot_implementation_registered",
  "pre_idr_capture_order_verified",
  "receipt_source_binding_verified",
  "source_enum_semantics_reviewed",
  "coordination_mapping_coverage_verified",
  "action_mapping_coverage_verified",
  "run_state_mapping_coverage_verified",
  "cross_field_consistency_tested",
  "no_circular_derivation_verified",
  "deterministic_mapper_verified",
  "failure_isolation_verified",
  "security_privacy_review_passed",
  "cross_language_conformance_passed",
];

const SNAPSHOT_FIELDS = [
  "schema_id",
  "schema_version",
  "snapshot_ref",
  "receipt_digest",
  "receipt_source_binding_digest",
  "captured_at_logical_time",
  "owner_ref",
  "owner_version",
  "policy_ref",
  "host_coordination_strategy",
  "host_action_state",
  "host_run_phase",
  "idr_output_used",
  "shadow_result_used",
  "snapshot_digest",
];

class GateError extends Error {
  constructor(code) {
    super(code);
    this.code = code;
  }
}

function deny(code) {
  throw new GateError(code);
}

function object(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function clone(value) {
  return structuredClone(value);
}

async function load(relativePath) {
  const value = JSON.parse(await readFile(join(root, relativePath), "utf8"));
  assertCanonicalValue(value);
  return value;
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

function same(actual, expected) {
  return canonicalJson(actual) === canonicalJson(expected);
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

function bytesDigest(bytes) {
  return `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
}

function gateDigest(gate) {
  return digest(GATE_DOMAIN, gate, "gate_digest");
}

function profileDigest(profile) {
  return digest(PROFILE_DOMAIN, profile, "profile_digest");
}

function snapshotDigest(snapshot) {
  return digest(SNAPSHOT_DOMAIN, snapshot, "snapshot_digest");
}

function sealProfile(profile) {
  profile.implementation_gate.gate_digest = gateDigest(profile.implementation_gate);
  profile.profile_digest = profileDigest(profile);
  return profile;
}

function sealSnapshot(snapshot) {
  snapshot.snapshot_digest = snapshotDigest(snapshot);
  return snapshot;
}

function assertExactKeys(value, keys) {
  if (!object(value)) deny("INVALID_OBJECT");
  const expected = new Set(keys);
  for (const key of Object.keys(value)) {
    if (!expected.has(key)) deny("UNKNOWN_FIELD");
  }
  for (const key of keys) {
    if (!(key in value)) deny("MISSING_FIELD");
  }
}

function scanForbidden(value) {
  if (Array.isArray(value)) {
    for (const entry of value) scanForbidden(entry);
    return;
  }
  if (!object(value)) return;
  for (const [key, entry] of Object.entries(value)) {
    if (PROHIBITED_FIELDS.has(key)) deny("PROHIBITED_DATA_FIELD");
    scanForbidden(entry);
  }
}

function flipDigest(value) {
  const tail = value.at(-1);
  return `${value.slice(0, -1)}${tail === "0" ? "1" : "0"}`;
}

async function validateTarget(target) {
  assertExactKeys(target, [
    "schema_id",
    "schema_version",
    "schema_path",
    "schema_digest",
    "projection_definition",
    "required_fields",
  ]);
  if (
    target.schema_id !== "idr.aegis.structured-observation" ||
    target.schema_version !== 1 ||
    target.schema_path !==
      "contracts/integrations/aegis-life/structured-observation/v1/idr-aegis-structured-observation-v1.schema.json" ||
    target.projection_definition !== "HostProjectionFactsV1" ||
    !same(target.required_fields, [
      "coordination_mode",
      "action_posture",
      "next_run_state",
    ]) ||
    !DIGEST.test(target.schema_digest)
  ) {
    deny("TARGET_SCHEMA_BINDING_MISMATCH");
  }
  const bytes = await readFile(join(repositoryRoot, target.schema_path));
  if (bytesDigest(bytes) !== target.schema_digest) deny("TARGET_SCHEMA_DIGEST_MISMATCH");
}

function validateSourcePin(pin) {
  assertExactKeys(pin, [
    "repository",
    "commit_sha",
    "tree_sha",
    "review_document",
    "review_scope",
  ]);
  if (
    pin.repository !== "Aegis Life" ||
    pin.commit_sha !== "381d14fb1dccfeae691286a39579535ef59f7b4e" ||
    pin.tree_sha !== "da76b01653bb194e4f192e5768b32a38fd3b2173" ||
    pin.review_document !== "docs/integrations/IDR-AEGIS-SOURCE-DATA-FLOW-REVIEW.md" ||
    pin.review_scope !== "tracked_source_and_documentation_only" ||
    !GIT_SHA.test(pin.commit_sha) ||
    !GIT_SHA.test(pin.tree_sha)
  ) {
    deny("SOURCE_REVIEW_PIN_MISMATCH");
  }
}

function validateLifecycle(lifecycle) {
  assertExactKeys(lifecycle, [
    "status",
    "created_at_logical_time",
    "valid_from_logical_time",
    "valid_until_logical_time",
  ]);
  if (
    lifecycle.status !== "SPECIFICATION_ONLY" ||
    lifecycle.created_at_logical_time > lifecycle.valid_from_logical_time ||
    lifecycle.valid_from_logical_time >= lifecycle.valid_until_logical_time
  ) {
    deny("LIFECYCLE_MISMATCH");
  }
}

function validateOwner(owner) {
  assertExactKeys(owner, ["owner_kind", "owner_ref", "trust_state", "reason_code"]);
  if (owner.trust_state !== "NOT_IMPLEMENTED") deny("OWNER_NOT_IMPLEMENTED");
  if (
    owner.owner_kind !== "aegis_host_decision_runtime" ||
    owner.owner_ref !== "component:aegis-host-decision-runtime" ||
    owner.reason_code !== "NO_INDEPENDENT_PRODUCT_HOST_SNAPSHOT" ||
    !REF_TOKEN.test(owner.owner_ref)
  ) {
    deny("OWNER_BOUNDARY_MISMATCH");
  }
}

async function validateSourceSnapshotContract(contract) {
  assertExactKeys(contract, [
    "schema_id",
    "schema_version",
    "schema_path",
    "schema_digest",
    "status",
    "required_fields",
    "digest_domain",
    "capture_order",
    "invalid_snapshot",
  ]);
  if (
    contract.schema_id !== "idr.aegis.host-decision-snapshot" ||
    contract.schema_version !== 1 ||
    contract.schema_path !==
      "contracts/integrations/aegis-life/host-projection-mapping/v1/idr-aegis-host-decision-snapshot-v1.schema.json" ||
    contract.status !== "SPECIFIED_NOT_IMPLEMENTED" ||
    !same(contract.required_fields, SNAPSHOT_FIELDS) ||
    contract.digest_domain !== SNAPSHOT_DOMAIN ||
    contract.capture_order !== "after_receipt_before_idr_assessment" ||
    contract.invalid_snapshot !== "REJECT_PROJECTION" ||
    !DIGEST.test(contract.schema_digest)
  ) {
    deny("SOURCE_SNAPSHOT_CONTRACT_MISMATCH");
  }
  const bytes = await readFile(join(repositoryRoot, contract.schema_path));
  if (bytesDigest(bytes) !== contract.schema_digest) deny("SOURCE_SCHEMA_DIGEST_MISMATCH");
}

function validateMapping(actual, expected, code) {
  if (!Array.isArray(actual) || actual.length !== expected.length) deny(code);
  for (const entry of actual) assertExactKeys(entry, ["source", "target"]);
  if (!same(actual, expected)) deny(code);
  if (new Set(actual.map((entry) => entry.source)).size !== actual.length) deny(code);
  if (new Set(actual.map((entry) => entry.target)).size !== actual.length) deny(code);
}

function validateIndependence(policy) {
  assertExactKeys(policy, [
    "capture_order",
    "mapping_mode",
    "network_access",
    "database_access",
    "model_call",
    "unknown_value",
    "prohibited_inputs",
  ]);
  const expected = {
    capture_order: "source_snapshot_before_idr_assessment",
    mapping_mode: "pure_exact_lookup",
    network_access: "forbidden",
    database_access: "forbidden",
    model_call: "forbidden",
    unknown_value: "REJECT_PROJECTION",
    prohibited_inputs: PROHIBITED_INPUTS,
  };
  if (!same(policy, expected)) deny("INDEPENDENCE_POLICY_MISMATCH");
}

function validateGate(gate, lifecycle) {
  assertExactKeys(gate, [
    "decision",
    "assessed_at_logical_time",
    "checks",
    "decision_reason",
    "gate_digest",
  ]);
  if (
    gate.decision !== "BLOCKED" ||
    gate.decision_reason !== "HOST_PROJECTION_SOURCE_NOT_IMPLEMENTED_OR_APPROVED"
  ) {
    deny("IMPLEMENTATION_GATE_BLOCKED");
  }
  if (
    gate.assessed_at_logical_time < lifecycle.valid_from_logical_time ||
    gate.assessed_at_logical_time >= lifecycle.valid_until_logical_time
  ) {
    deny("GATE_TIME_INVALID");
  }
  if (!Array.isArray(gate.checks) || gate.checks.length !== GATE_CHECKS.length) {
    deny("GATE_CHECKS_MISMATCH");
  }
  const names = gate.checks.map((check) => check.check);
  if (!same(names, GATE_CHECKS) || new Set(names).size !== names.length) {
    deny("GATE_CHECKS_MISMATCH");
  }
  gate.checks.forEach((check, index) => {
    assertExactKeys(check, ["check", "status", "evidence_refs"]);
    const expectedStatus = index === 0 ? "SATISFIED" : "UNSATISFIED";
    if (check.status === "UNSATISFIED" && check.evidence_refs.length !== 0) {
      deny("GATE_EVIDENCE_MISMATCH");
    }
    if (check.status !== expectedStatus) deny("GATE_STATUS_MISMATCH");
    const expectedEvidence = index === 0 ? ["spec:host-projection-mapping-v1"] : [];
    if (!same(check.evidence_refs, expectedEvidence)) deny("GATE_EVIDENCE_MISMATCH");
  });
  if (!DIGEST.test(gate.gate_digest) || gateDigest(gate) !== gate.gate_digest) {
    deny("GATE_DIGEST_MISMATCH");
  }
  return gate.checks.filter((check) => check.status === "SATISFIED").length;
}

async function validateProfile(profile) {
  assertCanonicalValue(profile);
  scanForbidden(profile);
  assertExactKeys(profile, [
    "schema_id",
    "schema_version",
    "mapping_ref",
    "target_projection_schema",
    "source_review_pin",
    "lifecycle",
    "owner",
    "source_snapshot_contract",
    "coordination_mapping",
    "action_mapping",
    "run_state_mapping",
    "consistency_rules",
    "independence_policy",
    "implementation_gate",
    "profile_digest",
  ]);
  if (
    profile.schema_id !== "idr.aegis.host-projection-mapping-profile" ||
    profile.schema_version !== 1 ||
    !REF_TOKEN.test(profile.mapping_ref)
  ) {
    deny("PROFILE_IDENTITY_MISMATCH");
  }
  await validateTarget(profile.target_projection_schema);
  validateSourcePin(profile.source_review_pin);
  validateLifecycle(profile.lifecycle);
  validateOwner(profile.owner);
  await validateSourceSnapshotContract(profile.source_snapshot_contract);
  validateMapping(profile.coordination_mapping, COORDINATION_MAPPING, "COORDINATION_MAPPING_MISMATCH");
  validateMapping(profile.action_mapping, ACTION_MAPPING, "ACTION_MAPPING_MISMATCH");
  validateMapping(profile.run_state_mapping, RUN_MAPPING, "RUN_MAPPING_MISMATCH");
  if (!same(profile.consistency_rules, CONSISTENCY_RULES)) {
    deny("CONSISTENCY_RULES_MISMATCH");
  }
  validateIndependence(profile.independence_policy);
  const satisfiedGateCount = validateGate(profile.implementation_gate, profile.lifecycle);
  if (!DIGEST.test(profile.profile_digest) || profileDigest(profile) !== profile.profile_digest) {
    deny("PROFILE_DIGEST_MISMATCH");
  }
  return satisfiedGateCount;
}

function validateConsistency(source) {
  const strategy = source.host_coordination_strategy;
  const action = source.host_action_state;
  const run = source.host_run_phase;
  if (action === "no_action" && strategy !== "response_only") {
    deny("INCONSISTENT_HOST_SNAPSHOT");
  }
  if (strategy !== "response_only" && action === "no_action") {
    deny("INCONSISTENT_HOST_SNAPSHOT");
  }
  if (
    action === "action_waiting_authorization" &&
    run !== "waiting_action_authorization"
  ) {
    deny("INCONSISTENT_HOST_SNAPSHOT");
  }
  if (
    run === "waiting_action_authorization" &&
    action !== "action_waiting_authorization"
  ) {
    deny("INCONSISTENT_HOST_SNAPSHOT");
  }
  if (action === "action_blocked" && run !== "declined") {
    deny("INCONSISTENT_HOST_SNAPSHOT");
  }
  if (run === "declined" && action !== "action_blocked") {
    deny("INCONSISTENT_HOST_SNAPSHOT");
  }
}

function lookup(entries, source) {
  const entry = entries.find((candidate) => candidate.source === source);
  if (!entry) deny("UNKNOWN_SOURCE_ENUM");
  return entry.target;
}

function projectSource(profile, source) {
  assertExactKeys(source, [
    "host_coordination_strategy",
    "host_action_state",
    "host_run_phase",
  ]);
  const coordination = lookup(profile.coordination_mapping, source.host_coordination_strategy);
  const action = lookup(profile.action_mapping, source.host_action_state);
  const run = lookup(profile.run_state_mapping, source.host_run_phase);
  validateConsistency(source);
  return {
    coordination_mode: coordination,
    action_posture: action,
    next_run_state: run,
  };
}

function validateSnapshot(snapshot, profile, receiptTime, assessmentStartTime) {
  assertCanonicalValue(snapshot);
  scanForbidden(snapshot);
  assertExactKeys(snapshot, SNAPSHOT_FIELDS);
  if (
    snapshot.schema_id !== "idr.aegis.host-decision-snapshot" ||
    snapshot.schema_version !== 1 ||
    !REF_TOKEN.test(snapshot.snapshot_ref) ||
    !DIGEST.test(snapshot.receipt_digest) ||
    !DIGEST.test(snapshot.receipt_source_binding_digest)
  ) {
    deny("SNAPSHOT_IDENTITY_MISMATCH");
  }
  if (
    snapshot.owner_ref !== profile.owner.owner_ref ||
    !REF_TOKEN.test(snapshot.owner_version) ||
    !REF_TOKEN.test(snapshot.policy_ref)
  ) {
    deny("SNAPSHOT_OWNER_MISMATCH");
  }
  if (snapshot.idr_output_used || snapshot.shadow_result_used) {
    deny("CIRCULAR_DERIVATION");
  }
  const source = {
    host_coordination_strategy: snapshot.host_coordination_strategy,
    host_action_state: snapshot.host_action_state,
    host_run_phase: snapshot.host_run_phase,
  };
  const projection = projectSource(profile, source);
  if (
    snapshot.captured_at_logical_time < receiptTime ||
    snapshot.captured_at_logical_time >= assessmentStartTime
  ) {
    deny("SNAPSHOT_CAPTURE_ORDER_INVALID");
  }
  if (!DIGEST.test(snapshot.snapshot_digest) || snapshotDigest(snapshot) !== snapshot.snapshot_digest) {
    deny("SNAPSHOT_DIGEST_MISMATCH");
  }
  return projection;
}

function validateVectors(vectors, profile) {
  assertExactKeys(vectors, ["schema_id", "schema_version", "vectors"]);
  if (
    vectors.schema_id !== "idr.aegis.host-projection-mapping-vectors" ||
    vectors.schema_version !== 1 ||
    !Array.isArray(vectors.vectors)
  ) {
    deny("VECTOR_SET_INVALID");
  }
  const ids = new Set();
  const sourceCoordination = new Set();
  const targetCoordination = new Set();
  const sourceAction = new Set();
  const targetAction = new Set();
  const sourceRun = new Set();
  const targetRun = new Set();
  const caseIds = [];
  for (const vector of vectors.vectors) {
    assertExactKeys(vector, ["id", "source", "expected"]);
    if (!REF_TOKEN.test(vector.id.toLowerCase().replaceAll("_", "-"))) deny("VECTOR_ID_INVALID");
    if (ids.has(vector.id)) deny("DUPLICATE_VECTOR_ID");
    ids.add(vector.id);
    assertExactKeys(vector.expected, ["coordination_mode", "action_posture", "next_run_state"]);
    const actual = projectSource(profile, vector.source);
    if (!same(actual, vector.expected)) deny("VECTOR_PROJECTION_MISMATCH");
    sourceCoordination.add(vector.source.host_coordination_strategy);
    targetCoordination.add(actual.coordination_mode);
    sourceAction.add(vector.source.host_action_state);
    targetAction.add(actual.action_posture);
    sourceRun.add(vector.source.host_run_phase);
    targetRun.add(actual.next_run_state);
    caseIds.push(vector.id);
  }
  if (
    sourceCoordination.size !== COORDINATION_MAPPING.length ||
    targetCoordination.size !== COORDINATION_MAPPING.length ||
    sourceAction.size !== ACTION_MAPPING.length ||
    targetAction.size !== ACTION_MAPPING.length ||
    sourceRun.size !== RUN_MAPPING.length ||
    targetRun.size !== RUN_MAPPING.length
  ) {
    deny("VECTOR_COVERAGE_INCOMPLETE");
  }
  return {
    vectorCount: vectors.vectors.length,
    coordinationCoverage: sourceCoordination.size,
    actionCoverage: sourceAction.size,
    runCoverage: sourceRun.size,
    caseIds,
  };
}

function mutateProfile(profile, mutation) {
  switch (mutation) {
    case "PROFILE_UNKNOWN_FIELD":
      profile.unreviewed = true;
      break;
    case "TARGET_SCHEMA_DIGEST_SUBSTITUTION":
      profile.target_projection_schema.schema_digest = flipDigest(
        profile.target_projection_schema.schema_digest,
      );
      break;
    case "SOURCE_SCHEMA_DIGEST_SUBSTITUTION":
      profile.source_snapshot_contract.schema_digest = flipDigest(
        profile.source_snapshot_contract.schema_digest,
      );
      break;
    case "SOURCE_PIN_DRIFT":
      profile.source_review_pin.tree_sha = "0a76b01653bb194e4f192e5768b32a38fd3b2173";
      break;
    case "OWNER_PRETENDS_IMPLEMENTED":
      profile.owner.trust_state = "ADMITTED";
      break;
    case "COORDINATION_MAPPING_MISSING":
      profile.coordination_mapping.pop();
      break;
    case "COORDINATION_TARGET_DUPLICATE":
      profile.coordination_mapping[1].target = "respond_only";
      break;
    case "ACTION_MAPPING_DRIFT":
      profile.action_mapping[2].target = "blocked";
      break;
    case "RUN_MAPPING_DRIFT":
      profile.run_state_mapping[0].target = "running";
      break;
    case "CONSISTENCY_RULE_MISSING":
      profile.consistency_rules.pop();
      break;
    case "IDR_INPUT_ALLOWED":
      profile.independence_policy.prohibited_inputs.shift();
      break;
    case "MAPPER_MODEL_CALL":
      profile.independence_policy.model_call = "allowed";
      break;
    case "MISSING_GATE_CHECK":
      profile.implementation_gate.checks.pop();
      break;
    case "UNSATISFIED_WITH_EVIDENCE":
      profile.implementation_gate.checks[1].evidence_refs.push("evidence:unverified");
      break;
    case "GATE_FALSE_READY":
      profile.implementation_gate.decision = "READY";
      break;
    case "GATE_DIGEST_SUBSTITUTION":
      profile.implementation_gate.gate_digest = flipDigest(
        profile.implementation_gate.gate_digest,
      );
      break;
    case "PROFILE_DIGEST_SUBSTITUTION":
      profile.profile_digest = flipDigest(profile.profile_digest);
      break;
    default:
      throw new Error(`unknown profile mutation ${mutation}`);
  }
  return profile;
}

function mutateSnapshot(snapshot, mutation) {
  switch (mutation) {
    case "SNAPSHOT_UNKNOWN_FIELD":
      snapshot.unreviewed = true;
      break;
    case "SNAPSHOT_RAW_TEXT":
      snapshot.raw_text = "synthetic prohibited content";
      break;
    case "IDR_OUTPUT_USED":
      snapshot.idr_output_used = true;
      break;
    case "SHADOW_RESULT_USED":
      snapshot.shadow_result_used = true;
      break;
    case "SNAPSHOT_OWNER_MISMATCH":
      snapshot.owner_ref = "component:shadow-validator";
      break;
    case "SNAPSHOT_DIGEST_SUBSTITUTION":
      snapshot.snapshot_digest = flipDigest(snapshot.snapshot_digest);
      break;
    case "NO_ACTION_WITH_ACTION_STRATEGY":
      snapshot.host_action_state = "no_action";
      break;
    case "WAITING_ACTION_WITHOUT_WAITING_RUN":
      snapshot.host_run_phase = "queued";
      break;
    case "DECLINED_WITHOUT_BLOCKED_ACTION":
      snapshot.host_run_phase = "declined";
      break;
    case "UNKNOWN_SOURCE_ENUM":
      snapshot.host_run_phase = "invented_state";
      break;
    default:
      throw new Error(`unknown snapshot mutation ${mutation}`);
  }
  return snapshot;
}

function mutateVectors(vectors, mutation) {
  if (mutation !== "VECTOR_EXPECTATION_DRIFT") {
    throw new Error(`unknown vector mutation ${mutation}`);
  }
  vectors.vectors[0].expected.next_run_state = "running";
  return vectors;
}

const manifest = await load("manifest.json");
const [profileSchemaBytes, snapshotSchemaBytes] = await Promise.all([
  readFile(join(root, manifest.profile_schema)),
  readFile(join(root, manifest.snapshot_schema)),
]);
if (bytesDigest(profileSchemaBytes) !== manifest.profile_schema_digest) {
  deny("MANIFEST_PROFILE_SCHEMA_DIGEST_MISMATCH");
}
if (bytesDigest(snapshotSchemaBytes) !== manifest.snapshot_schema_digest) {
  deny("MANIFEST_SNAPSHOT_SCHEMA_DIGEST_MISMATCH");
}

const [profile, snapshot, vectors] = await Promise.all([
  load(manifest.golden.profile_path),
  load(manifest.golden.snapshot_path),
  load(manifest.golden.vectors_path),
]);

if (process.argv.includes("--print-digests")) {
  const sealedProfile = sealProfile(clone(profile));
  const sealedSnapshot = sealSnapshot(clone(snapshot));
  console.log(
    JSON.stringify(
      {
        gate: sealedProfile.implementation_gate.gate_digest,
        profile: sealedProfile.profile_digest,
        snapshot: sealedSnapshot.snapshot_digest,
      },
      null,
      2,
    ),
  );
  process.exit(0);
}

const cases = [];
let failed = 0;
let satisfiedGateCount;
let vectorResult;

try {
  satisfiedGateCount = await validateProfile(profile);
  cases.push("SYNTHETIC_HOST_PROJECTION_MAPPING_PROFILE_GOLDEN_V1");
  const projection = validateSnapshot(
    snapshot,
    profile,
    manifest.synthetic_receipt_logical_time,
    manifest.synthetic_idr_assessment_start_logical_time,
  );
  if (
    !same(projection, {
      coordination_mode: "respond_then_confirm_then_act",
      action_posture: "waiting_authorization",
      next_run_state: "waiting_authorization",
    })
  ) {
    deny("GOLDEN_SNAPSHOT_PROJECTION_MISMATCH");
  }
  cases.push("SYNTHETIC_HOST_DECISION_SNAPSHOT_GOLDEN_V1");
  vectorResult = validateVectors(vectors, profile);
  cases.push(...vectorResult.caseIds);
  if (profile.implementation_gate.gate_digest !== manifest.golden.expected_gate_digest) {
    deny("MANIFEST_GATE_DIGEST_MISMATCH");
  }
  if (profile.profile_digest !== manifest.golden.expected_profile_digest) {
    deny("MANIFEST_PROFILE_DIGEST_MISMATCH");
  }
  if (snapshot.snapshot_digest !== manifest.golden.expected_snapshot_digest) {
    deny("MANIFEST_SNAPSHOT_DIGEST_MISMATCH");
  }
  if (
    vectorResult.vectorCount !== manifest.golden.expected_vector_count ||
    vectorResult.coordinationCoverage !== manifest.golden.expected_coordination_coverage ||
    vectorResult.actionCoverage !== manifest.golden.expected_action_coverage ||
    vectorResult.runCoverage !== manifest.golden.expected_run_state_coverage ||
    satisfiedGateCount !== manifest.golden.expected_satisfied_gate_count
  ) {
    deny("MANIFEST_COVERAGE_MISMATCH");
  }
} catch (error) {
  failed += 1;
  cases.push(`GOLDEN:FAILED:${error.code ?? error.message}`);
}

for (const testCase of manifest.negative_cases) {
  try {
    if (testCase.target === "profile") {
      await validateProfile(mutateProfile(clone(profile), testCase.mutation));
    } else if (testCase.target === "snapshot") {
      validateSnapshot(
        mutateSnapshot(clone(snapshot), testCase.mutation),
        profile,
        manifest.synthetic_receipt_logical_time,
        manifest.synthetic_idr_assessment_start_logical_time,
      );
    } else if (testCase.target === "vectors") {
      validateVectors(mutateVectors(clone(vectors), testCase.mutation), profile);
    } else {
      throw new Error(`unknown target ${testCase.target}`);
    }
    failed += 1;
    cases.push(`${testCase.id}:FAILED:ACCEPTED`);
  } catch (error) {
    const code = error.code ?? error.message;
    if (code !== testCase.expected_code) {
      failed += 1;
      cases.push(`${testCase.id}:FAILED:${code}`);
    } else {
      cases.push(testCase.id);
    }
  }
}

const result = {
  pack_id: manifest.pack_id,
  status: manifest.status,
  vector_count: vectorResult?.vectorCount ?? null,
  coordination_coverage: vectorResult?.coordinationCoverage ?? null,
  action_coverage: vectorResult?.actionCoverage ?? null,
  run_state_coverage: vectorResult?.runCoverage ?? null,
  satisfied_implementation_checks: satisfiedGateCount ?? null,
  passed: cases.length - failed,
  failed,
  cases,
  host_projection_semantic_mapping: "SPECIFIED_NOT_IMPLEMENTED",
  host_projection_source_owner: "NOT_APPROVED",
  host_projection_implementation_gate: "BLOCKED",
  representative_exporter: "BLOCKED",
  real_data_access: "NOT_AUTHORIZED",
};

console.log(JSON.stringify(result, null, 2));
if (failed !== 0) process.exit(1);
