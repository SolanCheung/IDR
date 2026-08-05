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

const GATE_DOMAIN = "idr:aegis:ingress-receipt-implementation-gate:v1";
const CONTRACT_DOMAIN = "idr:aegis:ingress-receipt-source-contract:v1";

const PROHIBITED_FIELDS = new Set([
  "api_key",
  "conversation",
  "embedding",
  "goal_text",
  "graph_memory",
  "human_model",
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

const EXPECTED_IDENTITY = {
  uuid_version: 4,
  canonical_format: "lowercase_hyphenated",
  mutually_distinct_fields: ["event_id", "run_id", "turn_id", "input_receipt_id"],
  collision_policy: "reject_and_regenerate_before_publish",
  published_identity_mutation: "forbidden",
  field_rules: [
    {
      field: "event_id",
      namespace: "aegis:event",
      source: "cryptographic_uuid_v4_at_ingress",
      cardinality: "one_per_accepted_event",
      reuse_policy: "never",
      missing_or_invalid: "REJECT_RECEIPT",
    },
    {
      field: "run_id",
      namespace: "aegis:run",
      source: "typed_host_run_context",
      cardinality: "one_per_host_run",
      reuse_policy: "same_run_only",
      missing_or_invalid: "REJECT_RECEIPT",
    },
    {
      field: "turn_id",
      namespace: "aegis:turn",
      source: "typed_interaction_turn_context",
      cardinality: "one_per_interaction_turn",
      reuse_policy: "same_turn_only",
      missing_or_invalid: "REJECT_RECEIPT",
    },
    {
      field: "input_receipt_id",
      namespace: "aegis:input-receipt",
      source: "cryptographic_uuid_v4_at_receipt_seal",
      cardinality: "one_per_sealed_receipt",
      reuse_policy: "never",
      missing_or_invalid: "REJECT_RECEIPT",
    },
  ],
};

const EXPECTED_ACTOR = {
  source_actor_source: "authenticated_typed_origin",
  source_actor_taxonomy_ref: "idr:v1.3:source-actor",
  free_form_actor_classification: "forbidden",
  actor_ref_source: "tenant_scoped_pseudonymization",
  actor_ref_namespace: "aegis:actor",
  direct_identifier_allowed: false,
  unresolved_actor: "REJECT_RECEIPT",
};

const EXPECTED_CONTENT = {
  content_ref_source: "fresh_opaque_reference",
  content_ref_namespace: "aegis:content",
  url_or_path_reference_allowed: false,
  content_digest_algorithm: "sha256",
  digest_preimage: "exact_received_utf8_bytes",
  digest_stage: "controller_boundary_before_downstream_processing",
  unicode_normalization: "none",
  raw_content_in_receipt: "forbidden",
  raw_content_persistence: "not_authorized",
  raw_content_export: "forbidden",
  raw_content_telemetry: "forbidden",
  hash_failure: "REJECT_RECEIPT",
};

const EXPECTED_CORRELATION = {
  source: "authenticated_typed_upstream_context",
  namespace: "aegis:correlation",
  meaning: "nonsemantic_grouping_only",
  fallback: "fresh_per_input_reference",
  reuse_policy: "explicit_upstream_group_only",
  goal_text_derivation: false,
  semantic_similarity_derivation: false,
  idr_output_derivation: false,
  authority_semantics: "none",
};

const EXPECTED_LOGICAL_TIME = {
  clock_kind: "externally_injected_monotonic_logical_clock",
  clock_ref: "clock:aegis-ingress-logical",
  epoch_ref: "epoch:aegis-ingress-v1",
  scope: "source_system",
  sequence_rule: "strictly_increasing_per_source_system",
  allocation_stage: "before_event_publish",
  atomic_with_receipt_seal: true,
  wall_clock_authoritative: false,
  journal_time_authoritative: false,
  trace_tick_authoritative: false,
  rollback_rule: "reject_and_require_new_approved_epoch",
  restart_rule: "restore_verified_anchor_or_reject",
  exhaustion_rule: "reject_and_require_new_approved_epoch",
  clock_failure: "REJECT_RECEIPT",
  export_verification_clock: "separate_unresolved_gate",
};

const EXPECTED_RECEIPT_DIGEST = {
  algorithm: "sha256",
  domain: "idr:aegis:structured-observation-receipt:v1",
  delimiter: "nul_byte",
  encoding: "utf8",
  canonicalization: "canonical_json",
  covered_fields: [
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
  ],
  excluded_fields: ["receipt_digest"],
};

const EXPECTED_SOURCE_BINDING = {
  schema_id: "idr.aegis.receipt-source-binding",
  schema_version: 1,
  status: "SPECIFIED_NOT_IMPLEMENTED",
  required_fields: [
    "schema_id",
    "schema_version",
    "binding_ref",
    "receipt_digest",
    "source_contract_digest",
    "source_review_tree",
    "owner_ref",
    "clock_ref",
    "epoch_ref",
    "bound_at_logical_time",
    "binding_digest",
  ],
  digest_domain: "idr:aegis:receipt-source-binding:v1",
  creation_mode: "atomic_with_receipt_acceptance",
  verification_stage: "before_fact_assertion_acceptance",
  missing_or_mismatch: "REJECT_RECEIPT",
};

const GATE_CHECKS = [
  "receipt_source_contract_locked",
  "source_owner_approved",
  "receipt_implementation_registered",
  "ingress_path_wired",
  "cryptographic_uuid_generation_verified",
  "run_context_binding_verified",
  "turn_context_binding_verified",
  "actor_pseudonymization_verified",
  "exact_content_digest_verified",
  "correlation_semantics_verified",
  "monotonic_clock_injection_verified",
  "rollback_restart_recovery_verified",
  "collision_and_atomicity_tested",
  "receipt_source_binding_implemented",
  "security_privacy_review_passed",
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

function contractDigest(contract) {
  return digest(CONTRACT_DOMAIN, contract, "contract_digest");
}

function seal(contract) {
  contract.implementation_gate.gate_digest = gateDigest(contract.implementation_gate);
  contract.contract_digest = contractDigest(contract);
  return contract;
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
    "receipt_definition",
    "receipt_digest_domain",
  ]);
  if (
    target.schema_id !== "idr.aegis.structured-observation" ||
    target.schema_version !== 1 ||
    target.schema_path !==
      "contracts/integrations/aegis-life/structured-observation/v1/idr-aegis-structured-observation-v1.schema.json" ||
    target.receipt_definition !== "IngressReceiptV1" ||
    target.receipt_digest_domain !==
      "idr:aegis:structured-observation-receipt:v1" ||
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
  assertExactKeys(owner, [
    "owner_kind",
    "owner_ref",
    "atomic_acceptance_required",
    "delegation_allowed",
    "trust_state",
    "reason_code",
  ]);
  if (owner.trust_state !== "NOT_IMPLEMENTED") deny("OWNER_NOT_IMPLEMENTED");
  if (
    owner.owner_kind !== "product_ingress_controller" ||
    owner.owner_ref !== "component:aegis-ingress-controller" ||
    !REF_TOKEN.test(owner.owner_ref) ||
    owner.atomic_acceptance_required !== true ||
    owner.delegation_allowed !== false ||
    owner.reason_code !== "NO_APPROVED_OWNER_OR_RECEIPT_SOURCE"
  ) {
    deny("OWNER_BOUNDARY_MISMATCH");
  }
}

function validateExactSection(value, expected, keys, code) {
  assertExactKeys(value, keys);
  if (!same(value, expected)) deny(code);
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
    gate.decision_reason !== "RECEIPT_SOURCE_NOT_IMPLEMENTED_OR_APPROVED"
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
    const expectedEvidence = index === 0 ? ["spec:ingress-receipt-source-v1"] : [];
    if (!same(check.evidence_refs, expectedEvidence)) deny("GATE_EVIDENCE_MISMATCH");
    for (const ref of check.evidence_refs) {
      if (!REF_TOKEN.test(ref)) deny("GATE_EVIDENCE_MISMATCH");
    }
  });
  if (!DIGEST.test(gate.gate_digest) || gateDigest(gate) !== gate.gate_digest) {
    deny("GATE_DIGEST_MISMATCH");
  }
  return gate.checks.filter((check) => check.status === "SATISFIED").length;
}

async function validateContract(contract) {
  assertCanonicalValue(contract);
  scanForbidden(contract);
  assertExactKeys(contract, [
    "schema_id",
    "schema_version",
    "contract_ref",
    "target_receipt_schema",
    "source_review_pin",
    "lifecycle",
    "owner",
    "identity_semantics",
    "actor_semantics",
    "content_semantics",
    "correlation_semantics",
    "logical_time_semantics",
    "receipt_digest_semantics",
    "receipt_source_binding",
    "implementation_gate",
    "contract_digest",
  ]);
  if (
    contract.schema_id !== "idr.aegis.ingress-receipt-source-contract" ||
    contract.schema_version !== 1 ||
    !REF_TOKEN.test(contract.contract_ref)
  ) {
    deny("CONTRACT_IDENTITY_MISMATCH");
  }
  await validateTarget(contract.target_receipt_schema);
  validateSourcePin(contract.source_review_pin);
  validateLifecycle(contract.lifecycle);
  validateOwner(contract.owner);
  validateExactSection(
    contract.identity_semantics,
    EXPECTED_IDENTITY,
    Object.keys(EXPECTED_IDENTITY),
    "IDENTITY_SEMANTICS_MISMATCH",
  );
  validateExactSection(
    contract.actor_semantics,
    EXPECTED_ACTOR,
    Object.keys(EXPECTED_ACTOR),
    "ACTOR_SEMANTICS_MISMATCH",
  );
  validateExactSection(
    contract.content_semantics,
    EXPECTED_CONTENT,
    Object.keys(EXPECTED_CONTENT),
    "CONTENT_SEMANTICS_MISMATCH",
  );
  validateExactSection(
    contract.correlation_semantics,
    EXPECTED_CORRELATION,
    Object.keys(EXPECTED_CORRELATION),
    "CORRELATION_SEMANTICS_MISMATCH",
  );
  if (contract.logical_time_semantics.export_verification_clock !== "separate_unresolved_gate") {
    deny("EXPORT_CLOCK_GATE_MISMATCH");
  }
  validateExactSection(
    contract.logical_time_semantics,
    EXPECTED_LOGICAL_TIME,
    Object.keys(EXPECTED_LOGICAL_TIME),
    "LOGICAL_TIME_SEMANTICS_MISMATCH",
  );
  validateExactSection(
    contract.receipt_digest_semantics,
    EXPECTED_RECEIPT_DIGEST,
    Object.keys(EXPECTED_RECEIPT_DIGEST),
    "RECEIPT_DIGEST_SEMANTICS_MISMATCH",
  );
  validateExactSection(
    contract.receipt_source_binding,
    EXPECTED_SOURCE_BINDING,
    Object.keys(EXPECTED_SOURCE_BINDING),
    "SOURCE_BINDING_MISMATCH",
  );
  const satisfiedGateCount = validateGate(contract.implementation_gate, contract.lifecycle);
  if (!DIGEST.test(contract.contract_digest) || contractDigest(contract) !== contract.contract_digest) {
    deny("CONTRACT_DIGEST_MISMATCH");
  }
  return {
    identityRuleCount: contract.identity_semantics.field_rules.length,
    coveredFieldCount: contract.receipt_digest_semantics.covered_fields.length,
    satisfiedGateCount,
  };
}

function mutate(contract, mutation) {
  switch (mutation) {
    case "TOP_UNKNOWN_FIELD":
      contract.unreviewed = true;
      break;
    case "TARGET_SCHEMA_DIGEST_SUBSTITUTION":
      contract.target_receipt_schema.schema_digest = flipDigest(
        contract.target_receipt_schema.schema_digest,
      );
      break;
    case "SOURCE_PIN_DRIFT":
      contract.source_review_pin.tree_sha = "0a76b01653bb194e4f192e5768b32a38fd3b2173";
      break;
    case "OWNER_PRETENDS_IMPLEMENTED":
      contract.owner.trust_state = "ADMITTED";
      break;
    case "OWNER_DELEGATION_ALLOWED":
      contract.owner.delegation_allowed = true;
      break;
    case "IDENTITY_RULE_MISSING":
      contract.identity_semantics.field_rules.pop();
      break;
    case "IDENTITY_NAMESPACE_REUSE":
      contract.identity_semantics.field_rules[3].namespace = "aegis:event";
      break;
    case "RUN_DERIVED_FROM_GOAL":
      contract.identity_semantics.field_rules[1].source = "goal_text_hash";
      break;
    case "IDENTITY_REUSE_ALLOWED":
      contract.identity_semantics.field_rules[0].reuse_policy = "allowed";
      break;
    case "FREE_FORM_ACTOR":
      contract.actor_semantics.free_form_actor_classification = "allowed";
      break;
    case "DIRECT_ACTOR_IDENTIFIER":
      contract.actor_semantics.direct_identifier_allowed = true;
      break;
    case "CONTENT_REF_URL_ALLOWED":
      contract.content_semantics.url_or_path_reference_allowed = true;
      break;
    case "CONTENT_NORMALIZED_BEFORE_HASH":
      contract.content_semantics.unicode_normalization = "nfc";
      break;
    case "RAW_CONTENT_PERSISTENCE_ALLOWED":
      contract.content_semantics.raw_content_persistence = "allowed";
      break;
    case "CORRELATION_FROM_GOAL_TEXT":
      contract.correlation_semantics.goal_text_derivation = true;
      break;
    case "CORRELATION_HAS_AUTHORITY":
      contract.correlation_semantics.authority_semantics = "dispatch_authority";
      break;
    case "WALL_CLOCK_AUTHORITATIVE":
      contract.logical_time_semantics.wall_clock_authoritative = true;
      break;
    case "NON_STRICT_LOGICAL_TIME":
      contract.logical_time_semantics.sequence_rule = "non_decreasing";
      break;
    case "CLOCK_ROLLBACK_CONTINUES":
      contract.logical_time_semantics.rollback_rule = "continue_from_local_value";
      break;
    case "EXPORT_CLOCK_FALSE_CLOSURE":
      contract.logical_time_semantics.export_verification_clock = "same_clock_approved";
      break;
    case "RECEIPT_DIGEST_FIELD_MISSING":
      contract.receipt_digest_semantics.covered_fields.pop();
      break;
    case "SOURCE_BINDING_FIELD_MISSING":
      contract.receipt_source_binding.required_fields.splice(4, 1);
      break;
    case "SOURCE_BINDING_PRETENDS_IMPLEMENTED":
      contract.receipt_source_binding.status = "IMPLEMENTED";
      break;
    case "MISSING_GATE_CHECK":
      contract.implementation_gate.checks.pop();
      break;
    case "UNSATISFIED_WITH_EVIDENCE":
      contract.implementation_gate.checks[1].evidence_refs.push("evidence:unverified");
      break;
    case "GATE_FALSE_READY":
      contract.implementation_gate.decision = "READY";
      break;
    case "GATE_DIGEST_SUBSTITUTION":
      contract.implementation_gate.gate_digest = flipDigest(
        contract.implementation_gate.gate_digest,
      );
      break;
    case "CONTRACT_DIGEST_SUBSTITUTION":
      contract.contract_digest = flipDigest(contract.contract_digest);
      break;
    default:
      throw new Error(`unknown mutation ${mutation}`);
  }
  return contract;
}

const manifest = await load("manifest.json");
const schemaBytes = await readFile(join(root, manifest.schema));
if (bytesDigest(schemaBytes) !== manifest.schema_digest) {
  deny("MANIFEST_SCHEMA_DIGEST_MISMATCH");
}
const golden = await load(manifest.golden.path);

if (process.argv.includes("--print-digests")) {
  const sealed = seal(clone(golden));
  console.log(
    JSON.stringify(
      {
        gate: sealed.implementation_gate.gate_digest,
        contract: sealed.contract_digest,
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
  goldenResult = await validateContract(golden);
  if (golden.implementation_gate.gate_digest !== manifest.golden.expected_gate_digest) {
    deny("MANIFEST_GATE_DIGEST_MISMATCH");
  }
  if (golden.contract_digest !== manifest.golden.expected_contract_digest) {
    deny("MANIFEST_CONTRACT_DIGEST_MISMATCH");
  }
  if (goldenResult.identityRuleCount !== manifest.golden.expected_identity_rule_count) {
    deny("MANIFEST_IDENTITY_COUNT_MISMATCH");
  }
  if (
    goldenResult.coveredFieldCount !==
    manifest.golden.expected_receipt_covered_field_count
  ) {
    deny("MANIFEST_FIELD_COUNT_MISMATCH");
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
    await validateContract(mutate(clone(golden), testCase.mutation));
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
  identity_rule_count: goldenResult?.identityRuleCount ?? null,
  receipt_covered_field_count: goldenResult?.coveredFieldCount ?? null,
  satisfied_implementation_checks: goldenResult?.satisfiedGateCount ?? null,
  passed: cases.length - failed,
  failed,
  cases,
  ingress_receipt_source_owner: "NOT_APPROVED",
  ingress_receipt_implementation_gate: "BLOCKED",
  trusted_export_and_verification_clock: "SPECIFIED_NOT_IMPLEMENTED",
  representative_exporter: "BLOCKED",
  real_data_access: "NOT_AUTHORIZED",
};

console.log(JSON.stringify(result, null, 2));
if (failed !== 0) process.exit(1);
