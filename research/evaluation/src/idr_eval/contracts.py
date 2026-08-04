from dataclasses import dataclass
import re
from typing import Any, Dict, FrozenSet, Tuple
import uuid


SEMANTIC_ROLES: FrozenSet[str] = frozenset(
    {
        "objective",
        "intent",
        "command",
        "fact",
        "observation",
        "proposal",
        "authorization",
        "policy",
        "result",
        "feedback",
        "time_trigger",
        "security_signal",
    }
)

SOURCE_ACTOR_ROLES: Dict[str, FrozenSet[str]] = {
    "user": frozenset(
        {"objective", "intent", "command", "fact", "proposal", "authorization", "feedback"}
    ),
    "host": SEMANTIC_ROLES - {"authorization"},
    "agent": frozenset({"fact", "observation", "proposal", "result", "feedback"}),
    "tool": frozenset({"fact", "observation", "result", "security_signal"}),
    "external_system": frozenset({"fact", "observation", "result", "security_signal"}),
    "human_approver": frozenset({"authorization", "feedback", "fact"}),
    "scheduler": frozenset({"time_trigger"}),
    "policy_engine": frozenset({"policy", "authorization", "security_signal"}),
    "device_environment": frozenset({"observation", "fact", "security_signal"}),
}

CANONICAL_INPUT_FIELDS: FrozenSet[str] = frozenset(
    {
        "event_id",
        "run_id",
        "turn_id",
        "source_actor",
        "actor_ref",
        "primary_semantic_role",
        "semantic_roles",
        "content_ref",
        "content_digest",
        "correlation_ref",
        "logical_time",
        "schema_version",
    }
)
MAX_SAFE_WIRE_INTEGER = 9_007_199_254_740_991


@dataclass(frozen=True)
class CanonicalInputObservation:
    event_id: str
    run_id: str
    turn_id: str
    source_actor: str
    actor_ref: str
    primary_semantic_role: str
    semantic_roles: Tuple[str, ...]
    content_ref: str
    content_digest: str
    correlation_ref: str
    logical_time: int
    schema_version: int

    @classmethod
    def from_mapping(cls, value: Dict[str, Any]) -> "CanonicalInputObservation":
        unknown_fields = set(value) - CANONICAL_INPUT_FIELDS
        if unknown_fields:
            raise ValueError(
                f"canonical input contains unknown fields: {sorted(unknown_fields)}"
            )
        roles = tuple(value.get("semantic_roles", ()))
        primary = value.get("primary_semantic_role")
        source_actor = value.get("source_actor")
        if (
            not roles
            or len(roles) > 16
            or len(set(roles)) != len(roles)
            or primary not in roles
            or any(role not in SEMANTIC_ROLES for role in roles)
            or source_actor not in SOURCE_ACTOR_ROLES
            or any(role not in SOURCE_ACTOR_ROLES[source_actor] for role in roles)
        ):
            raise ValueError(
                "semantic roles must be unique, supported, and contain the primary role"
            )
        event_id = _required_uuid(value.get("event_id"), "event_id")
        run_id = _required_uuid(value.get("run_id"), "run_id")
        turn_id = _required_uuid(value.get("turn_id"), "turn_id")
        actor_ref = _required_reference(value.get("actor_ref"), "actor_ref")
        content_ref = _required_reference(value.get("content_ref"), "content_ref")
        content_digest = value.get("content_digest")
        if not isinstance(source_actor, str) or not source_actor.strip():
            raise ValueError("source_actor is required")
        if not isinstance(content_digest, str) or re.fullmatch(
            r"sha256:[0-9a-f]{64}", content_digest
        ) is None:
            raise ValueError("content_digest must be a SHA-256 reference")
        correlation_ref = _required_reference(
            value.get("correlation_ref"), "correlation_ref"
        )
        logical_time = value.get("logical_time")
        if (
            not isinstance(logical_time, int)
            or isinstance(logical_time, bool)
            or logical_time <= 0
            or logical_time > MAX_SAFE_WIRE_INTEGER
        ):
            raise ValueError("logical_time must be a positive safe integer")
        if value.get("schema_version") != 1:
            raise ValueError("schema_version must be 1")
        return cls(
            event_id=event_id,
            run_id=run_id,
            turn_id=turn_id,
            source_actor=source_actor,
            actor_ref=actor_ref,
            primary_semantic_role=primary,
            semantic_roles=roles,
            content_ref=content_ref,
            content_digest=content_digest,
            correlation_ref=correlation_ref,
            logical_time=logical_time,
            schema_version=1,
        )


def _required_uuid(value: Any, field: str) -> str:
    if not isinstance(value, str):
        raise ValueError(f"{field} must be a UUID")
    try:
        parsed = uuid.UUID(value)
    except (ValueError, AttributeError) as error:
        raise ValueError(f"{field} must be a UUID") from error
    if (
        parsed.int == 0
        or parsed.variant != uuid.RFC_4122
        or parsed.version is None
        or not 1 <= parsed.version <= 8
    ):
        raise ValueError(f"{field} must be an RFC UUID version 1 through 8")
    return str(parsed)


def _required_reference(value: Any, field: str) -> str:
    if (
        not isinstance(value, str)
        or not value
        or value.strip() != value
        or len(value) > 512
        or any(ord(character) < 32 or 127 <= ord(character) <= 159 for character in value)
    ):
        raise ValueError(f"{field} must be a canonical reference")
    return value
