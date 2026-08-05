#!/usr/bin/env node

import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(fileURLToPath(import.meta.url));
const repositoryRoot = resolve(root, "../../../../..");
const APPROVAL_DOMAIN = "idr:aegis:representative-export-approval:v1";
const TIME_DOMAIN = "idr:aegis:trusted-current-time-evidence:v1";
const REQUEST_DOMAIN = "idr:aegis:representative-export-verification-request:v1";
const DIGEST = /^sha256:[0-9a-f]{64}$/;
const TOKEN = /^[a-z0-9][a-z0-9._:/-]{2,179}$/;
const NONCE = /^[a-z0-9._:-]{16,128}$/;
const GIT_SHA = /^[0-9a-f]{40}$/;
const SYNTHETIC_CLOCK_STATE = {
  epoch: 1,
  minimum_sequence: 42,
  used_nonces: new Set(["clock-nonce-synthetic-used"]),
};

const APPROVAL_KEYS = ["schema_id", "schema_version", "approval_id", "assurance", "source_pin", "exporter_ref", "purpose", "allowed_source_classes", "allowed_tenant_refs", "allowed_actor_refs", "contract_bindings", "data_policy", "limits", "lifecycle", "revocation_binding", "issuer", "approval_digest"];
const TIME_KEYS = ["schema_id", "schema_version", "assurance", "clock_source_ref", "clock_profile_version", "clock_epoch", "sequence", "purpose", "nonce", "attested_time_ms", "uncertainty_ms", "signature_algorithm", "signature", "time_evidence_digest"];
const REQUEST_KEYS = ["schema_id", "schema_version", "request_id", "request_nonce", "source_class", "source_pin", "exporter_ref", "purpose", "tenant_ref", "actor_ref", "contract_bindings", "requested_candidate_count", "approval_id", "approval_digest", "time_evidence_digest", "revocation_state", "lawful_basis", "requested_uses", "request_digest"];
const CONTRACT_KEYS = ["structured_observation_schema_digest", "ingress_source_contract_digest", "producer_registry_digest", "host_mapping_profile_digest", "representative_intake_schema_digest"];
const POLICY_KEYS = ["allows_raw_content", "allows_credentials", "allows_human_model", "allows_personality_inference", "allows_model_training", "allows_secondary_use", "allows_onward_transfer"];
const USE_KEYS = ["raw_content", "credentials", "human_model", "personality_inference", "model_training", "secondary_use", "onward_transfer"];

class GateError extends Error {
  constructor(code) {
    super(code);
    this.code = code;
  }
}

function deny(code) { throw new GateError(code); }
function object(value) { return value !== null && typeof value === "object" && !Array.isArray(value); }
function clone(value) { return structuredClone(value); }

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
  return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(",")}}`;
}

function same(left, right) { return canonicalJson(left) === canonicalJson(right); }

function digest(domain, value, field) {
  const preimage = { ...value };
  delete preimage[field];
  const hash = createHash("sha256");
  hash.update(domain, "ascii");
  hash.update(Buffer.from([0]));
  hash.update(canonicalJson(preimage), "utf8");
  return `sha256:${hash.digest("hex")}`;
}

function bytesDigest(bytes) { return `sha256:${createHash("sha256").update(bytes).digest("hex")}`; }
function approvalDigest(value) { return digest(APPROVAL_DOMAIN, value, "approval_digest"); }
function timeDigest(value) { return digest(TIME_DOMAIN, value, "time_evidence_digest"); }
function requestDigest(value) { return digest(REQUEST_DOMAIN, value, "request_digest"); }
function sealApproval(value) { value.approval_digest = approvalDigest(value); return value; }
function sealTime(value) { value.time_evidence_digest = timeDigest(value); return value; }
function sealRequest(value) { value.request_digest = requestDigest(value); return value; }

async function load(path) {
  const value = JSON.parse(await readFile(join(root, path), "utf8"));
  assertCanonicalValue(value);
  return value;
}

function exact(value, keys) {
  if (!object(value)) deny("INVALID_OBJECT");
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  if (!same(actual, expected)) deny("UNKNOWN_FIELD");
}

function token(value) { return typeof value === "string" && TOKEN.test(value); }
function safePositive(value) { return Number.isSafeInteger(value) && value > 0; }

function validateSourcePin(pin) {
  exact(pin, ["repository", "commit_sha", "tree_sha"]);
  if (pin.repository !== "Aegis Life" || !GIT_SHA.test(pin.commit_sha) || !GIT_SHA.test(pin.tree_sha) || pin.commit_sha !== "381d14fb1dccfeae691286a39579535ef59f7b4e" || pin.tree_sha !== "da76b01653bb194e4f192e5768b32a38fd3b2173") deny("SOURCE_PIN_MISMATCH");
}

function validateScopeList(list) {
  if (!Array.isArray(list) || list.length === 0 || new Set(list).size !== list.length) deny("INVALID_SCOPE");
  for (const entry of list) {
    if (!token(entry)) deny(entry === "*" || entry === "all" ? "WILDCARD_SCOPE_FORBIDDEN" : "INVALID_SCOPE");
    if (entry === "all" || entry.includes("*")) deny("WILDCARD_SCOPE_FORBIDDEN");
  }
}

async function expectedBindings() {
  const structured = await readFile(join(repositoryRoot, "contracts/integrations/aegis-life/structured-observation/v1/idr-aegis-structured-observation-v1.schema.json"));
  const intake = await readFile(join(repositoryRoot, "contracts/integrations/aegis-life/representative-corpus-intake/v1/idr-aegis-representative-intake-v1.schema.json"));
  const ingress = JSON.parse(await readFile(join(repositoryRoot, "contracts/integrations/aegis-life/ingress-receipt-trust/v1/fixtures/valid/synthetic-ingress-receipt-source-contract-v1.json"), "utf8"));
  const producers = JSON.parse(await readFile(join(repositoryRoot, "contracts/integrations/aegis-life/producer-trust-admission/v1/fixtures/valid/synthetic-producer-trust-registry-v1.json"), "utf8"));
  const host = JSON.parse(await readFile(join(repositoryRoot, "contracts/integrations/aegis-life/host-projection-mapping/v1/fixtures/valid/synthetic-host-projection-mapping-profile-v1.json"), "utf8"));
  return {
    structured_observation_schema_digest: bytesDigest(structured),
    ingress_source_contract_digest: ingress.contract_digest,
    producer_registry_digest: producers.registry_digest,
    host_mapping_profile_digest: host.profile_digest,
    representative_intake_schema_digest: bytesDigest(intake),
  };
}

async function validateBindings(bindings) {
  exact(bindings, CONTRACT_KEYS);
  for (const value of Object.values(bindings)) if (!DIGEST.test(value)) deny("CONTRACT_BINDING_MISMATCH");
  if (!same(bindings, await expectedBindings())) deny("CONTRACT_BINDING_MISMATCH");
}

function validateForbiddenFlags(value, keys) {
  exact(value, keys);
  if (Object.values(value).some((entry) => entry !== false)) deny("PROHIBITED_USE");
}

async function validateApproval(approval) {
  assertCanonicalValue(approval);
  exact(approval, APPROVAL_KEYS);
  if (approval.schema_id !== "idr.aegis.representative-export-approval.v1" || approval.schema_version !== 1 || !token(approval.approval_id) || !token(approval.exporter_ref) || approval.purpose !== "idr_offline_representative_evaluation") deny("APPROVAL_SHAPE_MISMATCH");
  validateSourcePin(approval.source_pin);
  validateScopeList(approval.allowed_tenant_refs);
  validateScopeList(approval.allowed_actor_refs);
  if (!same(approval.allowed_source_classes, ["synthetic_control"])) deny("SYNTHETIC_APPROVAL_SCOPE_VIOLATION");
  await validateBindings(approval.contract_bindings);
  validateForbiddenFlags(approval.data_policy, POLICY_KEYS);
  exact(approval.limits, ["max_candidates_per_request"]);
  if (!safePositive(approval.limits.max_candidates_per_request) || approval.limits.max_candidates_per_request > 10000) deny("INVALID_VOLUME_LIMIT");
  exact(approval.lifecycle, ["status", "valid_from_ms", "valid_until_ms", "supersedes_approval_id"]);
  if (approval.lifecycle.status !== "ACTIVE") deny("APPROVAL_NOT_ACTIVE");
  if (!safePositive(approval.lifecycle.valid_from_ms) || !safePositive(approval.lifecycle.valid_until_ms) || approval.lifecycle.valid_from_ms >= approval.lifecycle.valid_until_ms) deny("INVALID_APPROVAL_INTERVAL");
  if (approval.lifecycle.supersedes_approval_id !== null && !token(approval.lifecycle.supersedes_approval_id)) deny("INVALID_SUPERSESSION");
  exact(approval.revocation_binding, ["registry_ref", "minimum_registry_revision", "failure_policy"]);
  if (!token(approval.revocation_binding.registry_ref) || !safePositive(approval.revocation_binding.minimum_registry_revision) || approval.revocation_binding.failure_policy !== "DENY_ON_UNAVAILABLE_UNKNOWN_STALE_OR_ROLLBACK") deny("REVOCATION_POLICY_MISMATCH");
  exact(approval.issuer, ["issuer_ref", "key_ref", "signature_algorithm", "signature"]);
  if (approval.assurance !== "SYNTHETIC_ONLY" || approval.issuer.signature_algorithm !== "NONE_SYNTHETIC" || approval.issuer.issuer_ref !== "idr.synthetic.conformance-issuer.v1" || !token(approval.issuer.key_ref) || approval.issuer.signature !== "not-a-production-signature") deny("ASSURANCE_SIGNATURE_MISMATCH");
  if (!DIGEST.test(approval.approval_digest) || approval.approval_digest !== approvalDigest(approval)) deny("APPROVAL_DIGEST_MISMATCH");
}

function validateTime(evidence, approval) {
  assertCanonicalValue(evidence);
  exact(evidence, TIME_KEYS);
  if (evidence.schema_id !== "idr.aegis.trusted-current-time-evidence.v1" || evidence.schema_version !== 1) deny("TIME_SHAPE_MISMATCH");
  if (evidence.purpose !== "aegis_representative_export_verification") deny("TIME_PURPOSE_MISMATCH");
  if (evidence.clock_source_ref !== "idr.synthetic.conformance-clock.v1" || evidence.clock_profile_version !== 1 || !Number.isSafeInteger(evidence.clock_epoch) || !Number.isSafeInteger(evidence.sequence) || !NONCE.test(evidence.nonce)) deny("CLOCK_SOURCE_UNTRUSTED");
  if (evidence.assurance !== "SYNTHETIC_ONLY" || evidence.signature_algorithm !== "NONE_SYNTHETIC" || evidence.signature !== "not-a-production-signature") deny("TIME_ASSURANCE_SIGNATURE_MISMATCH");
  if (evidence.clock_epoch < SYNTHETIC_CLOCK_STATE.epoch) deny("CLOCK_EPOCH_ROLLBACK");
  if (evidence.clock_epoch === SYNTHETIC_CLOCK_STATE.epoch && evidence.sequence < SYNTHETIC_CLOCK_STATE.minimum_sequence) deny("CLOCK_SEQUENCE_ROLLBACK");
  if (SYNTHETIC_CLOCK_STATE.used_nonces.has(evidence.nonce)) deny("CLOCK_NONCE_REPLAY");
  if (!safePositive(evidence.attested_time_ms) || !Number.isSafeInteger(evidence.uncertainty_ms) || evidence.uncertainty_ms < 0 || evidence.uncertainty_ms > 60000) deny("TIME_UNCERTAINTY_EXCESSIVE");
  if (!DIGEST.test(evidence.time_evidence_digest) || evidence.time_evidence_digest !== timeDigest(evidence)) deny("TIME_EVIDENCE_DIGEST_MISMATCH");
  const earliest = evidence.attested_time_ms - evidence.uncertainty_ms;
  const latest = evidence.attested_time_ms + evidence.uncertainty_ms;
  if (earliest < approval.lifecycle.valid_from_ms || latest >= approval.lifecycle.valid_until_ms) deny("TIME_OUTSIDE_APPROVAL");
}

async function validateRequest(request, approval, evidence) {
  assertCanonicalValue(request);
  exact(request, REQUEST_KEYS);
  if (request.schema_id !== "idr.aegis.representative-export-verification-request.v1" || request.schema_version !== 1 || !token(request.request_id) || !NONCE.test(request.request_nonce) || request.purpose !== approval.purpose) deny("REQUEST_SHAPE_MISMATCH");
  if (request.source_class !== "synthetic_control") deny("REAL_DATA_NOT_AUTHORIZED");
  validateSourcePin(request.source_pin);
  if (!same(request.source_pin, approval.source_pin)) deny("SOURCE_PIN_MISMATCH");
  if (request.exporter_ref !== approval.exporter_ref) deny("EXPORTER_SCOPE_MISMATCH");
  if (!approval.allowed_tenant_refs.includes(request.tenant_ref)) deny("TENANT_OUT_OF_SCOPE");
  if (!approval.allowed_actor_refs.includes(request.actor_ref)) deny("ACTOR_OUT_OF_SCOPE");
  await validateBindings(request.contract_bindings);
  if (!same(request.contract_bindings, approval.contract_bindings)) deny("CONTRACT_BINDING_MISMATCH");
  if (!safePositive(request.requested_candidate_count) || request.requested_candidate_count > approval.limits.max_candidates_per_request) deny("VOLUME_LIMIT_EXCEEDED");
  if (request.approval_id !== approval.approval_id || request.approval_digest !== approval.approval_digest) deny("APPROVAL_BINDING_MISMATCH");
  if (request.time_evidence_digest !== evidence.time_evidence_digest) deny("TIME_BINDING_MISMATCH");
  exact(request.revocation_state, ["registry_ref", "registry_revision", "approval_status", "checked_at_time_evidence_digest"]);
  if (request.revocation_state.registry_ref !== approval.revocation_binding.registry_ref) deny("REVOCATION_REGISTRY_MISMATCH");
  if (request.revocation_state.registry_revision < approval.revocation_binding.minimum_registry_revision) deny("REVOCATION_REVISION_ROLLBACK");
  if (request.revocation_state.approval_status !== "NOT_REVOKED") deny("APPROVAL_REVOKED_OR_UNKNOWN");
  if (request.revocation_state.checked_at_time_evidence_digest !== evidence.time_evidence_digest) deny("REVOCATION_TIME_BINDING_MISMATCH");
  exact(request.lawful_basis, ["status", "proof_digest"]);
  if (request.lawful_basis.status !== "NOT_REQUIRED_SYNTHETIC" || request.lawful_basis.proof_digest !== null) deny("LAWFUL_BASIS_MISMATCH");
  validateForbiddenFlags(request.requested_uses, USE_KEYS);
  if (!DIGEST.test(request.request_digest) || request.request_digest !== requestDigest(request)) deny("REQUEST_DIGEST_MISMATCH");
  return "ALLOW_SYNTHETIC_CONTROL_ONLY";
}

function flip(value) { return `${value.slice(0, -1)}${value.endsWith("0") ? "1" : "0"}`; }

function mutate(target, mutation) {
  switch (mutation) {
    case "APPROVAL_UNKNOWN_FIELD": target.unexpected = true; break;
    case "APPROVAL_DIGEST_SUBSTITUTION": target.approval_digest = flip(target.approval_digest); return;
    case "APPROVAL_NOT_ACTIVE": target.lifecycle.status = "SUSPENDED"; break;
    case "APPROVAL_WILDCARD_TENANT": target.allowed_tenant_refs = ["*"]; break;
    case "APPROVAL_PRODUCT_SOURCE": target.allowed_source_classes.push("product_structured_observation"); break;
    case "APPROVAL_RAW_CONTENT": target.data_policy.allows_raw_content = true; break;
    case "APPROVAL_MODEL_TRAINING": target.data_policy.allows_model_training = true; break;
    case "APPROVAL_BAD_SOURCE_PIN": target.source_pin.tree_sha = "0".repeat(40); break;
    case "APPROVAL_CONTRACT_DRIFT": target.contract_bindings.producer_registry_digest = flip(target.contract_bindings.producer_registry_digest); break;
    case "APPROVAL_BAD_SIGNATURE_MODE": target.issuer.signature_algorithm = "ED25519"; break;
    case "APPROVAL_INVALID_INTERVAL": target.lifecycle.valid_until_ms = target.lifecycle.valid_from_ms; break;
    case "REVOCATION_FAIL_OPEN": target.revocation_binding.failure_policy = "ALLOW"; break;
    case "TIME_UNKNOWN_FIELD": target.unexpected = true; break;
    case "TIME_DIGEST_SUBSTITUTION": target.time_evidence_digest = flip(target.time_evidence_digest); return;
    case "TIME_WRONG_PURPOSE": target.purpose = "bundle_manifest_time"; break;
    case "TIME_UNTRUSTED_SOURCE": target.clock_source_ref = "local.process.clock"; break;
    case "TIME_BAD_SIGNATURE_MODE": target.signature_algorithm = "ED25519"; break;
    case "TIME_EPOCH_ROLLBACK": target.clock_epoch = 0; break;
    case "TIME_SEQUENCE_ROLLBACK": target.sequence = 41; break;
    case "TIME_NONCE_REPLAY": target.nonce = "clock-nonce-synthetic-used"; break;
    case "TIME_UNCERTAINTY_EXCESSIVE": target.uncertainty_ms = 60001; break;
    case "TIME_BEFORE_VALIDITY": target.attested_time_ms = 1999000; break;
    case "TIME_AFTER_VALIDITY": target.attested_time_ms = 3000000; break;
    case "REQUEST_UNKNOWN_FIELD": target.unexpected = true; break;
    case "REQUEST_DIGEST_SUBSTITUTION": target.request_digest = flip(target.request_digest); return;
    case "REQUEST_APPROVAL_SUBSTITUTION": target.approval_digest = flip(target.approval_digest); break;
    case "REQUEST_TIME_SUBSTITUTION": target.time_evidence_digest = flip(target.time_evidence_digest); break;
    case "REQUEST_TENANT_OUT_OF_SCOPE": target.tenant_ref = "tenant.synthetic.other"; break;
    case "REQUEST_ACTOR_OUT_OF_SCOPE": target.actor_ref = "actor.synthetic.other"; break;
    case "REQUEST_EXPORTER_MISMATCH": target.exporter_ref = "aegis.synthetic.other-exporter.v1"; break;
    case "REQUEST_TOO_MANY_CANDIDATES": target.requested_candidate_count = 11; break;
    case "REQUEST_REVOKED": target.revocation_state.approval_status = "REVOKED"; break;
    case "REQUEST_REVOCATION_REGISTRY_MISMATCH": target.revocation_state.registry_ref = "aegis.synthetic.other-registry.v1"; break;
    case "REQUEST_REVOCATION_ROLLBACK": target.revocation_state.registry_revision = 6; break;
    case "REQUEST_REVOCATION_TIME_DRIFT": target.revocation_state.checked_at_time_evidence_digest = flip(target.revocation_state.checked_at_time_evidence_digest); break;
    case "REQUEST_SECONDARY_USE": target.requested_uses.secondary_use = true; break;
    case "REQUEST_PRODUCT_DATA": target.source_class = "product_structured_observation"; target.lawful_basis.status = "UNRESOLVED"; break;
    case "REQUEST_CONTRACT_DRIFT": target.contract_bindings.host_mapping_profile_digest = flip(target.contract_bindings.host_mapping_profile_digest); break;
    default: deny("UNKNOWN_MUTATION");
  }
  if (mutation.startsWith("APPROVAL_") || mutation === "REVOCATION_FAIL_OPEN") sealApproval(target);
  else if (mutation.startsWith("TIME_")) sealTime(target);
  else sealRequest(target);
}

const manifest = await load("manifest.json");
const approval = await load(manifest.golden.approval_path);
const evidence = await load(manifest.golden.time_evidence_path);
const request = await load(manifest.golden.request_path);

for (const schema of manifest.schemas) {
  exact(schema, ["path", "sha256"]);
  const bytes = await readFile(join(root, schema.path));
  if (bytesDigest(bytes) !== `sha256:${schema.sha256}`) deny("SCHEMA_DIGEST_MISMATCH");
  JSON.parse(bytes.toString("utf8"));
}

if (process.argv.includes("--print-digests")) {
  const sealedApproval = sealApproval(clone(approval));
  const sealedTime = sealTime(clone(evidence));
  const sealedRequest = clone(request);
  sealedRequest.approval_digest = sealedApproval.approval_digest;
  sealedRequest.time_evidence_digest = sealedTime.time_evidence_digest;
  sealedRequest.revocation_state.checked_at_time_evidence_digest = sealedTime.time_evidence_digest;
  sealRequest(sealedRequest);
  console.log(JSON.stringify({ approval_digest: sealedApproval.approval_digest, time_evidence_digest: sealedTime.time_evidence_digest, request_digest: sealedRequest.request_digest }, null, 2));
  process.exit(0);
}

await validateApproval(approval);
validateTime(evidence, approval);
const result = await validateRequest(request, approval, evidence);
if (approval.approval_digest !== manifest.golden.expected_approval_digest || evidence.time_evidence_digest !== manifest.golden.expected_time_evidence_digest || request.request_digest !== manifest.golden.expected_request_digest || result !== manifest.golden.expected_result) deny("GOLDEN_MANIFEST_MISMATCH");

let passed = 1;
for (const test of manifest.negative_cases) {
  const a = clone(approval);
  const t = clone(evidence);
  const r = clone(request);
  const target = test.target === "approval" ? a : test.target === "time" ? t : r;
  mutate(target, test.mutation);
  try {
    await validateApproval(a);
    validateTime(t, a);
    await validateRequest(r, a, t);
    deny("NEGATIVE_CASE_ACCEPTED");
  } catch (error) {
    if (!(error instanceof GateError) || error.code !== test.expected_code) {
      console.error(`${test.id}: expected ${test.expected_code}, got ${error.code ?? error.message}`);
      process.exit(1);
    }
  }
  passed += 1;
}

console.log(`source export approval trust conformance: ${passed}/${manifest.negative_cases.length + 1} cases passed`);
console.log(`result: ${result}`);
console.log("real-data export: BLOCKED");
