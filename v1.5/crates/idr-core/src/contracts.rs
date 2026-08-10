use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const SCHEMA_VERSION_V1: &str = "1.0";

fn schema_version() -> String {
    SCHEMA_VERSION_V1.to_owned()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(transparent)]
pub struct ScopeV1(pub BTreeMap<String, Value>);

impl ScopeV1 {
    pub fn matches(&self, request_scope: &Self) -> bool {
        self.0
            .iter()
            .all(|(key, value)| request_scope.0.get(key) == Some(value))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ActionV1 {
    pub action: String,
    #[serde(default)]
    pub parameters: Value,
}

impl ActionV1 {
    pub fn new(action: impl Into<String>) -> Self {
        Self {
            action: action.into(),
            parameters: Value::Object(Default::default()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IntentSourceV1 {
    ExplicitUser,
    Host,
    HostModel,
    Inferred,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct IntentCandidateV1 {
    pub intent: String,
    pub confidence: f64,
    pub source: IntentSourceV1,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ContextV1 {
    #[serde(default)]
    pub scope: ScopeV1,
    #[serde(default)]
    pub current_constraints: Vec<String>,
    #[serde(default)]
    pub data: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HostCapabilitiesV1 {
    pub model_inference: bool,
    #[serde(default)]
    pub supported_actions: Vec<String>,
}

impl Default for HostCapabilitiesV1 {
    fn default() -> Self {
        Self {
            model_inference: true,
            supported_actions: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKindV1 {
    ExplicitStatement,
    ImplicitBehavior,
    DecisionOverride,
    Outcome,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EvidenceV1 {
    pub evidence_id: String,
    pub kind: EvidenceKindV1,
    pub strength: f64,
    pub source: String,
    pub observed_at: String,
    #[serde(default)]
    pub data: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ResolveRequestV1 {
    #[serde(default = "schema_version")]
    pub schema_version: String,
    pub request_id: String,
    pub subject_ref: String,
    #[serde(default)]
    pub input: Value,
    #[serde(default)]
    pub intent_candidates: Vec<IntentCandidateV1>,
    #[serde(default)]
    pub context: ContextV1,
    #[serde(default)]
    pub human_model_refs: Vec<String>,
    #[serde(default)]
    pub human_model_snapshot: Vec<HumanModelAssertionV1>,
    #[serde(default)]
    pub evidence: Vec<EvidenceV1>,
    #[serde(default)]
    pub host_capabilities: HostCapabilitiesV1,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelUsageV1 {
    NotRequired,
    HostSupplied,
    HostDelegated,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DecisionContractV1 {
    #[serde(default = "schema_version")]
    pub schema_version: String,
    pub decision_id: String,
    pub decision_digest: String,
    pub request_id: String,
    pub resolved_intent: String,
    pub intent_confidence: f64,
    pub recommended_action: ActionV1,
    #[serde(default)]
    pub alternatives: Vec<ActionV1>,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub ambiguities: Vec<String>,
    #[serde(default)]
    pub human_model_basis: Vec<String>,
    #[serde(default)]
    pub evidence_basis: Vec<String>,
    #[serde(default)]
    pub reason_codes: Vec<String>,
    pub decision_confidence: f64,
    pub model_usage: ModelUsageV1,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HostModelPurposeV1 {
    IntentResolution,
    IntentDisambiguation,
    DecisionResolution,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HostModelRequestV1 {
    pub request_id: String,
    pub purpose: HostModelPurposeV1,
    pub structured_context: Value,
    pub expected_response_schema: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HostModelResultV1 {
    pub request_id: String,
    pub resolved_intent: String,
    pub recommended_action: ActionV1,
    #[serde(default)]
    pub alternatives: Vec<ActionV1>,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub ambiguities: Vec<String>,
    pub confidence: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UnresolvedReasonV1 {
    UnsupportedAction,
    ModelInferenceUnavailable,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UnresolvedResultV1 {
    pub request_id: String,
    pub resolved_intent: Option<String>,
    pub reason: UnresolvedReasonV1,
    #[serde(default)]
    pub reason_codes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResolveOutcomeV1 {
    Decision { decision: Box<DecisionContractV1> },
    ModelInferenceRequired { model_request: HostModelRequestV1 },
    Unresolved { unresolved: UnresolvedResultV1 },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ContinueResolveRequestV1 {
    pub original_request: ResolveRequestV1,
    pub model_result: HostModelResultV1,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UserResponseV1 {
    Accepted,
    Rejected,
    Corrected,
    Ignored,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeStatusV1 {
    Success,
    Failure,
    Partial,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct OutcomeV1 {
    pub status: OutcomeStatusV1,
    #[serde(default)]
    pub details: Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct CorrectionV1 {
    pub corrected_intent: Option<String>,
    pub preferred_action: Option<ActionV1>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct OutcomeFeedbackV1 {
    pub decision_id: String,
    pub decision_digest: String,
    pub recommended_action: ActionV1,
    pub actual_action: ActionV1,
    pub user_response: UserResponseV1,
    pub outcome: OutcomeV1,
    pub correction: Option<CorrectionV1>,
    pub scope: ScopeV1,
    pub observed_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssertionKindV1 {
    Preference,
    Constraint,
    Expertise,
    Goal,
    Workflow,
    DecisionPattern,
    InteractionPattern,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssertionSourceTypeV1 {
    Explicit,
    Inferred,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssertionStatusV1 {
    Candidate,
    Active,
    Inactive,
    Superseded,
    Contradicted,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HumanModelAssertionV1 {
    pub assertion_id: String,
    pub subject_ref: String,
    pub kind: AssertionKindV1,
    pub predicate: String,
    pub value: Value,
    pub scope: ScopeV1,
    pub confidence: f64,
    pub source_type: AssertionSourceTypeV1,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    pub status: AssertionStatusV1,
    pub valid_from: String,
    pub valid_until: Option<String>,
    pub recorded_at: String,
    pub last_confirmed_at: Option<String>,
    #[serde(default)]
    pub supersedes: Vec<String>,
    #[serde(default)]
    pub contradicts: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ObservationV1 {
    pub evidence_id: String,
    pub subject_ref: String,
    pub kind: AssertionKindV1,
    pub predicate: String,
    pub value: Value,
    pub scope: ScopeV1,
    pub source_type: AssertionSourceTypeV1,
    pub observed_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct QueryHumanModelRequestV1 {
    pub subject_ref: String,
    #[serde(default)]
    pub scope: ScopeV1,
    pub as_of: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationOutcomeStatusV1 {
    Pending,
    Success,
    Failure,
    Partial,
    Unknown,
}

impl From<OutcomeStatusV1> for EvaluationOutcomeStatusV1 {
    fn from(value: OutcomeStatusV1) -> Self {
        match value {
            OutcomeStatusV1::Success => Self::Success,
            OutcomeStatusV1::Failure => Self::Failure,
            OutcomeStatusV1::Partial => Self::Partial,
            OutcomeStatusV1::Unknown => Self::Unknown,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EvaluationRecordV1 {
    pub decision_id: String,
    pub intent_corrected: bool,
    pub decision_overridden: bool,
    pub clarification_required: bool,
    pub model_inference_used: bool,
    #[serde(default)]
    pub human_model_assertion_refs: Vec<String>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    pub outcome_status: EvaluationOutcomeStatusV1,
    pub resolved_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FeedbackResultV1 {
    pub evidence: Vec<EvidenceV1>,
    pub assertions_updated: Vec<String>,
    pub evaluation_record: EvaluationRecordV1,
}
