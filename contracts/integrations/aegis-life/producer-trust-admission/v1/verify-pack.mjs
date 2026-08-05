#!/usr/bin/env node

import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(fileURLToPath(import.meta.url));
const repositoryRoot = resolve(root, "../../../../..");
const DIGEST = /^sha256:[0-9a-f]{64}$/;
const REF_TOKEN = /^[a-z0-9][a-z0-9._:/-]{2,159}$/;
const GIT_SHA = /^[0-9a-f]{40}$/;

const PROFILE_DOMAIN = "idr:aegis:producer-trust-profile:v1";
const POLICY_DOMAIN = "idr:aegis:producer-trust-policy:v1";
const GATE_DOMAIN = "idr:aegis:producer-implementation-gate:v1";
const REGISTRY_DOMAIN = "idr:aegis:producer-trust-registry:v1";

const PROHIBITED_FIELDS = new Set([
  "api_key",
  "content",
  "conversation",
  "embedding",
  "goal_text",
  "graph_memory",
  "human_model",
  "message",
  "output",
  "password",
  "personality",
  "prompt",
  "psychological_profile",
  "raw_text",
  "secret",
  "token",
  "vector",
]);

const PROHIBITED_INPUTS = [
  "idr_assessment",
  "idr_decision",
  "shadow_comparison_result",
  "evaluation_label",
  "chosen_action",
  "execution_result",
  "goal_outcome",
  "journal_record",
  "persistence_snapshot",
  "trace_diagnostic",
  "provider_response",
  "arbitrary_json",
  "offline_corpus_feedback",
];

const REQUIRED_PROOFS = [
  "implementation_identity",
  "policy_artifact_identity",
  "receipt_lineage",
  "exact_fact_coverage",
  "prohibited_input_non_use",
  "fail_closed_behavior",
  "reproducibility_bounds",
  "security_privacy_isolation",
  "revocation_rollback",
  "cross_language_conformance",
];

const FAMILY_ORDER = [
  "semantic_roles",
  "fast_path_facts",
  "decision_facts",
  "coordination_facts",
  "host_projection",
];

const FAMILY_RULES = Object.freeze({
  semantic_roles: {
    producerKind: "semantic_classifier",
    ownerBoundary: "semantic_ingress",
    facts: [
      "semantic_roles.primary_semantic_role",
      "semantic_roles.semantic_roles",
    ],
    allowedInputs: [
      "ingress_receipt",
      "ephemeral_user_input",
      "locale_context",
      "semantic_taxonomy",
    ],
    rawContentAccess: "ephemeral_ingress_only",
    derivationClasses: [
      "deterministic",
      "governed_model",
      "human_reviewed",
      "mixed",
    ],
    modelGovernanceRequired: true,
  },
  fast_path_facts: {
    producerKind: "command_policy_classifier",
    ownerBoundary: "command_policy",
    facts: [
      "fast_path_facts.deterministic_command_match",
      "fast_path_facts.required_parameters_complete",
      "fast_path_facts.ambiguity_present",
      "fast_path_facts.authority_context_valid",
      "fast_path_facts.policy_allows_request",
      "fast_path_facts.impact_level",
      "fast_path_facts.reversible",
      "fast_path_facts.depends_on_human_model",
    ],
    allowedInputs: [
      "ingress_receipt",
      "ephemeral_user_input",
      "authority_context",
      "command_registry",
      "parameter_validation_result",
      "tenant_policy_context",
    ],
    rawContentAccess: "ephemeral_ingress_only",
    derivationClasses: ["deterministic", "governed_model", "mixed"],
    modelGovernanceRequired: true,
  },
  decision_facts: {
    producerKind: "decision_context_classifier",
    ownerBoundary: "decision_context",
    facts: [
      "decision_facts.multiple_viable_options",
      "decision_facts.material_tradeoffs",
      "decision_facts.evidence_conflict",
      "decision_facts.impact_level",
      "decision_facts.irreversible_result",
      "decision_facts.affects_long_term_goal",
      "decision_facts.host_user_interest_conflict",
      "decision_facts.simple_lookup",
      "decision_facts.unique_legal_operation",
      "decision_facts.user_choice_already_explicit",
    ],
    allowedInputs: [
      "ingress_receipt",
      "semantic_role_assertion",
      "structured_decision_context",
      "evidence_conflict_summary",
      "tenant_policy_context",
    ],
    rawContentAccess: "none",
    derivationClasses: [
      "deterministic",
      "governed_model",
      "human_reviewed",
      "mixed",
    ],
    modelGovernanceRequired: true,
  },
  coordination_facts: {
    producerKind: "coordination_planner",
    ownerBoundary: "coordination_planning",
    facts: [
      "coordination_facts.response_planned",
      "coordination_facts.action_planned",
      "coordination_facts.explicit_confirmation_requested",
      "coordination_facts.authorization_required",
      "coordination_facts.action_is_read_only",
      "coordination_facts.long_running",
      "coordination_facts.stream_progress",
      "coordination_facts.safe_to_parallelize",
    ],
    allowedInputs: [
      "ingress_receipt",
      "semantic_role_assertion",
      "decision_fact_assertion",
      "structured_coordination_context",
      "authority_context",
      "tenant_policy_context",
    ],
    rawContentAccess: "none",
    derivationClasses: ["deterministic", "governed_model", "mixed"],
    modelGovernanceRequired: true,
  },
  host_projection: {
    producerKind: "aegis_host_decision_runtime",
    ownerBoundary: "host_decision_runtime",
    facts: [
      "host_projection.coordination_mode",
      "host_projection.action_posture",
      "host_projection.next_run_state",
    ],
    allowedInputs: [
      "ingress_receipt",
      "semantic_role_assertion",
      "fast_path_assertion",
      "decision_fact_assertion",
      "coordination_fact_assertion",
      "host_runtime_state",
    ],
    rawContentAccess: "none",
    derivationClasses: ["deterministic"],
    modelGovernanceRequired: false,
  },
});

const GATE_CHECKS = [
  "responsibility_contract_locked",
  "source_owner_approved",
  "implementation_registered",
  "implementation_tree_pinned",
  "policy_and_model_artifacts_pinned",
  "trusted_logical_clock_verified",
  "receipt_lineage_verified",
  "exact_fact_coverage_verified",
  "no_circular_derivation_verified",
  "failure_isolation_verified",
  "security_privacy_review_passed",
  "revocation_rollback_tested",
  "cross_language_conformance_passed",
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

function sameArray(actual, expected) {
  return (
    Array.isArray(actual) &&
    actual.length === expected.length &&
    actual.every((value, index) => value === expected[index])
  );
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

function policyDigest(policy) {
  return digest(POLICY_DOMAIN, policy, "policy_digest");
}

function profileDigest(profile) {
  return digest(`${PROFILE_DOMAIN}:${profile.family}`, profile, "profile_digest");
}

function gateDigest(gate) {
  return digest(GATE_DOMAIN, gate, "gate_digest");
}

function registryDigest(registry) {
  return digest(REGISTRY_DOMAIN, registry, "registry_digest");
}

function seal(registry) {
  registry.trust_policy.policy_digest = policyDigest(registry.trust_policy);
  for (const profile of registry.producers) {
    profile.profile_digest = profileDigest(profile);
  }
  registry.implementation_gate.gate_digest = gateDigest(registry.implementation_gate);
  registry.registry_digest = registryDigest(registry);
  return registry;
}

async function validateTargetSchema(target) {
  assertExactKeys(target, [
    "schema_id",
    "schema_version",
    "schema_path",
    "schema_digest",
    "assertion_binding_mode",
  ]);
  if (
    target.schema_id !== "idr.aegis.structured-observation" ||
    target.schema_version !== 1 ||
    target.schema_path !==
      "contracts/integrations/aegis-life/structured-observation/v1/idr-aegis-structured-observation-v1.schema.json" ||
    target.assertion_binding_mode !== "exact_receipt_digest" ||
    !DIGEST.test(target.schema_digest)
  ) {
    deny("TARGET_SCHEMA_BINDING_MISMATCH");
  }
  const schemaBytes = await readFile(join(repositoryRoot, target.schema_path));
  if (bytesDigest(schemaBytes) !== target.schema_digest) {
    deny("TARGET_SCHEMA_DIGEST_MISMATCH");
  }
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

function validatePolicy(policy) {
  assertExactKeys(policy, ["policy_ref", "policy_version", "policy_digest"]);
  if (
    policy.policy_ref !== "policy:idr-aegis-producer-trust-admission" ||
    policy.policy_version !== "v1" ||
    !REF_TOKEN.test(policy.policy_ref) ||
    !DIGEST.test(policy.policy_digest)
  ) {
    deny("POLICY_BINDING_MISMATCH");
  }
  if (policyDigest(policy) !== policy.policy_digest) deny("POLICY_DIGEST_MISMATCH");
}

function validateLifecycle(lifecycle) {
  assertExactKeys(lifecycle, [
    "status",
    "created_at_logical_time",
    "valid_from_logical_time",
    "valid_until_logical_time",
  ]);
  if (lifecycle.status !== "SPECIFICATION_ONLY") deny("LIFECYCLE_MISMATCH");
  if (
    lifecycle.created_at_logical_time > lifecycle.valid_from_logical_time ||
    lifecycle.valid_from_logical_time >= lifecycle.valid_until_logical_time
  ) {
    deny("LIFECYCLE_MISMATCH");
  }
}

function validateResponsibility(responsibility, rule, allOwned) {
  assertExactKeys(responsibility, [
    "owned_fact_paths",
    "required_output_mode",
    "receipt_binding",
    "failure_mode",
    "independence_rule",
  ]);
  if (
    responsibility.required_output_mode !== "receipt_bound_assertion" ||
    responsibility.receipt_binding !== "exact_receipt_digest" ||
    responsibility.failure_mode !== "deny_assertion" ||
    responsibility.independence_rule !== "no_idr_or_downstream_feedback"
  ) {
    deny("RESPONSIBILITY_MISMATCH");
  }
  if (!Array.isArray(responsibility.owned_fact_paths)) {
    deny("FACT_OWNERSHIP_MISMATCH");
  }
  if (new Set(responsibility.owned_fact_paths).size !== responsibility.owned_fact_paths.length) {
    deny("DUPLICATE_FACT_OWNERSHIP");
  }
  if (!sameArray(responsibility.owned_fact_paths, rule.facts)) {
    deny("FACT_OWNERSHIP_MISMATCH");
  }
  for (const path of responsibility.owned_fact_paths) {
    if (allOwned.has(path)) deny("DUPLICATE_FACT_OWNERSHIP");
    allOwned.add(path);
  }
}

function validateInputPolicy(policy, rule) {
  assertExactKeys(policy, [
    "allowed_input_classes",
    "prohibited_input_classes",
    "raw_content_access",
    "raw_content_persistence",
    "unstructured_journal_read",
    "arbitrary_json_input",
  ]);
  if (
    policy.raw_content_access !== rule.rawContentAccess ||
    policy.raw_content_persistence !== "forbidden"
  ) {
    deny("RAW_CONTENT_POLICY_MISMATCH");
  }
  if (
    policy.unstructured_journal_read !== "forbidden" ||
    policy.arbitrary_json_input !== "forbidden" ||
    !sameArray(policy.allowed_input_classes, rule.allowedInputs) ||
    !sameArray(policy.prohibited_input_classes, PROHIBITED_INPUTS)
  ) {
    deny("INPUT_POLICY_MISMATCH");
  }
  for (const value of policy.allowed_input_classes) {
    if (PROHIBITED_INPUTS.includes(value)) deny("INPUT_POLICY_MISMATCH");
  }
}

function validateDerivationPolicy(policy, rule) {
  assertExactKeys(policy, [
    "allowed_derivation_classes",
    "model_governance_required",
    "reproducibility_required",
    "idr_output_used",
    "downstream_feedback_used",
  ]);
  if (policy.idr_output_used || policy.downstream_feedback_used) {
    deny("CIRCULAR_DERIVATION");
  }
  if (
    !sameArray(policy.allowed_derivation_classes, rule.derivationClasses) ||
    policy.model_governance_required !== rule.modelGovernanceRequired ||
    policy.reproducibility_required !== true
  ) {
    deny("DERIVATION_POLICY_MISMATCH");
  }
}

function validateTrustState(state) {
  assertExactKeys(state, ["status", "reason_code", "last_review_logical_time"]);
  if (
    state.status !== "NOT_IMPLEMENTED" ||
    state.reason_code !== "NO_IMPLEMENTATION_OR_ADMISSION_PACKET"
  ) {
    deny("TRUST_STATE_NOT_ALLOWED");
  }
}

function validateProfile(profile, expectedFamily, allOwned) {
  assertExactKeys(profile, [
    "family",
    "producer_kind",
    "owner_boundary",
    "responsibility",
    "input_policy",
    "derivation_policy",
    "required_proofs",
    "trust_state",
    "profile_digest",
  ]);
  const rule = FAMILY_RULES[profile.family];
  if (!rule || profile.family !== expectedFamily) deny("PRODUCER_FAMILY_MISMATCH");
  if (profile.producer_kind !== rule.producerKind) deny("PRODUCER_KIND_MISMATCH");
  if (profile.owner_boundary !== rule.ownerBoundary) deny("OWNER_BOUNDARY_MISMATCH");
  validateResponsibility(profile.responsibility, rule, allOwned);
  validateInputPolicy(profile.input_policy, rule);
  validateDerivationPolicy(profile.derivation_policy, rule);
  if (!sameArray(profile.required_proofs, REQUIRED_PROOFS)) {
    deny("REQUIRED_PROOFS_MISMATCH");
  }
  validateTrustState(profile.trust_state);
  if (!DIGEST.test(profile.profile_digest)) deny("PROFILE_DIGEST_MISMATCH");
  if (profileDigest(profile) !== profile.profile_digest) deny("PROFILE_DIGEST_MISMATCH");
}

function validateProfiles(profiles) {
  if (!Array.isArray(profiles) || profiles.length !== FAMILY_ORDER.length) {
    deny("PRODUCER_SET_MISMATCH");
  }
  const families = profiles.map((profile) => profile.family);
  if (new Set(families).size !== families.length) deny("DUPLICATE_FAMILY");
  const allOwned = new Set();
  profiles.forEach((profile, index) => validateProfile(profile, FAMILY_ORDER[index], allOwned));
  const expectedFacts = FAMILY_ORDER.flatMap((family) => FAMILY_RULES[family].facts);
  if (allOwned.size !== expectedFacts.length) deny("FACT_OWNERSHIP_MISMATCH");
  for (const path of expectedFacts) {
    if (!allOwned.has(path)) deny("FACT_OWNERSHIP_MISMATCH");
  }
  return allOwned.size;
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
    gate.decision_reason !== "PRODUCERS_NOT_IMPLEMENTED_OR_ADMITTED"
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
  if (new Set(names).size !== names.length || !sameArray(names, GATE_CHECKS)) {
    deny("GATE_CHECKS_MISMATCH");
  }
  gate.checks.forEach((check, index) => {
    assertExactKeys(check, ["check", "status", "evidence_refs"]);
    const expectedStatus = index === 0 ? "SATISFIED" : "UNSATISFIED";
    if (check.status === "UNSATISFIED" && check.evidence_refs.length !== 0) {
      deny("GATE_EVIDENCE_MISMATCH");
    }
    if (check.status !== expectedStatus) deny("GATE_STATUS_MISMATCH");
    if (
      index === 0 &&
      !sameArray(check.evidence_refs, ["spec:producer-responsibility-v1"])
    ) {
      deny("GATE_EVIDENCE_MISMATCH");
    }
    if (!Array.isArray(check.evidence_refs)) deny("GATE_EVIDENCE_MISMATCH");
    for (const evidenceRef of check.evidence_refs) {
      if (!REF_TOKEN.test(evidenceRef)) deny("GATE_EVIDENCE_MISMATCH");
    }
  });
  if (!DIGEST.test(gate.gate_digest)) deny("GATE_DIGEST_MISMATCH");
  if (gateDigest(gate) !== gate.gate_digest) deny("GATE_DIGEST_MISMATCH");
  return gate.checks.filter((check) => check.status === "SATISFIED").length;
}

async function validateRegistry(registry) {
  assertCanonicalValue(registry);
  scanForbidden(registry);
  assertExactKeys(registry, [
    "schema_id",
    "schema_version",
    "registry_ref",
    "target_observation_schema",
    "source_review_pin",
    "trust_policy",
    "lifecycle",
    "producers",
    "implementation_gate",
    "registry_digest",
  ]);
  if (
    registry.schema_id !== "idr.aegis.producer-trust-registry" ||
    registry.schema_version !== 1 ||
    !REF_TOKEN.test(registry.registry_ref)
  ) {
    deny("REGISTRY_IDENTITY_MISMATCH");
  }
  await validateTargetSchema(registry.target_observation_schema);
  validateSourcePin(registry.source_review_pin);
  validatePolicy(registry.trust_policy);
  validateLifecycle(registry.lifecycle);
  const ownedFactCount = validateProfiles(registry.producers);
  const satisfiedGateCount = validateGate(registry.implementation_gate, registry.lifecycle);
  if (!DIGEST.test(registry.registry_digest)) deny("REGISTRY_DIGEST_MISMATCH");
  if (registryDigest(registry) !== registry.registry_digest) {
    deny("REGISTRY_DIGEST_MISMATCH");
  }
  return { ownedFactCount, satisfiedGateCount };
}

function mutate(registry, mutation) {
  switch (mutation) {
    case "TOP_UNKNOWN_FIELD":
      registry.unreviewed = true;
      break;
    case "DUPLICATE_FAMILY":
      registry.producers[4].family = "semantic_roles";
      break;
    case "PRODUCER_KIND_MISMATCH":
      registry.producers[0].producer_kind = "coordination_planner";
      break;
    case "OWNER_BOUNDARY_MISMATCH":
      registry.producers[0].owner_boundary = "decision_context";
      break;
    case "MISSING_FACT_OWNERSHIP":
      registry.producers[2].responsibility.owned_fact_paths.pop();
      break;
    case "CROSS_FAMILY_FACT_OWNERSHIP":
      registry.producers[0].responsibility.owned_fact_paths[1] =
        "decision_facts.material_tradeoffs";
      break;
    case "DUPLICATE_FACT_OWNERSHIP":
      registry.producers[4].responsibility.owned_fact_paths[2] =
        "host_projection.coordination_mode";
      break;
    case "IDR_INPUT_ALLOWED":
      registry.producers[2].input_policy.allowed_input_classes.push("idr_assessment");
      break;
    case "SHADOW_FEEDBACK_ALLOWED":
      registry.producers[4].input_policy.allowed_input_classes.push(
        "shadow_comparison_result",
      );
      break;
    case "RAW_CONTENT_ACCESS_WIDENED":
      registry.producers[2].input_policy.raw_content_access = "ephemeral_ingress_only";
      break;
    case "RAW_CONTENT_PERSISTENCE":
      registry.producers[0].input_policy.raw_content_persistence = "allowed";
      break;
    case "JOURNAL_READ_ALLOWED":
      registry.producers[1].input_policy.unstructured_journal_read = "allowed";
      break;
    case "FAIL_OPEN":
      registry.producers[3].responsibility.failure_mode = "best_effort";
      break;
    case "RECEIPT_BINDING_WEAKENED":
      registry.producers[1].responsibility.receipt_binding = "receipt_id_only";
      break;
    case "IDR_OUTPUT_USED":
      registry.producers[4].derivation_policy.idr_output_used = true;
      break;
    case "DOWNSTREAM_FEEDBACK_USED":
      registry.producers[0].derivation_policy.downstream_feedback_used = true;
      break;
    case "ADMITTED_WITHOUT_PACKET":
      registry.producers[0].trust_state.status = "ADMITTED";
      break;
    case "MISSING_REQUIRED_PROOF":
      registry.producers[1].required_proofs.pop();
      break;
    case "POLICY_DIGEST_SUBSTITUTION":
      registry.trust_policy.policy_digest = flipDigest(registry.trust_policy.policy_digest);
      break;
    case "PROFILE_DIGEST_SUBSTITUTION":
      registry.producers[2].profile_digest = flipDigest(
        registry.producers[2].profile_digest,
      );
      break;
    case "SOURCE_PIN_DRIFT":
      registry.source_review_pin.tree_sha =
        "0a76b01653bb194e4f192e5768b32a38fd3b2173";
      break;
    case "MISSING_GATE_CHECK":
      registry.implementation_gate.checks.pop();
      break;
    case "UNSATISFIED_WITH_EVIDENCE":
      registry.implementation_gate.checks[1].evidence_refs.push("evidence:unverified");
      break;
    case "GATE_FALSE_READY":
      registry.implementation_gate.decision = "READY";
      break;
    case "GATE_DIGEST_SUBSTITUTION":
      registry.implementation_gate.gate_digest = flipDigest(
        registry.implementation_gate.gate_digest,
      );
      break;
    case "REGISTRY_DIGEST_SUBSTITUTION":
      registry.registry_digest = flipDigest(registry.registry_digest);
      break;
    default:
      throw new Error(`unknown mutation ${mutation}`);
  }
  return registry;
}

const manifest = await load("manifest.json");
const registrySchemaBytes = await readFile(join(root, manifest.schema));
if (bytesDigest(registrySchemaBytes) !== manifest.schema_digest) {
  deny("MANIFEST_SCHEMA_DIGEST_MISMATCH");
}
const golden = await load(manifest.golden.path);

if (process.argv.includes("--print-digests")) {
  const sealed = seal(clone(golden));
  console.log(
    JSON.stringify(
      {
        policy: sealed.trust_policy.policy_digest,
        profiles: Object.fromEntries(
          sealed.producers.map((profile) => [profile.family, profile.profile_digest]),
        ),
        gate: sealed.implementation_gate.gate_digest,
        registry: sealed.registry_digest,
      },
      null,
      2,
    ),
  );
  process.exit(0);
}

const cases = [];
let failed = 0;
let goldenResult;

try {
  goldenResult = await validateRegistry(golden);
  if (golden.trust_policy.policy_digest !== manifest.golden.expected_policy_digest) {
    deny("MANIFEST_POLICY_DIGEST_MISMATCH");
  }
  for (const profile of golden.producers) {
    if (
      profile.profile_digest !==
      manifest.golden.expected_profile_digests[profile.family]
    ) {
      deny("MANIFEST_PROFILE_DIGEST_MISMATCH");
    }
  }
  if (golden.implementation_gate.gate_digest !== manifest.golden.expected_gate_digest) {
    deny("MANIFEST_GATE_DIGEST_MISMATCH");
  }
  if (golden.registry_digest !== manifest.golden.expected_registry_digest) {
    deny("MANIFEST_REGISTRY_DIGEST_MISMATCH");
  }
  if (goldenResult.ownedFactCount !== manifest.golden.expected_owned_fact_count) {
    deny("MANIFEST_FACT_COUNT_MISMATCH");
  }
  if (
    goldenResult.satisfiedGateCount !==
    manifest.golden.expected_satisfied_gate_count
  ) {
    deny("MANIFEST_GATE_COUNT_MISMATCH");
  }
  cases.push(manifest.golden.id);
} catch (error) {
  failed += 1;
  cases.push(`${manifest.golden.id}:FAILED:${error.code ?? error.message}`);
}

for (const testCase of manifest.negative_cases) {
  try {
    const candidate = mutate(clone(golden), testCase.mutation);
    await validateRegistry(candidate);
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
  producer_count: golden.producers.length,
  owned_fact_count: goldenResult?.ownedFactCount ?? null,
  satisfied_implementation_checks: goldenResult?.satisfiedGateCount ?? null,
  passed: cases.length - failed,
  failed,
  cases,
  authoritative_fact_producers: "RESPONSIBILITIES_SPECIFIED_NOT_IMPLEMENTED",
  producer_implementation_gate: "BLOCKED",
  representative_exporter: "BLOCKED",
  real_data_access: "NOT_AUTHORIZED",
};

console.log(JSON.stringify(result, null, 2));
if (failed !== 0) process.exit(1);
