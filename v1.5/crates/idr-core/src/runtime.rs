use crate::*;
use crate::learning::assertion_applies;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

const MIN_INTENT_CONFIDENCE: f64 = 0.75;
const AMBIGUITY_DELTA: f64 = 0.08;

#[derive(Clone, Debug)]
pub(crate) struct StoredDecision {
    pub subject_ref: String,
    pub decision: DecisionContractV1,
}

#[derive(Debug, Default)]
pub struct IdrCore {
    pub(crate) assertions: Vec<HumanModelAssertionV1>,
    pub(crate) evidence: Vec<EvidenceV1>,
    pub(crate) decisions: HashMap<String, StoredDecision>,
    pub(crate) evaluations: Vec<EvaluationRecordV1>,
    pub(crate) next_sequence: u64,
}

enum IntentResolution<'a> {
    Resolved(&'a IntentCandidateV1),
    NeedsModel(HostModelPurposeV1),
}

impl IdrCore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn resolve(&mut self, request: ResolveRequestV1) -> Result<ResolveOutcomeV1, IdrError> {
        validate_resolve_request(&request)?;
        match resolve_intent(&request) {
            IntentResolution::Resolved(candidate) => {
                let model_usage = if candidate.source == IntentSourceV1::HostModel {
                    ModelUsageV1::HostSupplied
                } else {
                    ModelUsageV1::NotRequired
                };
                let decision = self.build_decision(
                    &request,
                    candidate.intent.clone(),
                    candidate.confidence,
                    default_action(&request, &candidate.intent),
                    Vec::new(),
                    candidate.constraints.clone(),
                    Vec::new(),
                    model_usage,
                    false,
                );
                Ok(ResolveOutcomeV1::Decision {
                    decision: Box::new(decision),
                })
            }
            IntentResolution::NeedsModel(purpose) => {
                Ok(ResolveOutcomeV1::ModelInferenceRequired {
                    model_request: host_model_request(&request, purpose),
                })
            }
        }
    }

    pub fn continue_resolve(
        &mut self,
        continuation: ContinueResolveRequestV1,
    ) -> Result<DecisionContractV1, IdrError> {
        validate_resolve_request(&continuation.original_request)?;
        validate_model_result(
            &continuation.original_request.request_id,
            &continuation.model_result,
        )?;

        let clarification_required = matches!(
            resolve_intent(&continuation.original_request),
            IntentResolution::NeedsModel(HostModelPurposeV1::IntentDisambiguation)
        );
        let result = continuation.model_result;
        Ok(self.build_decision(
            &continuation.original_request,
            result.resolved_intent,
            result.confidence,
            result.recommended_action,
            result.alternatives,
            result.constraints,
            result.ambiguities,
            ModelUsageV1::HostDelegated,
            clarification_required,
        ))
    }

    pub async fn resolve_with_provider<P: HostModelProvider>(
        &mut self,
        request: ResolveRequestV1,
        provider: &P,
    ) -> Result<DecisionContractV1, IdrError> {
        match self.resolve(request.clone())? {
            ResolveOutcomeV1::Decision { decision } => Ok(*decision),
            ResolveOutcomeV1::ModelInferenceRequired { model_request } => {
                let model_result = provider
                    .infer(model_request)
                    .await
                    .map_err(|error| IdrError::HostModel(error.to_string()))?;
                self.continue_resolve(ContinueResolveRequestV1 {
                    original_request: request,
                    model_result,
                })
            }
        }
    }

    pub fn evaluation_records(&self) -> &[EvaluationRecordV1] {
        &self.evaluations
    }

    pub fn evidence(&self) -> &[EvidenceV1] {
        &self.evidence
    }

    pub fn assertions(&self) -> &[HumanModelAssertionV1] {
        &self.assertions
    }

    pub(crate) fn next_id(&mut self, prefix: &str) -> String {
        self.next_sequence += 1;
        format!("{prefix}-{}-{}", epoch_millis(), self.next_sequence)
    }

    #[allow(clippy::too_many_arguments)]
    fn build_decision(
        &mut self,
        request: &ResolveRequestV1,
        resolved_intent: String,
        intent_confidence: f64,
        base_action: ActionV1,
        alternatives: Vec<ActionV1>,
        additional_constraints: Vec<String>,
        ambiguities: Vec<String>,
        model_usage: ModelUsageV1,
        clarification_required: bool,
    ) -> DecisionContractV1 {
        let as_of = request
            .context
            .data
            .get("as_of")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(now_string);
        let mut assertions = self.applicable_assertions(
            &request.subject_ref,
            &request.context.scope,
            &as_of,
        );
        assertions.extend(
            request
                .human_model_snapshot
                .iter()
                .filter(|assertion| {
                    assertion_applies(assertion, &request.context.scope, &as_of)
                })
                .cloned(),
        );
        assertions.sort_by(assertion_priority);

        let learned_action = assertions.iter().find_map(|assertion| {
            if assertion.kind != AssertionKindV1::Preference
                && assertion.kind != AssertionKindV1::Workflow
                && assertion.kind != AssertionKindV1::DecisionPattern
            {
                return None;
            }
            if assertion.predicate != "preferred_action" {
                return None;
            }
            let action = match serde_json::from_value::<ActionV1>(assertion.value.clone()) {
                Ok(action) => action,
                Err(_) => ActionV1::new(assertion.value.as_str()?),
            };
            if request.host_capabilities.supported_actions.is_empty()
                || request
                    .host_capabilities
                    .supported_actions
                    .contains(&action.action)
            {
                Some((action, assertion))
            } else {
                None
            }
        });

        let (recommended_action, human_model_basis, human_model_confidence) =
            if let Some((action, assertion)) = learned_action {
                (action, vec![assertion.assertion_id.clone()], assertion.confidence)
            } else {
                (base_action, Vec::new(), intent_confidence)
            };

        let mut constraints = request.context.current_constraints.clone();
        append_unique(&mut constraints, additional_constraints);
        let mut evidence_basis = request
            .evidence
            .iter()
            .map(|evidence| evidence.evidence_id.clone())
            .collect::<Vec<_>>();
        for candidate in &request.intent_candidates {
            append_unique(&mut evidence_basis, candidate.evidence_refs.clone());
        }

        let mut reason_codes = Vec::new();
        match model_usage {
            ModelUsageV1::NotRequired => reason_codes.push("intent_resolved_without_model".into()),
            ModelUsageV1::HostSupplied => reason_codes.push("host_supplied_intent".into()),
            ModelUsageV1::HostDelegated => reason_codes.push("host_model_result_validated".into()),
        }
        if !human_model_basis.is_empty() {
            reason_codes.push("human_model_preference_applied".into());
        }
        if !request.context.current_constraints.is_empty() {
            reason_codes.push("current_constraints_prioritized".into());
        }
        if !ambiguities.is_empty() {
            reason_codes.push("ambiguity_recorded".into());
        }

        let decision_id = self.next_id("decision");
        let decision = DecisionContractV1 {
            schema_version: SCHEMA_VERSION_V1.to_owned(),
            decision_id: decision_id.clone(),
            request_id: request.request_id.clone(),
            resolved_intent,
            intent_confidence,
            recommended_action,
            alternatives,
            constraints,
            ambiguities,
            human_model_basis: human_model_basis.clone(),
            evidence_basis: evidence_basis.clone(),
            reason_codes,
            decision_confidence: ((intent_confidence + human_model_confidence) / 2.0)
                .clamp(0.0, 1.0),
            model_usage: model_usage.clone(),
        };

        self.decisions.insert(
            decision_id.clone(),
            StoredDecision {
                subject_ref: request.subject_ref.clone(),
                decision: decision.clone(),
            },
        );
        self.evaluations.push(EvaluationRecordV1 {
            decision_id,
            intent_corrected: false,
            decision_overridden: false,
            clarification_required,
            model_inference_used: model_usage == ModelUsageV1::HostDelegated,
            human_model_assertion_refs: human_model_basis,
            evidence_refs: evidence_basis,
            outcome_status: EvaluationOutcomeStatusV1::Pending,
            resolved_at: as_of,
        });
        decision
    }
}

fn assertion_priority(
    left: &HumanModelAssertionV1,
    right: &HumanModelAssertionV1,
) -> std::cmp::Ordering {
    source_rank(&right.source_type)
        .cmp(&source_rank(&left.source_type))
        .then_with(|| right.confidence.total_cmp(&left.confidence))
}

fn source_rank(source: &AssertionSourceTypeV1) -> u8 {
    match source {
        AssertionSourceTypeV1::Explicit => 2,
        AssertionSourceTypeV1::Inferred => 1,
    }
}

fn default_action(request: &ResolveRequestV1, intent: &str) -> ActionV1 {
    if request.host_capabilities.supported_actions.is_empty()
        || request
            .host_capabilities
            .supported_actions
            .iter()
            .any(|action| action == intent)
    {
        ActionV1::new(intent)
    } else {
        ActionV1::new(request.host_capabilities.supported_actions[0].clone())
    }
}

fn resolve_intent(request: &ResolveRequestV1) -> IntentResolution<'_> {
    let mut candidates = request.intent_candidates.iter().collect::<Vec<_>>();
    candidates.sort_by(|left, right| right.confidence.total_cmp(&left.confidence));
    let Some(top) = candidates.first().copied() else {
        return IntentResolution::NeedsModel(HostModelPurposeV1::IntentResolution);
    };
    if top.confidence < MIN_INTENT_CONFIDENCE {
        return IntentResolution::NeedsModel(HostModelPurposeV1::IntentResolution);
    }
    if let Some(second) = candidates.get(1) {
        if (top.confidence - second.confidence).abs() <= AMBIGUITY_DELTA {
            return IntentResolution::NeedsModel(HostModelPurposeV1::IntentDisambiguation);
        }
    }
    IntentResolution::Resolved(top)
}

fn host_model_request(
    request: &ResolveRequestV1,
    purpose: HostModelPurposeV1,
) -> HostModelRequestV1 {
    HostModelRequestV1 {
        request_id: request.request_id.clone(),
        purpose,
        structured_context: json!({
            "subject_ref": request.subject_ref,
            "input": request.input,
            "intent_candidates": request.intent_candidates,
            "context": request.context,
            "human_model_refs": request.human_model_refs,
            "human_model_snapshot": request.human_model_snapshot,
            "evidence": request.evidence,
            "host_capabilities": request.host_capabilities,
        }),
        expected_response_schema: json!({
            "type": "object",
            "required": [
                "request_id", "resolved_intent", "recommended_action",
                "alternatives", "constraints", "ambiguities", "confidence"
            ],
            "additionalProperties": false
        }),
    }
}

fn validate_resolve_request(request: &ResolveRequestV1) -> Result<(), IdrError> {
    if request.schema_version != SCHEMA_VERSION_V1 {
        return Err(IdrError::InvalidContract("unsupported schema_version".into()));
    }
    if request.request_id.trim().is_empty() || request.subject_ref.trim().is_empty() {
        return Err(IdrError::InvalidContract(
            "request_id and subject_ref are required".into(),
        ));
    }
    for candidate in &request.intent_candidates {
        if candidate.intent.trim().is_empty()
            || !candidate.confidence.is_finite()
            || !(0.0..=1.0).contains(&candidate.confidence)
        {
            return Err(IdrError::InvalidContract(
                "intent candidate is invalid".into(),
            ));
        }
    }
    Ok(())
}

fn validate_model_result(
    request_id: &str,
    result: &HostModelResultV1,
) -> Result<(), IdrError> {
    if result.request_id != request_id {
        return Err(IdrError::InvalidContract(
            "host model result request_id mismatch".into(),
        ));
    }
    if result.resolved_intent.trim().is_empty()
        || result.recommended_action.action.trim().is_empty()
        || !result.confidence.is_finite()
        || !(0.0..=1.0).contains(&result.confidence)
    {
        return Err(IdrError::InvalidContract(
            "host model result is invalid".into(),
        ));
    }
    Ok(())
}

fn append_unique<T: PartialEq>(target: &mut Vec<T>, values: Vec<T>) {
    for value in values {
        if !target.contains(&value) {
            target.push(value);
        }
    }
}

pub(crate) fn now_string() -> String {
    format!("unix-ms:{}", epoch_millis())
}

fn epoch_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
