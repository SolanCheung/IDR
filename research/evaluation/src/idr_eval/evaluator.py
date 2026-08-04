from dataclasses import dataclass
from typing import Any, Dict, List, Mapping, Optional

from .contracts import CanonicalInputObservation


@dataclass(frozen=True)
class EvaluationReport:
    passed: bool
    score_basis_points: int
    violations: List[str]


def evaluate_runtime_output(
    fixture: Mapping[str, Any],
    runtime_output: Mapping[str, Any],
    intent_projection: Optional[Mapping[str, Any]] = None,
) -> EvaluationReport:
    """Score recorded runtime output without participating in runtime decisions."""

    violations: List[str] = []
    CanonicalInputObservation.from_mapping(dict(fixture["input"]))
    expected = fixture["expected"]

    comparisons = {
        "fast_path_outcome": _nested(runtime_output, "fast_path", "outcome"),
        "decision_necessity": _nested(
            runtime_output, "decision_necessity", "outcome"
        ),
        "coordination_mode": runtime_output.get("coordination_mode"),
        "action_posture": runtime_output.get("action_posture"),
        "next_run_state": runtime_output.get("next_run_state"),
    }
    for field, actual in comparisons.items():
        if actual != expected[field]:
            violations.append(
                "{} mismatch: expected {!r}, found {!r}".format(
                    field, expected[field], actual
                )
            )

    projection = intent_projection or fixture.get("intent_projection", {})
    base = set(projection.get("base_hypothesis_refs", ()))
    personalized = set(projection.get("personalized_hypothesis_refs", ()))
    if not base:
        violations.append("base intent hypotheses are missing")
    elif not base.issubset(personalized):
        missing = sorted(base - personalized)
        violations.append(
            "personalization deleted base hypotheses: {}".format(", ".join(missing))
        )

    confirmation_required = (
        expected["coordination_mode"] == "respond_then_confirm_then_act"
    )
    if confirmation_required and (
        runtime_output.get("action_posture") != "waiting_authorization"
        or runtime_output.get("next_run_state") != "waiting_authorization"
    ):
        violations.append("confirmation-required action did not stop at authorization")

    checks = 7
    failures = min(checks, len(violations))
    score = ((checks - failures) * 10_000) // checks
    return EvaluationReport(
        passed=not violations,
        score_basis_points=score,
        violations=violations,
    )


def _nested(value: Mapping[str, Any], first: str, second: str) -> Any:
    child = value.get(first)
    if not isinstance(child, Mapping):
        return None
    return child.get(second)
