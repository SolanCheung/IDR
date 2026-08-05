import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.dirname(fileURLToPath(import.meta.url));
const reviewPath = path.join(root, "aegis-source-readiness-review-v1.json");
const review = JSON.parse(await readFile(reviewPath, "utf8"));

const expectedTopLevel = [
  "schema_id",
  "schema_version",
  "review_id",
  "reviewed_at",
  "reviewed_host",
  "scope",
  "decision",
  "reviewed_files",
  "source_surfaces",
  "field_groups",
  "required_gates",
];

const expectedPaths = [
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
];

const expectedGates = [
  "VERSIONED_STRUCTURED_SOURCE_SCHEMA",
  "AUTHORITATIVE_FACT_PRODUCERS",
  "EXACT_INGRESS_IDENTITY_AND_TIME_SEMANTICS",
  "SOURCE_SPECIFIC_EXPORT_APPROVAL",
  "CONSENT_OR_LAWFUL_BASIS",
  "FAIL_CLOSED_ALLOWLIST_PROJECTION",
  "REDACTION_PROOF",
  "TRUSTED_EXPORT_AND_VERIFICATION_CLOCK",
  "SOURCE_TO_CANDIDATE_LINEAGE_DIGEST",
  "RETENTION_EXECUTOR",
  "VERIFIABLE_DELETION_RECEIPT",
  "INDEPENDENT_HUMAN_LABEL_WORKFLOW",
  "DEDICATED_OFFLINE_SINK",
  "HOST_PROJECTION_SEMANTIC_MAPPING",
  "SECURITY_AND_PRIVACY_REVIEW",
];
const expectedGateStatuses = Object.fromEntries(
  expectedGates.map((gate) => [gate, "UNRESOLVED"]),
);
expectedGateStatuses.VERSIONED_STRUCTURED_SOURCE_SCHEMA = "SPECIFIED_NOT_IMPLEMENTED";
expectedGateStatuses.AUTHORITATIVE_FACT_PRODUCERS =
  "RESPONSIBILITIES_SPECIFIED_NOT_IMPLEMENTED";
expectedGateStatuses.EXACT_INGRESS_IDENTITY_AND_TIME_SEMANTICS =
  "SPECIFIED_NOT_IMPLEMENTED";

function fail(message) {
  throw new Error(`SOURCE_READINESS_INVALID: ${message}`);
}

function assertExactKeys(value, expected, label) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    fail(`${label} must be an object`);
  }
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (JSON.stringify(actual) !== JSON.stringify(wanted)) {
    fail(`${label} keys differ: ${actual.join(",")}`);
  }
}

assertExactKeys(review, expectedTopLevel, "review");
if (review.schema_id !== "idr.aegis.source-readiness-review.v1" || review.schema_version !== 1) {
  fail("schema identity mismatch");
}

assertExactKeys(review.reviewed_host, ["repository", "commit", "tree"], "reviewed_host");
if (!/^[a-f0-9]{40}$/.test(review.reviewed_host.commit)) fail("invalid reviewed commit");
if (!/^[a-f0-9]{40}$/.test(review.reviewed_host.tree)) fail("invalid reviewed tree");

assertExactKeys(
  review.scope,
  [
    "review_scope",
    "real_data_access",
    "database_access",
    "network_access",
    "production_code_changes",
    "working_tree_content_included",
  ],
  "scope",
);
if (
  review.scope.review_scope !== "TRACKED_SOURCE_ONLY" ||
  review.scope.real_data_access !== "NONE" ||
  review.scope.database_access !== "NONE" ||
  review.scope.network_access !== "NONE" ||
  review.scope.production_code_changes !== "NONE" ||
  review.scope.working_tree_content_included !== false
) {
  fail("review scope must remain source-only and non-mutating");
}

assertExactKeys(
  review.decision,
  [
    "source_data_flow_review",
    "representative_exporter",
    "representative_data_access",
    "passive_capture",
    "live_mirror",
    "authorized_corpus_sources",
  ],
  "decision",
);
if (
  review.decision.source_data_flow_review !== "COMPLETE" ||
  review.decision.representative_exporter !== "BLOCKED" ||
  review.decision.representative_data_access !== "NOT_AUTHORIZED" ||
  review.decision.passive_capture !== "NOT_AUTHORIZED" ||
  review.decision.live_mirror !== "NOT_AUTHORIZED" ||
  JSON.stringify(review.decision.authorized_corpus_sources) !== JSON.stringify(["synthetic_control"])
) {
  fail("decision must keep every real-data path blocked");
}

if (!Array.isArray(review.reviewed_files) || review.reviewed_files.length < 10) {
  fail("reviewed_files is incomplete");
}
const filePaths = new Set();
for (const file of review.reviewed_files) {
  assertExactKeys(file, ["path", "sha256", "evidence_lines"], `reviewed_file:${file.path}`);
  if (filePaths.has(file.path)) fail(`duplicate reviewed file ${file.path}`);
  filePaths.add(file.path);
  if (!/^[a-f0-9]{64}$/.test(file.sha256)) fail(`invalid digest for ${file.path}`);
  if (!Array.isArray(file.evidence_lines) || file.evidence_lines.length === 0) {
    fail(`missing evidence lines for ${file.path}`);
  }
}

const sourceIds = new Set();
const allowedSourceStatuses = new Set([
  "CANDIDATE_ONLY",
  "PROHIBITED_RAW_SOURCE",
  "TYPE_ONLY_NOT_WIRED",
  "UNRELATED_EXPORT",
  "SYNTHETIC_OR_PREBUILT_ONLY",
]);
for (const source of review.source_surfaces) {
  assertExactKeys(source, ["id", "status", "evidence_refs", "finding"], `source:${source.id}`);
  if (sourceIds.has(source.id)) fail(`duplicate source surface ${source.id}`);
  sourceIds.add(source.id);
  if (!allowedSourceStatuses.has(source.status)) fail(`unapproved source status ${source.status}`);
  if (!Array.isArray(source.evidence_refs) || source.evidence_refs.length === 0) {
    fail(`source ${source.id} lacks evidence refs`);
  }
}

const flattened = [];
const allowedFieldStatuses = new Set(["CANDIDATE_ONLY", "NOT_FOUND", "SYNTHETIC_ONLY"]);
for (const group of review.field_groups) {
  assertExactKeys(group, ["status", "source_surface", "target_paths", "reason"], "field_group");
  if (!allowedFieldStatuses.has(group.status)) fail(`unapproved field status ${group.status}`);
  if (group.source_surface !== null && !sourceIds.has(group.source_surface)) {
    fail(`unknown source surface ${group.source_surface}`);
  }
  if (!Array.isArray(group.target_paths) || group.target_paths.length === 0) {
    fail("empty field group");
  }
  flattened.push(...group.target_paths);
}

const uniquePaths = new Set(flattened);
if (uniquePaths.size !== flattened.length) fail("duplicate allowlisted target path");
const actualPaths = [...uniquePaths].sort();
const wantedPaths = [...expectedPaths].sort();
if (JSON.stringify(actualPaths) !== JSON.stringify(wantedPaths)) {
  fail(`allowlisted target coverage mismatch: got ${actualPaths.length}, expected ${wantedPaths.length}`);
}

const candidatePaths = review.field_groups
  .filter((group) => group.status === "CANDIDATE_ONLY")
  .flatMap((group) => group.target_paths)
  .sort();
const expectedCandidatePaths = [
  "assessment_request.input.correlation_ref",
  "assessment_request.input.event_id",
  "assessment_request.input.source_actor",
  "observation_ref",
];
if (JSON.stringify(candidatePaths) !== JSON.stringify(expectedCandidatePaths)) {
  fail("candidate-only field set changed without a new review");
}

const gates = review.required_gates.map((gate) => {
  assertExactKeys(gate, ["gate", "status"], `gate:${gate.gate}`);
  if (gate.status !== expectedGateStatuses[gate.gate]) {
    fail(`gate ${gate.gate} has unexpected status ${gate.status}`);
  }
  return gate.gate;
});
if (new Set(gates).size !== gates.length) fail("duplicate required gate");
if (JSON.stringify([...gates].sort()) !== JSON.stringify([...expectedGates].sort())) {
  fail("required gate coverage mismatch");
}

console.log(`SOURCE_READINESS_REVIEW=PASS fields=${expectedPaths.length} gates=${expectedGates.length}`);
console.log("VERSIONED_STRUCTURED_SOURCE_SCHEMA=SPECIFIED_NOT_IMPLEMENTED");
console.log(
  "AUTHORITATIVE_FACT_PRODUCERS=RESPONSIBILITIES_SPECIFIED_NOT_IMPLEMENTED",
);
console.log(
  "EXACT_INGRESS_IDENTITY_AND_TIME_SEMANTICS=SPECIFIED_NOT_IMPLEMENTED",
);
console.log("REPRESENTATIVE_EXPORTER=BLOCKED");
console.log("AUTHORIZED_CORPUS_SOURCE=SYNTHETIC_CONTROL_ONLY");
