import copy
import json
import pathlib
import unittest

from idr_eval import evaluate_runtime_output


FIXTURE_PATH = (
    pathlib.Path(__file__).resolve().parents[3]
    / "contracts"
    / "human-centered"
    / "v1"
    / "fixtures"
    / "supplier-confirmation.json"
)


def load_fixture():
    with FIXTURE_PATH.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def golden_runtime_output(fixture):
    expected = fixture["expected"]
    return {
        "schema_version": 1,
        "fast_path": {
            "outcome": expected["fast_path_outcome"],
            "reason_codes": ["ambiguity_present"],
            "rule_version": 1,
        },
        "decision_necessity": {
            "outcome": expected["decision_necessity"],
            "enters_decision_runtime": True,
            "reason_codes": ["multiple_viable_options"],
            "rule_version": 1,
        },
        "coordination_mode": expected["coordination_mode"],
        "action_posture": expected["action_posture"],
        "next_run_state": expected["next_run_state"],
    }


class EvaluationTests(unittest.TestCase):
    def test_golden_supplier_output_passes(self):
        fixture = load_fixture()
        report = evaluate_runtime_output(
            fixture,
            golden_runtime_output(fixture),
        )
        self.assertTrue(report.passed)
        self.assertEqual(report.score_basis_points, 10_000)
        self.assertEqual(report.violations, [])

    def test_deleted_base_hypothesis_is_detected(self):
        fixture = load_fixture()
        projection = copy.deepcopy(fixture["intent_projection"])
        projection["personalized_hypothesis_refs"].remove(
            "intent:prepare-suspension"
        )
        report = evaluate_runtime_output(
            fixture,
            golden_runtime_output(fixture),
            projection,
        )
        self.assertFalse(report.passed)
        self.assertIn(
            "personalization deleted base hypotheses",
            report.violations[0],
        )

    def test_confirmation_bypass_is_detected(self):
        fixture = load_fixture()
        output = golden_runtime_output(fixture)
        output["action_posture"] = "planned"
        output["next_run_state"] = "running"
        report = evaluate_runtime_output(fixture, output)
        self.assertFalse(report.passed)
        self.assertTrue(
            any(
                "did not stop at authorization" in violation
                for violation in report.violations
            )
        )

    def test_invalid_input_digest_is_rejected(self):
        fixture = load_fixture()
        fixture["input"]["content_digest"] = "not-a-digest"

        with self.assertRaisesRegex(ValueError, "content_digest"):
            evaluate_runtime_output(
                fixture,
                golden_runtime_output(fixture),
            )

    def test_actor_role_impersonation_is_rejected(self):
        fixture = load_fixture()
        fixture["input"].update(
            {
                "source_actor": "tool",
                "actor_ref": "tool:search",
                "primary_semantic_role": "authorization",
                "semantic_roles": ["authorization"],
            }
        )

        with self.assertRaisesRegex(ValueError, "semantic roles"):
            evaluate_runtime_output(
                fixture,
                golden_runtime_output(fixture),
            )

    def test_unknown_input_fields_are_rejected(self):
        fixture = load_fixture()
        fixture["input"]["untrusted_extension"] = True

        with self.assertRaisesRegex(ValueError, "unknown fields"):
            evaluate_runtime_output(
                fixture,
                golden_runtime_output(fixture),
            )

    def test_logical_time_must_fit_the_cross_language_wire_range(self):
        fixture = load_fixture()
        fixture["input"]["logical_time"] = 9_007_199_254_740_992

        with self.assertRaisesRegex(ValueError, "safe integer"):
            evaluate_runtime_output(
                fixture,
                golden_runtime_output(fixture),
            )


if __name__ == "__main__":
    unittest.main()
