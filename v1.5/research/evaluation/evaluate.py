"""Compute V1 metrics from EvaluationRecordV1 JSONL without third-party packages."""

from __future__ import annotations

import argparse
import json
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Iterable, Mapping, Any


@dataclass(frozen=True)
class Metrics:
    records: int
    intent_correction_rate: float
    decision_override_rate: float
    reclarification_rate: float
    outcome_success_rate: float
    host_model_invocation_rate: float


def read_jsonl(path: Path) -> list[Mapping[str, Any]]:
    records: list[Mapping[str, Any]] = []
    with path.open("r", encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, start=1):
            if not line.strip():
                continue
            value = json.loads(line)
            if not isinstance(value, dict):
                raise ValueError(f"line {line_number} is not a JSON object")
            records.append(value)
    return records


def calculate(records: Iterable[Mapping[str, Any]]) -> Metrics:
    values = list(records)
    total = len(values)
    if total == 0:
        return Metrics(0, 0.0, 0.0, 0.0, 0.0, 0.0)

    def rate(predicate) -> float:
        return sum(1 for record in values if predicate(record)) / total

    return Metrics(
        records=total,
        intent_correction_rate=rate(lambda record: record.get("intent_corrected") is True),
        decision_override_rate=rate(lambda record: record.get("decision_overridden") is True),
        reclarification_rate=rate(lambda record: record.get("clarification_required") is True),
        outcome_success_rate=rate(lambda record: record.get("outcome_status") == "success"),
        host_model_invocation_rate=rate(lambda record: record.get("model_inference_used") is True),
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("jsonl", type=Path, help="EvaluationRecordV1 JSONL file")
    args = parser.parse_args()
    print(json.dumps(asdict(calculate(read_jsonl(args.jsonl))), indent=2, sort_keys=True))


if __name__ == "__main__":
    main()

