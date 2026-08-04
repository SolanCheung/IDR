#!/usr/bin/env node

import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(fileURLToPath(import.meta.url));
const bindingKeys = new Set([
  "schema_id",
  "schema_version",
  "snapshot_ref",
  "policy_ref",
  "purpose",
  "decision_context_digest",
  "accepted_warning_codes",
  "bound_at_ms",
  "binding_digest",
]);

async function load(path) {
  const value = JSON.parse(await readFile(join(root, path), "utf8"));
  assertCanonicalValue(value);
  return value;
}

function assertCanonicalValue(value) {
  if (value === null || typeof value === "boolean") return;
  if (typeof value === "string") {
    if (value !== value.normalize("NFC")) throw new Error("NON_NFC_STRING");
    return;
  }
  if (typeof value === "number") {
    if (!Number.isSafeInteger(value) || value < 0) throw new Error("UNSAFE_NUMBER");
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
  throw new Error("UNSUPPORTED_JSON_TYPE");
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
  return hash.digest("hex");
}

function assertExactKeys(value, expected) {
  const unknown = Object.keys(value).filter((key) => !expected.has(key));
  if (unknown.length > 0) throw new Error(`UNKNOWN_FIELD:${unknown.join(",")}`);
}

const manifest = await load("manifest.json");
await load("idr-evidence-governance-v1.schema.json");
const passed = [];

const golden = await load(manifest.valid_cases[0].path);
assertExactKeys(golden, bindingKeys);
const goldenDigest = digest(
  "idr:v1.4:decision-evidence-binding:v1",
  golden,
  "binding_digest",
);
if (goldenDigest !== manifest.valid_cases[0].expected_binding_digest) {
  throw new Error(`GOLDEN_DIGEST:${goldenDigest}`);
}
if (golden.binding_digest !== goldenDigest) throw new Error("GOLDEN_BINDING_MISMATCH");
passed.push(manifest.valid_cases[0].id);

for (const testCase of manifest.invalid_cases) {
  const value = await load(testCase.path);
  switch (testCase.id) {
    case "BINDING_SNAPSHOT_SUBSTITUTION":
    case "BINDING_WARNING_DROP": {
      const actual = digest(
        "idr:v1.4:decision-evidence-binding:v1",
        value,
        "binding_digest",
      );
      if (actual === value.binding_digest) throw new Error(`${testCase.id}:NOT_REJECTED`);
      break;
    }
    case "BINDING_UNKNOWN_FIELD": {
      let rejected = false;
      try {
        assertExactKeys(value, bindingKeys);
      } catch (error) {
        rejected = String(error).includes("UNKNOWN_FIELD");
      }
      if (!rejected) throw new Error(`${testCase.id}:NOT_REJECTED`);
      break;
    }
    case "SNAPSHOT_ADMITS_DENY":
      if (!value.entries.some((entry) => entry.verdict === "DENY")) {
        throw new Error(`${testCase.id}:INVALID_FIXTURE`);
      }
      break;
    case "PROHIBITED_PERSONALITY_FACT":
      if (!value.predicate.startsWith("personality.")) {
        throw new Error(`${testCase.id}:INVALID_FIXTURE`);
      }
      break;
    case "PRE_DISPATCH_STALE_PASS":
      if (!(value.result === "PASS" && value.checked_at_ms >= value.valid_until_ms)) {
        throw new Error(`${testCase.id}:INVALID_FIXTURE`);
      }
      break;
    default:
      throw new Error(`UNKNOWN_CASE:${testCase.id}`);
  }
  passed.push(testCase.id);
}

process.stdout.write(
  `${JSON.stringify({
    pack_id: manifest.pack_id,
    status: manifest.status,
    passed: passed.length,
    failed: 0,
    cases: passed,
  }, null, 2)}\n`,
);
