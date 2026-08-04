//! Generates the checked-in cross-language IDR production contract artifacts.
//!
//! Run from the workspace root with:
//! `cargo run -p idr-protocol --example generate_production_contracts`.

use ed25519_dalek::{Signer, SigningKey};
use idr_protocol::production::{
    canonical_digest_v1, canonical_json_bytes_v1, ProductionProofClaimsV1,
};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

const PROOF_KINDS: &[&str] = &[
    "caller_authentication",
    "input_admission",
    "context_snapshot",
    "intent_fast_path_admission",
    "decision_necessity_admission",
    "turn_coordination_admission",
    "response_policy",
    "response_admission",
    "authority",
    "capability",
    "policy",
    "exact_authorization",
    "action_admission",
    "execution_permit",
    "provider_receipt",
    "outcome_observation",
    "human_model_promotion",
    "human_model_user_confirmation",
    "human_model_correction",
];

const CANDIDATE_KINDS: &[&str] = &[
    "canonical_input",
    "context_snapshot",
    "intent",
    "decision",
    "turn_coordination",
    "response",
    "action",
    "action_admission_decision",
    "execution_receipt",
    "outcome",
    "human_model_candidate",
    "human_model_promotion_decision",
    "human_model_assertion",
];

fn main() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf();
    let contract_dir = workspace.join("contracts/production/v1");
    fs::create_dir_all(&contract_dir).expect("create contract directory");

    let schema = production_schema();
    write_json(&contract_dir.join("idr-production-v1.schema.json"), &schema);
    write_json(
        &contract_dir.join("canonical-golden-v1.json"),
        &golden_vector(),
    );

    let ts_dir = workspace.join("packages/interaction-client/src/generated");
    fs::create_dir_all(&ts_dir).expect("create TypeScript generated directory");
    fs::write(ts_dir.join("idr-production-v1.ts"), typescript_contract())
        .expect("write TypeScript generated contract");

    let python_dir = workspace.join("research/evaluation/src/idr_eval");
    fs::write(
        python_dir.join("generated_production_v1.py"),
        python_contract(),
    )
    .expect("write Python generated contract");
}

fn production_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://idr.local/schema/production/v1",
        "title": "The Human-Centered Intent & Decision Runtime production wire V1",
        "type": "object",
        "oneOf": [
            {"$ref": "#/$defs/CandidateSubmissionV1"},
            {"$ref": "#/$defs/ProductionProofEnvelopeV1"}
        ],
        "$defs": {
            "WireCompatibilityV1": {
                "type": "object",
                "additionalProperties": false,
                "required": ["schema_version", "minimum_reader_version", "critical_feature_flags"],
                "properties": {
                    "schema_version": {"const": 1},
                    "minimum_reader_version": {"const": 1},
                    "critical_feature_flags": {
                        "type": "array",
                        "maxItems": 0,
                        "items": {"type": "string"}
                    }
                }
            },
            "CandidateSubmissionV1": {
                "type": "object",
                "additionalProperties": false,
                "required": [
                    "compatibility", "candidate_id", "candidate_kind", "run_id", "turn_id",
                    "subject_ref", "tenant_ref", "scope_ref", "purpose_ref",
                    "policy_revision_ref", "valid_from", "valid_until", "payload"
                ],
                "properties": {
                    "compatibility": {"$ref": "#/$defs/WireCompatibilityV1"},
                    "candidate_id": {"type": "string", "format": "uuid"},
                    "candidate_kind": {"enum": CANDIDATE_KINDS},
                    "run_id": {"type": "string", "format": "uuid"},
                    "turn_id": {"type": "string", "format": "uuid"},
                    "subject_ref": {"type": "string", "minLength": 1, "maxLength": 512},
                    "tenant_ref": {"type": "string", "minLength": 1, "maxLength": 512},
                    "scope_ref": {"type": "string", "minLength": 1, "maxLength": 512},
                    "purpose_ref": {"type": "string", "minLength": 1, "maxLength": 512},
                    "policy_revision_ref": {"type": "string", "minLength": 1, "maxLength": 512},
                    "valid_from": {"type": "integer", "minimum": 1, "maximum": 253_402_300_799_i64},
                    "valid_until": {"type": "integer", "minimum": 1, "maximum": 253_402_300_799_i64},
                    "payload": true
                }
            },
            "ProductionProofClaimsV1": {
                "type": "object",
                "additionalProperties": false,
                "required": [
                    "compatibility", "proof_id", "trust_domain", "environment_ref",
                    "proof_kind", "issuer_ref", "subject_ref",
                    "subject_digest", "audience_ref", "tenant_ref", "scope_ref", "purpose_ref",
                    "policy_revision_ref", "issued_at", "not_before", "expires_at", "nonce",
                    "principal_digest", "assertion"
                ],
                "properties": {
                    "compatibility": {"$ref": "#/$defs/WireCompatibilityV1"},
                    "proof_id": {"type": "string", "format": "uuid"},
                    "trust_domain": {"enum": ["shadow", "production"]},
                    "environment_ref": {"type": "string", "minLength": 1, "maxLength": 512},
                    "proof_kind": {"enum": PROOF_KINDS},
                    "issuer_ref": {"type": "string", "minLength": 1, "maxLength": 512},
                    "subject_ref": {"type": "string", "minLength": 1, "maxLength": 512},
                    "subject_digest": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                    "audience_ref": {"type": "string", "minLength": 1, "maxLength": 512},
                    "tenant_ref": {"type": "string", "minLength": 1, "maxLength": 512},
                    "scope_ref": {"type": "string", "minLength": 1, "maxLength": 512},
                    "purpose_ref": {"type": "string", "minLength": 1, "maxLength": 512},
                    "policy_revision_ref": {"type": "string", "minLength": 1, "maxLength": 512},
                    "issued_at": {"type": "integer", "minimum": 1, "maximum": 253_402_300_799_i64},
                    "not_before": {"type": "integer", "minimum": 1, "maximum": 253_402_300_799_i64},
                    "expires_at": {"type": "integer", "minimum": 1, "maximum": 253_402_300_799_i64},
                    "nonce": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                    "principal_digest": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                    "assertion": true
                }
            },
            "ProductionProofEnvelopeV1": {
                "type": "object",
                "additionalProperties": false,
                "required": ["claims", "key_id", "algorithm", "signature"],
                "properties": {
                    "claims": {"$ref": "#/$defs/ProductionProofClaimsV1"},
                    "key_id": {"type": "string", "minLength": 1, "maxLength": 512},
                    "algorithm": {"const": "ed25519"},
                    "signature": {"type": "string", "pattern": "^[0-9a-f]{128}$"}
                }
            }
        }
    })
}

fn golden_vector() -> Value {
    let canonical_value = json!({
        "\u{e000}": "private-use",
        "😀": "astral-first-under-utf16",
        "escaped": "\n\t\"\\",
        "integer": 9_007_199_254_740_991_u64,
        "unicode": "意图与决策"
    });
    let expected_bytes =
        String::from_utf8(canonical_json_bytes_v1(&canonical_value).expect("canonical bytes"))
            .expect("UTF-8");
    let domain = "idr-cross-language-golden-v1";
    let expected_digest = canonical_digest_v1(domain, &canonical_value).expect("digest");
    let claims_value = json!({
        "compatibility": {
            "schema_version": 1,
            "minimum_reader_version": 1,
            "critical_feature_flags": []
        },
        "proof_id": "018f22e2-4fd1-7a11-8a44-7c58f9bc0011",
        "trust_domain": "shadow",
        "environment_ref": "environment:idr:shadow:golden",
        "proof_kind": "input_admission",
        "issuer_ref": "issuer:golden",
        "subject_ref": "candidate:018f22e2-4fd1-7a11-8a44-7c58f9bc0022",
        "subject_digest": "11".repeat(32),
        "audience_ref": "audience:idr:shadow:golden",
        "tenant_ref": "tenant:golden",
        "scope_ref": "scope:interaction",
        "purpose_ref": "purpose:conformance",
        "policy_revision_ref": "policy:idr:v1",
        "issued_at": 1_800_000_000,
        "not_before": 1_800_000_000,
        "expires_at": 1_800_000_300,
        "nonce": "22".repeat(32),
        "principal_digest": "33".repeat(32),
        "assertion": {"outcome": "admit"}
    });
    let claims: ProductionProofClaimsV1 =
        serde_json::from_value(claims_value.clone()).expect("fixed claims");
    let signing_bytes = claims.canonical_signing_bytes().expect("signing bytes");
    let signing_key = SigningKey::from_bytes(&[17_u8; 32]);
    let signature = signing_key.sign(&signing_bytes);
    json!({
        "schema_version": 1,
        "canonical_value": canonical_value,
        "expected_canonical_utf8": expected_bytes,
        "digest_domain": domain,
        "expected_digest": expected_digest,
        "proof_claims": claims_value,
        "expected_signing_utf8": String::from_utf8(signing_bytes).expect("UTF-8"),
        "public_key_hex": encode_hex(signing_key.verifying_key().as_bytes()),
        "signature_hex": encode_hex(&signature.to_bytes())
    })
}

fn typescript_contract() -> String {
    format!(
        r#"// @generated by crates/idr-protocol/examples/generate_production_contracts.rs
// Do not edit by hand.
import {{ createHash, createPublicKey, verify }} from "node:crypto";

export const productionProofKinds = {proof_kinds} as const;
export const candidateKinds = {candidate_kinds} as const;
export const maxAuthorityEpochSecondsV1 = 253_402_300_799;
export type ProductionProofKindV1 = typeof productionProofKinds[number];
export type CandidateKindV1 = typeof candidateKinds[number];
export type WireCompatibilityV1 = {{
  schema_version: 1;
  minimum_reader_version: 1;
  critical_feature_flags: [];
}};
export type CandidateSubmissionV1 = {{
  compatibility: WireCompatibilityV1;
  candidate_id: string;
  candidate_kind: CandidateKindV1;
  run_id: string;
  turn_id: string;
  subject_ref: string;
  tenant_ref: string;
  scope_ref: string;
  purpose_ref: string;
  policy_revision_ref: string;
  valid_from: number;
  valid_until: number;
  payload: unknown;
}};
export type ProductionProofClaimsV1 = {{
  compatibility: WireCompatibilityV1;
  proof_id: string;
  trust_domain: "shadow" | "production";
  environment_ref: string;
  proof_kind: ProductionProofKindV1;
  issuer_ref: string;
  subject_ref: string;
  subject_digest: string;
  audience_ref: string;
  tenant_ref: string;
  scope_ref: string;
  purpose_ref: string;
  policy_revision_ref: string;
  issued_at: number;
  not_before: number;
  expires_at: number;
  nonce: string;
  principal_digest: string;
  assertion: unknown;
}};
export type ProductionProofEnvelopeV1 = {{
  claims: ProductionProofClaimsV1;
  key_id: string;
  algorithm: "ed25519";
  signature: string;
}};

export function canonicalJsonV1(value: unknown): string {{
  let nodes = 0;
  const encode = (item: unknown, depth: number): string => {{
    nodes += 1;
    if (depth > 32 || nodes > 10_000) throw new TypeError("canonical JSON resource limit");
    if (item === null) return "null";
    if (typeof item === "boolean" || typeof item === "string") return JSON.stringify(item);
    if (typeof item === "number") {{
      if (!Number.isSafeInteger(item)) throw new TypeError("only safe integers are allowed");
      return String(item);
    }}
    if (Array.isArray(item)) return `[${{item.map((value) => encode(value, depth + 1)).join(",")}}]`;
    if (typeof item === "object") {{
      const object = item as Record<string, unknown>;
      return `{{${{Object.keys(object).sort().map((key) =>
        `${{JSON.stringify(key)}}:${{encode(object[key], depth + 1)}}`).join(",")}}}}`;
    }}
    throw new TypeError("unsupported canonical JSON value");
  }};
  return encode(value, 1);
}}

export function canonicalDigestV1(domain: string, value: unknown): string {{
  return createHash("sha256").update(canonicalJsonV1([domain, value]), "utf8").digest("hex");
}}

export function parseProductionProofEnvelope(value: unknown): ProductionProofEnvelopeV1 {{
  const envelope = strictRecord(value, ["claims", "key_id", "algorithm", "signature"], "proof envelope");
  const claims = strictRecord(envelope.claims, [
    "compatibility", "proof_id", "trust_domain", "environment_ref",
    "proof_kind", "issuer_ref", "subject_ref",
    "subject_digest", "audience_ref", "tenant_ref", "scope_ref", "purpose_ref",
    "policy_revision_ref", "issued_at", "not_before", "expires_at", "nonce",
    "principal_digest", "assertion",
  ], "proof claims");
  parseCompatibility(claims.compatibility);
  if ((claims.trust_domain !== "shadow" && claims.trust_domain !== "production") ||
      !reference(claims.environment_ref) ||
      !productionProofKinds.includes(claims.proof_kind as ProductionProofKindV1) ||
      !uuid(claims.proof_id) || !digest(claims.subject_digest) || !digest(claims.nonce) ||
      !digest(claims.principal_digest) ||
      !reference(claims.issuer_ref) || !reference(claims.subject_ref) ||
      !reference(claims.audience_ref) || !reference(claims.tenant_ref) ||
      !reference(claims.scope_ref) || !reference(claims.purpose_ref) ||
      !reference(claims.policy_revision_ref) ||
      !safePositive(claims.issued_at) || !safePositive(claims.not_before) ||
      !safePositive(claims.expires_at) ||
      (claims.expires_at as number) > maxAuthorityEpochSecondsV1 ||
      (claims.not_before as number) < (claims.issued_at as number) ||
      (claims.expires_at as number) <= (claims.not_before as number)) {{
    throw new TypeError("invalid proof claims");
  }}
  if (envelope.algorithm !== "ed25519" || !reference(envelope.key_id) ||
      typeof envelope.signature !== "string" || !/^[0-9a-f]{{128}}$/.test(envelope.signature)) {{
    throw new TypeError("invalid proof envelope");
  }}
  return envelope as ProductionProofEnvelopeV1;
}}

export function verifyEd25519GoldenV1(
  claims: ProductionProofClaimsV1,
  publicKeyHex: string,
  signatureHex: string,
): boolean {{
  const spkiPrefix = Buffer.from("302a300506032b6570032100", "hex");
  const key = createPublicKey({{key: Buffer.concat([spkiPrefix, Buffer.from(publicKeyHex, "hex")]),
    format: "der", type: "spki"}});
  return verify(null, Buffer.from(canonicalJsonV1(["idr-production-proof-v1", claims]), "utf8"),
    key, Buffer.from(signatureHex, "hex"));
}}

function parseCompatibility(value: unknown): void {{
  const compatibility = strictRecord(value,
    ["schema_version", "minimum_reader_version", "critical_feature_flags"], "compatibility");
  if (compatibility.schema_version !== 1 || compatibility.minimum_reader_version !== 1 ||
      !Array.isArray(compatibility.critical_feature_flags) ||
      compatibility.critical_feature_flags.length !== 0) {{
    throw new TypeError("unsupported wire compatibility");
  }}
}}
function strictRecord(value: unknown, fields: string[], name: string): Record<string, unknown> {{
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw new TypeError(`${{name}} must be an object`);
  const record = value as Record<string, unknown>;
  if (Object.keys(record).some((key) => !fields.includes(key)) ||
      fields.some((field) => !(field in record))) throw new TypeError(`${{name}} has missing/unknown fields`);
  return record;
}}
function reference(value: unknown): value is string {{
  return typeof value === "string" && value.length > 0 && value.length <= 512 &&
    value.trim() === value && !/[\u0000-\u001f\u007f]/u.test(value);
}}
function digest(value: unknown): value is string {{
  return typeof value === "string" && /^[0-9a-f]{{64}}$/.test(value);
}}
function uuid(value: unknown): value is string {{
  return typeof value === "string" &&
    /^[0-9a-f]{{8}}-[0-9a-f]{{4}}-[1-8][0-9a-f]{{3}}-[89ab][0-9a-f]{{3}}-[0-9a-f]{{12}}$/.test(value);
}}
function safePositive(value: unknown): value is number {{
  return typeof value === "number" && Number.isSafeInteger(value) && value > 0;
}}
"#,
        proof_kinds = serde_json::to_string(PROOF_KINDS).expect("proof kinds"),
        candidate_kinds = serde_json::to_string(CANDIDATE_KINDS).expect("candidate kinds"),
    )
}

fn python_contract() -> &'static str {
    r#"# @generated by crates/idr-protocol/examples/generate_production_contracts.rs
# Do not edit by hand. This module is evaluation-only and never grants authority.
from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass
from typing import Any, Mapping

MAX_SAFE_INTEGER = 9_007_199_254_740_991
MAX_AUTHORITY_EPOCH_SECONDS = 253_402_300_799


def canonical_json_v1(value: Any) -> str:
    nodes = 0

    def encode(item: Any, depth: int) -> str:
        nonlocal nodes
        nodes += 1
        if depth > 32 or nodes > 10_000:
            raise ValueError("canonical JSON resource limit")
        if item is None:
            return "null"
        if item is True:
            return "true"
        if item is False:
            return "false"
        if isinstance(item, int) and not isinstance(item, bool):
            if abs(item) > MAX_SAFE_INTEGER:
                raise ValueError("integer outside safe wire range")
            return str(item)
        if isinstance(item, float):
            raise ValueError("floating-point values are forbidden")
        if isinstance(item, str):
            return json.dumps(item, ensure_ascii=False, separators=(",", ":"))
        if isinstance(item, list):
            return "[" + ",".join(encode(child, depth + 1) for child in item) + "]"
        if isinstance(item, Mapping):
            if not all(isinstance(key, str) for key in item):
                raise ValueError("object keys must be strings")
            keys = sorted(item, key=lambda key: key.encode("utf-16-be", "surrogatepass"))
            return "{" + ",".join(
                json.dumps(key, ensure_ascii=False) + ":" + encode(item[key], depth + 1)
                for key in keys
            ) + "}"
        raise ValueError("unsupported canonical JSON value")

    return encode(value, 1)


def canonical_digest_v1(domain: str, value: Any) -> str:
    return hashlib.sha256(
        canonical_json_v1([domain, value]).encode("utf-8")
    ).hexdigest()


@dataclass(frozen=True)
class ProductionProofObservationV1:
    claims: Mapping[str, Any]
    key_id: str
    algorithm: str
    signature: str

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "ProductionProofObservationV1":
        expected = {"claims", "key_id", "algorithm", "signature"}
        if set(value) != expected or value.get("algorithm") != "ed25519":
            raise ValueError("invalid or unknown proof envelope fields")
        claims = value.get("claims")
        if not isinstance(claims, Mapping):
            raise ValueError("claims must be an object")
        expected_claims = {
            "compatibility", "proof_id", "trust_domain", "environment_ref",
            "proof_kind", "issuer_ref", "subject_ref",
            "subject_digest", "audience_ref", "tenant_ref", "scope_ref", "purpose_ref",
            "policy_revision_ref", "issued_at", "not_before", "expires_at", "nonce",
            "principal_digest", "assertion",
        }
        if set(claims) != expected_claims:
            raise ValueError("invalid or unknown proof claim fields")
        if claims.get("trust_domain") not in {"shadow", "production"}:
            raise ValueError("invalid trust domain")
        if not isinstance(claims.get("environment_ref"), str) or not claims["environment_ref"]:
            raise ValueError("invalid environment")
        compatibility = claims.get("compatibility")
        if compatibility != {
            "schema_version": 1,
            "minimum_reader_version": 1,
            "critical_feature_flags": [],
        }:
            raise ValueError("unsupported compatibility")
        for field in ("subject_digest", "nonce"):
            item = claims.get(field)
            if not isinstance(item, str) or len(item) != 64 or any(
                character not in "0123456789abcdef" for character in item
            ):
                raise ValueError("invalid digest")
        for field in ("issued_at", "not_before", "expires_at"):
            item = claims.get(field)
            if not isinstance(item, int) or isinstance(item, bool) or not (0 < item <= MAX_SAFE_INTEGER):
                raise ValueError("invalid wire time")
        if claims["expires_at"] > MAX_AUTHORITY_EPOCH_SECONDS:
            raise ValueError("authority expiry exceeds portable limit")
        signature = value.get("signature")
        if not isinstance(signature, str) or len(signature) != 128 or any(
            character not in "0123456789abcdef" for character in signature
        ):
            raise ValueError("invalid signature")
        return cls(claims=dict(claims), key_id=str(value["key_id"]),
                   algorithm="ed25519", signature=signature)


# Minimal RFC 8032 verifier used only for cross-language conformance tests.
_Q = 2**255 - 19
_L = 2**252 + 27742317777372353535851937790883648493
_D = (-121665 * pow(121666, _Q - 2, _Q)) % _Q
_I = pow(2, (_Q - 1) // 4, _Q)


def _xrecover(y: int) -> int:
    xx = (y * y - 1) * pow(_D * y * y + 1, _Q - 2, _Q)
    x = pow(xx, (_Q + 3) // 8, _Q)
    if (x * x - xx) % _Q != 0:
        x = (x * _I) % _Q
    if x & 1:
        x = _Q - x
    return x


_BY = (4 * pow(5, _Q - 2, _Q)) % _Q
_B = (_xrecover(_BY), _BY)


def _decodepoint(data: bytes) -> tuple[int, int]:
    if len(data) != 32:
        raise ValueError("invalid point")
    y = int.from_bytes(data, "little") & ((1 << 255) - 1)
    x = _xrecover(y)
    if bool(x & 1) != bool(data[31] & 0x80):
        x = _Q - x
    point = (x, y)
    if (-x * x + y * y - 1 - _D * x * x * y * y) % _Q != 0:
        raise ValueError("point is not on curve")
    return point


def _add(left: tuple[int, int], right: tuple[int, int]) -> tuple[int, int]:
    x1, y1 = left
    x2, y2 = right
    factor = _D * x1 * x2 * y1 * y2
    return (
        ((x1 * y2 + x2 * y1) * pow(1 + factor, _Q - 2, _Q)) % _Q,
        ((y1 * y2 + x1 * x2) * pow(1 - factor, _Q - 2, _Q)) % _Q,
    )


def _scalarmult(point: tuple[int, int], scalar: int) -> tuple[int, int]:
    result = (0, 1)
    addend = point
    while scalar:
        if scalar & 1:
            result = _add(result, addend)
        addend = _add(addend, addend)
        scalar >>= 1
    return result


def verify_ed25519_v1(public_key_hex: str, message: bytes, signature_hex: str) -> bool:
    try:
        public_key = bytes.fromhex(public_key_hex)
        signature = bytes.fromhex(signature_hex)
        if len(public_key) != 32 or len(signature) != 64:
            return False
        point_a = _decodepoint(public_key)
        point_r = _decodepoint(signature[:32])
        scalar_s = int.from_bytes(signature[32:], "little")
        if scalar_s >= _L:
            return False
        challenge = int.from_bytes(
            hashlib.sha512(signature[:32] + public_key + message).digest(), "little"
        ) % _L
        return _scalarmult(_B, scalar_s) == _add(point_r, _scalarmult(point_a, challenge))
    except (ValueError, OverflowError):
        return False
"#
}

fn write_json(path: &Path, value: &Value) {
    let mut encoded = serde_json::to_string_pretty(value).expect("encode JSON");
    encoded.push('\n');
    fs::write(path, encoded).expect("write generated JSON");
}

fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
