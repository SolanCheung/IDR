import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  canonicalDigestV1,
  canonicalJsonV1,
  parseProductionProofEnvelope,
  verifyEd25519GoldenV1,
  type ProductionProofClaimsV1,
} from "../src/generated/idr-production-v1.ts";

const fixture = JSON.parse(
  readFileSync(
    new URL(
      "../../../contracts/production/v1/canonical-golden-v1.json",
      import.meta.url,
    ),
    "utf8",
  ),
) as {
  canonical_value: unknown;
  expected_canonical_utf8: string;
  digest_domain: string;
  expected_digest: string;
  proof_claims: ProductionProofClaimsV1;
  expected_signing_utf8: string;
  public_key_hex: string;
  signature_hex: string;
};

test("generated TypeScript matches Rust canonical and digest golden vectors", () => {
  assert.equal(
    canonicalJsonV1(fixture.canonical_value),
    fixture.expected_canonical_utf8,
  );
  assert.equal(
    canonicalDigestV1(fixture.digest_domain, fixture.canonical_value),
    fixture.expected_digest,
  );
  assert.equal(
    canonicalJsonV1(["idr-production-proof-v1", fixture.proof_claims]),
    fixture.expected_signing_utf8,
  );
  assert.equal(
    verifyEd25519GoldenV1(
      fixture.proof_claims,
      fixture.public_key_hex,
      fixture.signature_hex,
    ),
    true,
  );
});

test("generated TypeScript proof parser rejects unknown fields", () => {
  const envelope = {
    claims: fixture.proof_claims,
    key_id: "key:golden:v1",
    algorithm: "ed25519",
    signature: fixture.signature_hex,
  };
  assert.deepEqual(parseProductionProofEnvelope(envelope), envelope);
  assert.throws(
    () => parseProductionProofEnvelope({ ...envelope, bypass: true }),
    /missing\/unknown fields/,
  );
});
