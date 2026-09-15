export const SCHEMA_VERSION_V1 = "1.0" as const;

export type JsonValue =
  | null
  | boolean
  | number
  | string
  | JsonValue[]
  | { [key: string]: JsonValue };

export type ScopeV1 = Record<string, string | number | boolean>;

export type ActionV1 = {
  action: string;
  parameters: JsonValue;
};

export type IntentCandidateV1 = {
  intent: string;
  confidence: number;
  source: "explicit_user" | "host" | "host_model" | "inferred";
  constraints: string[];
  evidence_refs: string[];
};

export type HumanModelAssertionV1 = {
  assertion_id: string;
  subject_ref: string;
  kind:
    | "preference"
    | "constraint"
    | "expertise"
    | "goal"
    | "workflow"
    | "decision_pattern"
    | "interaction_pattern";
  predicate: string;
  value: JsonValue;
  scope: ScopeV1;
  confidence: number;
  source_type: "explicit" | "inferred";
  evidence_refs: string[];
  status: "candidate" | "active" | "inactive" | "superseded" | "contradicted";
  valid_from: string;
  valid_until: string | null;
  recorded_at: string;
  last_confirmed_at: string | null;
  supersedes: string[];
  contradicts: string[];
};

export type EvidenceV1 = {
  evidence_id: string;
  kind: "explicit_statement" | "implicit_behavior" | "decision_override" | "outcome";
  strength: number;
  source: string;
  observed_at: string;
  data: JsonValue;
};

export type ResolveRequestV1 = {
  schema_version: typeof SCHEMA_VERSION_V1;
  request_id: string;
  subject_ref: string;
  input: JsonValue;
  intent_candidates: IntentCandidateV1[];
  context: {
    scope: ScopeV1;
    current_constraints: string[];
    data: JsonValue;
  };
  human_model_refs: string[];
  human_model_snapshot: HumanModelAssertionV1[];
  evidence: EvidenceV1[];
  host_capabilities: {
    model_inference: boolean;
    supported_actions: string[];
  };
};

export type DecisionContractV1 = {
  schema_version: typeof SCHEMA_VERSION_V1;
  decision_id: string;
  decision_digest: string;
  request_id: string;
  resolved_intent: string;
  intent_confidence: number;
  recommended_action: ActionV1;
  alternatives: ActionV1[];
  constraints: string[];
  ambiguities: string[];
  human_model_basis: string[];
  evidence_basis: string[];
  reason_codes: string[];
  decision_confidence: number;
  model_usage: "not_required" | "host_supplied" | "host_delegated";
};

export type HostModelRequestV1 = {
  request_id: string;
  purpose: "intent_resolution" | "intent_disambiguation" | "decision_resolution";
  structured_context: JsonValue;
  expected_response_schema: JsonValue;
};

export type HostModelResultV1 = {
  request_id: string;
  resolved_intent: string;
  recommended_action: ActionV1;
  alternatives: ActionV1[];
  constraints: string[];
  ambiguities: string[];
  confidence: number;
};

export type UnresolvedResultV1 = {
  request_id: string;
  resolved_intent: string | null;
  reason: "unsupported_action" | "model_inference_unavailable";
  reason_codes: string[];
};

export type ResolveOutcomeV1 =
  | { type: "decision"; decision: DecisionContractV1 }
  | { type: "model_inference_required"; model_request: HostModelRequestV1 }
  | { type: "unresolved"; unresolved: UnresolvedResultV1 };

export type ContinueResolveRequestV1 = {
  original_request: ResolveRequestV1;
  model_result: HostModelResultV1;
};

export type OutcomeFeedbackV1 = {
  decision_id: string;
  decision_digest: string;
  recommended_action: ActionV1;
  actual_action: ActionV1;
  user_response: "accepted" | "rejected" | "corrected" | "ignored";
  outcome: {
    status: "success" | "failure" | "partial" | "unknown";
    details: JsonValue;
  };
  correction: {
    corrected_intent: string | null;
    preferred_action: ActionV1 | null;
  } | null;
  scope: ScopeV1;
  observed_at: string;
};

export type EvaluationRecordV1 = {
  decision_id: string;
  intent_corrected: boolean;
  decision_overridden: boolean;
  clarification_required: boolean;
  model_inference_used: boolean;
  human_model_assertion_refs: string[];
  evidence_refs: string[];
  outcome_status: "pending" | "success" | "failure" | "partial" | "unknown";
  resolved_at: string;
};

export type FeedbackResultV1 = {
  evidence: EvidenceV1[];
  assertions_updated: string[];
  evaluation_record: EvaluationRecordV1;
};
