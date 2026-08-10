import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from evaluation.evaluate import calculate, read_jsonl


class EvaluationTests(unittest.TestCase):
    def test_calculates_required_rates(self):
        metrics = calculate(
            [
                {
                    "intent_corrected": True,
                    "decision_overridden": True,
                    "clarification_required": False,
                    "model_inference_used": True,
                    "outcome_status": "success",
                },
                {
                    "intent_corrected": False,
                    "decision_overridden": False,
                    "clarification_required": True,
                    "model_inference_used": False,
                    "outcome_status": "failure",
                },
            ]
        )
        self.assertEqual(metrics.records, 2)
        self.assertEqual(metrics.intent_correction_rate, 0.5)
        self.assertEqual(metrics.decision_override_rate, 0.5)
        self.assertEqual(metrics.reclarification_rate, 0.5)
        self.assertEqual(metrics.outcome_success_rate, 0.5)
        self.assertEqual(metrics.host_model_invocation_rate, 0.5)

    def test_empty_input_returns_zeroes(self):
        metrics = calculate([])
        self.assertEqual(metrics.records, 0)
        self.assertEqual(metrics.outcome_success_rate, 0.0)

    def test_reads_jsonl_and_ignores_blank_lines(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "records.jsonl"
            path.write_text(json.dumps({"outcome_status": "success"}) + "\n\n", encoding="utf-8")
            self.assertEqual(len(read_jsonl(path)), 1)


if __name__ == "__main__":
    unittest.main()
