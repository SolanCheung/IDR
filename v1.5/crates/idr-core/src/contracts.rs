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
    pub host_capabilities: z÷ž­¢G§²ÚîÆ­yÔ,
        _ => 0.65,
    }
}

fn outcome_strength(status: &OutcomeStatusV1) -> f64 {
    match status {
        OutcomeStatusV1::Success | OutcomeStatusV1::Failure => 0.9,
        OutcomeStatusV1::Partial => 0.65,
        OutcomeStatusV1::Unknown => 0.25,
    }
}

fn source_rank(source: &AssertionSourceTypeV1) -> u8 {
    match source {
        AssertionSourceTypeV1::Explicit => 2,
        AssertionSourceTypeV1::Inferred => 1,
    }
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.contains(&value) {
        values.push(value);
    }
}
