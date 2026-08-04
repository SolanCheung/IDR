import json
from pathlib import Path
import unittest

from idr_eval.generated_production_v1 import (
    ProductionProofObservationV1,
    canonical_digest_v1,
    canonical_json_v1,
    verify_ed25519_v1,
)


FIXTURE = json.loads(
    (
        Path(__file__).parents[3]
        / "contracts"
        / "production"
        / "v1"
        / "canonical-golden-v1.json"
    ).read_text(encoding="utf-8")
)


class ProductionConformanceTests(unittest.TestCase):
    def test_python_matches_rust_canonical_digest_and_signature_vectors(self):
        self.assertEqual(
            canonical_json_v1(FIXTURE["canonical_value"]),
            FIXTURE["expected_canonical_utf8"],
        )
        self.assertEqual(
            canonical_digest_v1(
                FIXTURE["digest_domain"], FIXTURE["canonical_value"]
            ),
            FIXTURE["expected_digest"],
        )
        signing_bytes = canonical_json_v1(
            ["idr-production-proof-v1", FIXTURE["proof_claims"]]
        ).encode("utf-8")
        self.assertEqual(
            signing_bytes.decode("utf-8"), FIXTURE["expected_signing_utf8"]
        )
        self.assertTrue(
            verify_ed25519_v1(
                FIXTURE["public_key_hex"],
                signing_bytes,
                FIXTURE["signature_hex"],
            )
        )

    def test_python_observation_parser_rejects_unknown_fields(self):
        envelope = {
            "claims": FIXTURE["proof_claims"],
            "key_id": "key:golden:v1",
            "algorithm": "ed25519",
            "signature": FIXTURE["signature_hex"],
        }
        ProductionProofObservationV1.from_mapping(envelope)
        with self.assertRaises(ValueError):
            ProductionProofObservationV1.from_mapping(
                {**envelope, "bypass": True}
            )


if __name__ == "__main__":
    unittest.main()
