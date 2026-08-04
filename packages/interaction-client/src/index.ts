export * from "./generated/idr-production-v1.ts";

export type SourceActor =
  | "user"
  | "host"
  | "agent"
  | "tool"
  | "external_system"
  | "human_approver"
  | "scheduler"
  | "policy_engine"
  | "device_environment";

export type SemanticRole =
  | "objective"
  | "intent"
  | "command"
  | "fact"
  | "observation"
  | "proposal"
  | "authorization"
  | "policy"
  | "result"
  | "feedback"
  | "time_trigger"
  | "security_signal";

export type CanonicalInputEventV1 = {
  event_id: string;
  run_id: string;
  turn_id: string;
  source_actor: SourceActor;
  actor_ref: string;
  primary_semantic_role: SemanticRole;
  semantic_roles: SemanticRole[];
  content_ref: string;
  content_digest: string;
  correlation_ref: string;
  logical_time: number;
  schema_version: 1;
};

export type IntentFastPathOutcome =
  | "allow_fast_path"
  | "unknown"
  | "block";

export type DecisionNecessityOutcome =
  | "not_required"
  | "required"
  | "unknown";

export type TurnCoordinationMode =
  | "respond_only"
  | "act_then_respond"
  | "respond_then_act"
  | "respond_then_confirm_then_act"
  | "parallel"
  | "acknowledge_then_run"
  | "stream_progress_then_final";

export type ActionPlanningPosture =
  | "none"
  | "blocked"
  | "planned"
  | "waiting_authorization";

export type InteractionRunState =
  | "pending"
  | "running"
  | "waiting_input"
  | "waiting_authorization"
  | "waiting_dependency"
  | "succeeded"
  | "rejected"
  | "failed"
  | "cancelled"
  | "timed_out"
  | "invalidated";

export type GuardDecision<TOutcome extends string> = {
  outcome: TOutcome;
  reason_codes: string[];
  rule_version: number;
};

export type DecisionNecessityDecisionV1 =
  GuardDecision<DecisionNecessityOutcome> & {
    enters_decision_runtime: boolean;
  };

export type InteractionAssessmentV1 = {
  schema_version: 1;
  fast_path: GuardDecision<IntentFastPathOutcome>;
  decision_necessity: DecisionNecessityDecisionV1;
  coordination_mode: TurnCoordinationMode;
  action_posture: ActionPlanningPosture;
  next_run_state: InteractionRunState;
};

export type ExactActionRefV1 = {
  kind: "action";
  contract_id: string;
  revision: number;
  record_digest: string;
};

export type ExactActionAuthorizationCandidateV1 = {
  schema_version: 1;
  action_ref: ExactActionRefV1;
  parameter_digest: string;
  decision: "approve" | "deny";
};

const sourceActors = new Set<SourceActor>([
  "user",
  "host",
  "agent",
  "tool",
  "external_system",
  "human_approver",
  "scheduler",
  "policy_engine",
  "device_environment",
]);

const semanticRoles = new Set<SemanticRole>([
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
]);

const fastPathOutcomes = new Set<IntentFastPathOutcome>([
  "allow_fast_path",
  "unknown",
  "block",
]);

const decisionOutcomes = new Set<DecisionNecessityOutcome>([
  "not_required",
  "required",
  "unknown",
]);

const coordinationModes = new Set<TurnCoordinationMode>([
  "respond_only",
  "act_then_respond",
  "respond_then_act",
  "respond_then_confirm_then_act",
  "parallel",
  "acknowledge_then_run",
  "stream_progress_then_final",
]);

const actionPostures = new Set<ActionPlanningPosture>([
  "none",
  "blocked",
  "planned",
  "waiting_authorization",
]);

const runStates = new Set<InteractionRunState>([
  "pending",
  "running",
  "waiting_input",
  "waiting_authorization",
  "waiting_dependency",
  "succeeded",
  "rejected",
  "failed",
  "cancelled",
  "timed_out",
  "invalidated",
]);

export function parseCanonicalInputEvent(
  value: unknown,
): CanonicalInputEventV1 {
  const input = asRecord(value, "canonical input");
  rejectUnknownFields(
    input,
    new Set([
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
    ]),
    "canonical input",
  );
  const sourceActor = requiredEnum(
    input.source_actor,
    sourceActors,
    "source_actor",
  );
  const primaryRole = requiredEnum(
    input.primary_semantic_role,
    semanticRoles,
    "primary_semantic_role",
  );
  if (!Array.isArray(input.semantic_roles)) {
    throw new TypeError("semantic_roles must be an array");
  }
  const roles = input.semantic_roles.map((role, index) =>
    requiredEnum(role, semanticRoles, `semantic_roles[${index}]`),
  );
  if (
    roles.length === 0 ||
    roles.length > 16 ||
    new Set(roles).size !== roles.length ||
    !roles.includes(primaryRole) ||
    roles.some((role) => !sourceActorMayAssert(sourceActor, role))
  ) {
    throw new TypeError(
      "semantic_roles must be allowed for source_actor, unique, and contain primary_semantic_role",
    );
  }
  return {
    event_id: requiredUuid(input.event_id, "event_id"),
    run_id: requiredUuid(input.run_id, "run_id"),
    turn_id: requiredUuid(input.turn_id, "turn_id"),
    source_actor: sourceActor,
    actor_ref: requiredReference(input.actor_ref, "actor_ref"),
    primary_semantic_role: primaryRole,
    semantic_roles: roles,
    content_ref: requiredReference(input.content_ref, "content_ref"),
    content_digest: requiredContentDigest(input.content_digest),
    correlation_ref: requiredReference(input.correlation_ref, "correlation_ref"),
    logical_time: requiredPositiveInteger(input.logical_time, "logical_time"),
    schema_version: requiredSchemaVersion(input.schema_version),
  };
}

export function parseInteractionAssessment(
  value: unknown,
): InteractionAssessmentV1 {
  const root = asRecord(value, "interaction assessment");
  rejectUnknownFields(
    root,
    new Set([
      "schema_version",
      "fast_path",
      "decision_necessity",
      "coordination_mode",
      "action_posture",
      "next_run_state",
    ]),
    "interaction assessment",
  );
  const fastPath = parseGuardDecision(
    root.fast_path,
    fastPathOutcomes,
    "fast_path",
  );
  const decisionRoot = asRecord(
    root.decision_necessity,
    "decision_necessity",
  );
  const decision = {
    ...parseGuardDecision(
      decisionRoot,
      decisionOutcomes,
      "decision_necessity",
    ),
    enters_decision_runtime: requiredBoolean(
      decisionRoot.enters_decision_runtime,
      "enters_decision_runtime",
    ),
  };
  const assessment: InteractionAssessmentV1 = {
    schema_version: requiredSchemaVersion(root.schema_version),
    fast_path: fastPath,
    decision_necessity: decision,
    coordination_mode: requiredEnum(
      root.coordination_mode,
      coordinationModes,
      "coordination_mode",
    ),
    action_posture: requiredEnum(
      root.action_posture,
      actionPostures,
      "action_posture",
    ),
    next_run_state: requiredEnum(
      root.next_run_state,
      runStates,
      "next_run_state",
    ),
  };
  validateAssessmentInvariants(assessment);
  return assessment;
}

export function buildExactActionAuthorizationCandidate(input: {
  actionRef: ExactActionRefV1;
  parameterDigest: string;
  decision: "approve" | "deny";
}): ExactActionAuthorizationCandidateV1 {
  rejectUnknownFields(
    input as unknown as Record<string, unknown>,
    new Set(["actionRef", "parameterDigest", "decision"]),
    "authorization submission",
  );
  validateActionRef(input.actionRef);
  requireSha256(input.parameterDigest, "parameterDigest");
  const decision = requiredEnum(
    input.decision,
    new Set(["approve", "deny"] as const),
    "decision",
  );
  return {
    schema_version: 1,
    action_ref: { ...input.actionRef },
    parameter_digest: input.parameterDigest,
    decision,
  };
}

function parseGuardDecision<T extends string>(
  value: unknown,
  allowed: ReadonlySet<T>,
  field: string,
): GuardDecision<T> {
  const record = asRecord(value, field);
  rejectUnknownFields(
    record,
    new Set([
      "outcome",
      "reason_codes",
      "rule_version",
      ...(field === "decision_necessity"
        ? ["enters_decision_runtime"]
        : []),
    ]),
    field,
  );
  if (!Array.isArray(record.reason_codes)) {
    throw new TypeError(`${field}.reason_codes must be an array`);
  }
  return {
    outcome: requiredEnum(record.outcome, allowed, `${field}.outcome`),
    reason_codes: record.reason_codes.map((reason, index) =>
      requiredText(reason, `${field}.reason_codes[${index}]`),
    ),
    rule_version: requiredPositiveInteger(
      record.rule_version,
      `${field}.rule_version`,
    ),
  };
}

function validateActionRef(value: ExactActionRefV1): void {
  const record = asRecord(value, "actionRef");
  rejectUnknownFields(
    record,
    new Set(["kind", "contract_id", "revision", "record_digest"]),
    "actionRef",
  );
  if (value.kind !== "action") {
    throw new TypeError("actionRef.kind must be action");
  }
  requiredUuid(value.contract_id, "actionRef.contract_id");
  requiredPositiveInteger(value.revision, "actionRef.revision");
  requireSha256(value.record_digest, "actionRef.record_digest");
}

function requireSha256(value: unknown, field: string): string {
  const text = requiredText(value, field);
  if (!/^[a-f0-9]{64}$/.test(text)) {
    throw new TypeError(`${field} must be a 64-character SHA-256 digest`);
  }
  return text;
}

function requiredContentDigest(value: unknown): string {
  const text = requiredText(value, "content_digest");
  if (!/^sha256:[a-f0-9]{64}$/.test(text)) {
    throw new TypeError(
      "content_digest must use the sha256:<64 hex characters> format",
    );
  }
  return text;
}

function requiredSchemaVersion(value: unknown): 1 {
  if (value !== 1) throw new TypeError("schema_version must be 1");
  return 1;
}

function requiredBoolean(value: unknown, field: string): boolean {
  if (typeof value !== "boolean") {
    throw new TypeError(`${field} must be a boolean`);
  }
  return value;
}

function requiredPositiveInteger(value: unknown, field: string): number {
  if (!Number.isSafeInteger(value) || Number(value) <= 0) {
    throw new TypeError(`${field} must be a positive safe integer`);
  }
  return Number(value);
}

function requiredText(value: unknown, field: string): string {
  if (
    typeof value !== "string" ||
    value.trim().length === 0 ||
    value.length > 4_096
  ) {
    throw new TypeError(`${field} must be non-empty text`);
  }
  return value;
}

function requiredReference(value: unknown, field: string): string {
  const text = requiredText(value, field);
  if (
    text.length > 512 ||
    text.trim() !== text ||
    /[\u0000-\u001f\u007f-\u009f]/u.test(text)
  ) {
    throw new TypeError(`${field} must be a canonical reference`);
  }
  return text;
}

function sourceActorMayAssert(
  source: SourceActor,
  role: SemanticRole,
): boolean {
  const allowed: Record<SourceActor, ReadonlySet<SemanticRole>> = {
    user: new Set([
      "objective",
      "intent",
      "command",
      "fact",
      "proposal",
      "authorization",
      "feedback",
    ]),
    host: new Set([
      "objective",
      "intent",
      "command",
      "fact",
      "observation",
      "proposal",
      "policy",
      "result",
      "feedback",
      "time_trigger",
      "security_signal",
    ]),
    agent: new Set(["fact", "observation", "proposal", "result", "feedback"]),
    tool: new Set(["fact", "observation", "result", "security_signal"]),
    external_system: new Set([
      "fact",
      "observation",
      "result",
      "security_signal",
    ]),
    human_approver: new Set(["authorization", "feedback", "fact"]),
    scheduler: new Set(["time_trigger"]),
    policy_engine: new Set(["policy", "authorization", "security_signal"]),
    device_environment: new Set(["observation", "fact", "security_signal"]),
  };
  return allowed[source].has(role);
}

function requiredUuid<T extends string>(
  value: unknown,
  field: string,
): T {
  const text = requiredText(value, field);
  if (
    !/^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(
      text,
    )
  ) {
    throw new TypeError(`${field} must be a UUID`);
  }
  return text.toLowerCase() as T;
}

function requiredEnum<T extends string>(
  value: unknown,
  allowed: ReadonlySet<T>,
  field: string,
): T {
  if (typeof value !== "string" || !allowed.has(value as T)) {
    throw new TypeError(`${field} is unsupported`);
  }
  return value as T;
}

function asRecord(
  value: unknown,
  field: string,
): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new TypeError(`${field} must be an object`);
  }
  return value as Record<string, unknown>;
}

function rejectUnknownFields(
  value: Record<string, unknown>,
  allowed: ReadonlySet<string>,
  field: string,
): void {
  const unknown = Object.keys(value).filter((key) => !allowed.has(key));
  if (unknown.length > 0) {
    throw new TypeError(`${field} contains unknown fields: ${unknown.join(", ")}`);
  }
}

function validateAssessmentInvariants(
  assessment: InteractionAssessmentV1,
): void {
  const decisionEntryExpected =
    assessment.decision_necessity.outcome !== "not_required";
  if (
    assessment.decision_necessity.enters_decision_runtime !==
      decisionEntryExpected ||
    (assessment.fast_path.outcome === "allow_fast_path" &&
      assessment.decision_necessity.enters_decision_runtime)
  ) {
    throw new TypeError("interaction assessment contains contradictory guard decisions");
  }

  if (assessment.fast_path.outcome === "block") {
    if (
      assessment.coordination_mode !== "respond_only" ||
      assessment.action_posture !== "blocked" ||
      assessment.next_run_state !== "rejected"
    ) {
      throw new TypeError("blocked assessment has an invalid terminal posture");
    }
    return;
  }

  const expected =
    assessment.coordination_mode === "respond_only"
      ? (["none", "running"] as const)
      : assessment.coordination_mode === "respond_then_confirm_then_act"
        ? (["waiting_authorization", "waiting_authorization"] as const)
        : (["planned", "running"] as const);
  if (
    assessment.action_posture !== expected[0] ||
    assessment.next_run_state !== expected[1]
  ) {
    throw new TypeError("interaction assessment posture does not match coordination mode");
  }
}
