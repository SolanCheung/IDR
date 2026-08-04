use super::{
    canonical_references, contract_digest, valid_digest, validate_text_collection,
    HumanCenteredContractKindV1, HumanCenteredContractMetadataV1, HumanCenteredContractRefV1,
    HumanCenteredProtocolError, HUMAN_CENTERED_RULE_VERSION, HUMAN_CENTERED_SCHEMA_VERSION,
};
use crate::ProductionReferenceV1;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponsePhaseV1 {
    PreAction,
    Progress,
    PostAction,
    Final,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseTypeV1 {
    Answer,
    Clarification,
    Recommendation,
    Challenge,
    ConfirmationRequest,
    ProgressUpdate,
    ExecutionReport,
    RecoveryOffer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExplanationDepthV1 {
    Minimal,
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminologyLevelV1 {
    Plain,
    Professional,
    Expert,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResponseContractV1 {
    metadata: HumanCenteredContractMetadataV1,
    source_contract_refs: Vec<HumanCenteredContractRefV1>,
    response_phase: ResponsePhaseV1,
    response_type: ResponseTypeV1,
    answer_structure: Vec<String>,
    evidence_refs: Vec<ProductionReferenceV1>,
    explanation_depth: ExplanationDepthV1,
    terminology_level: TerminologyLevelV1,
    clarification_questions: Vec<String>,
    uncertainty_disclosures: Vec<String>,
    prohibited_content_refs: Vec<ProductionReferenceV1>,
    requires_post_render_policy_check: bool,
    record_digest: String,
}

impl ResponseContractV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn issue(
        metadata: HumanCenteredContractMetadataV1,
        source_contract_refs: Vec<HumanCenteredContractRefV1>,
        response_phase: ResponsePhaseV1,
        response_type: ResponseTypeV1,
        answer_structure: Vec<String>,
        evidence_refs: Vec<ProductionReferenceV1>,
        explanation_depth: ExplanationDepthV1,
        terminology_level: TerminologyLevelV1,
        clarification_questions: Vec<String>,
        uncertainty_disclosures: Vec<String>,
        prohibited_content_refs: Vec<ProductionReferenceV1>,
    ) -> Result<Self, HumanCenteredProtocolError> {
        let mut value = Self {
            metadata,
            source_contract_refs,
            response_phase,
            response_type,
            answer_structure,
            evidence_refs: canonical_references(evidence_refs)?,
            explanation_depth,
            terminology_level,
            clarification_questions,
            uncertainty_disclosures,
            prohibited_content_refs: canonical_references(prohibited_content_refs)?,
            requires_post_render_policy_check: true,
            record_digest: String::new(),
        };
        value.record_digest = {
            let input = value.digest_input();
            contract_digest("response-contract-v1", &input)?
        };
        value.validate()?;
        Ok(value)
    }

    fn digest_input(&self) -> impl Serialize + '_ {
        (
            &self.metadata,
            &self.source_contract_refs,
            self.response_phase,
            self.response_type,
            &self.answer_structure,
            &self.evidence_refs,
            self.explanation_depth,
            self.terminology_level,
            &self.clarification_questions,
            &self.uncertainty_disclosures,
            &self.prohibited_content_refs,
            self.requires_post_render_policy_check,
        )
    }

    pub fn validate(&self) -> Result<(), HumanCenteredProtocolError> {
        self.metadata.validate()?;
        if self.metadata.contract_kind() != HumanCenteredContractKindV1::Response {
            return Err(HumanCenteredProtocolError::ContractKindMismatch);
        }
        validate_text_collection(&self.answer_structure)?;
        validate_text_collection(&self.clarification_questions)?;
        validate_text_collection(&self.uncertainty_disclosures)?;
        if self.source_contract_refs.is_empty()
            || self.answer_structure.is_empty()
            || !self.requires_post_render_policy_check
        {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }
        for reference in &self.source_contract_refs {
            reference.validate()?;
            if !matches!(
                reference.kind(),
                HumanCenteredContractKindV1::Intent
                    | HumanCenteredContractKindV1::Decision
                    | HumanCenteredContractKindV1::Action
                    | HumanCenteredContractKindV1::ExecutionReceipt
                    | HumanCenteredContractKindV1::Outcome
            ) || !self.metadata.dependency_refs().contains(reference)
            {
                return Err(HumanCenteredProtocolError::InvalidContract);
            }
            if self.response_phase == ResponsePhaseV1::PreAction
                && matches!(
                    reference.kind(),
                    HumanCenteredContractKindV1::ExecutionReceipt
                        | HumanCenteredContractKindV1::Outcome
                )
            {
                return Err(HumanCenteredProtocolError::InvalidContract);
            }
        }
        if self.response_type == ResponseTypeV1::Clarification
            && self.clarification_questions.is_empty()
        {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }
        if contract_digest("response-contract-v1", &self.digest_input())? != self.record_digest {
            return Err(HumanCenteredProtocolError::DigestMismatch);
        }
        Ok(())
    }

    pub fn record_ref(&self) -> Result<HumanCenteredContractRefV1, HumanCenteredProtocolError> {
        self.validate()?;
        HumanCenteredContractRefV1::new(
            HumanCenteredContractKindV1::Response,
            self.metadata.contract_id(),
            self.metadata.revision(),
            self.record_digest.clone(),
        )
    }

    pub fn metadata(&self) -> &HumanCenteredContractMetadataV1 {
        &self.metadata
    }

    pub fn source_contract_refs(&self) -> &[HumanCenteredContractRefV1] {
        &self.source_contract_refs
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenderedResponseEnvelopeV1 {
    response_ref: HumanCenteredContractRefV1,
    content_digest: String,
    renderer_ref: ProductionReferenceV1,
    rendered_at: u64,
    schema_version: u16,
}

impl RenderedResponseEnvelopeV1 {
    pub fn new(
        response: &ResponseContractV1,
        rendered_content: &str,
        renderer_ref: ProductionReferenceV1,
        rendered_at: u64,
    ) -> Result<Self, HumanCenteredProtocolError> {
        if rendered_content.trim().is_empty() || rendered_content.len() > 1_048_576 {
            return Err(HumanCenteredProtocolError::InvalidText);
        }
        let value = Self {
            response_ref: response.record_ref()?,
            content_digest: format!(
                "sha256:{}",
                contract_digest("rendered-response-content-v1", rendered_content)?
            ),
            renderer_ref,
            rendered_at,
            schema_version: HUMAN_CENTERED_SCHEMA_VERSION,
        };
        value.validate_for(response)?;
        Ok(value)
    }

    pub fn validate_for(
        &self,
        response: &ResponseContractV1,
    ) -> Result<(), HumanCenteredProtocolError> {
        response.validate()?;
        if self.response_ref != response.record_ref()?
            || !self
                .content_digest
                .strip_prefix("sha256:")
                .is_some_and(valid_digest)
            || self.renderer_ref.as_str().trim().is_empty()
            || self.rendered_at == 0
            || self.rendered_at >= response.metadata().valid_until()
            || self.schema_version != HUMAN_CENTERED_SCHEMA_VERSION
        {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }
        Ok(())
    }

    pub fn validate_content(
        &self,
        rendered_content: &str,
    ) -> Result<(), HumanCenteredProtocolError> {
        if rendered_content.trim().is_empty()
            || rendered_content.len() > 1_048_576
            || self.content_digest
                != format!(
                    "sha256:{}",
                    contract_digest("rendered-response-content-v1", rendered_content)?
                )
        {
            return Err(HumanCenteredProtocolError::DigestMismatch);
        }
        Ok(())
    }

    pub fn response_ref(&self) -> &HumanCenteredContractRefV1 {
        &self.response_ref
    }

    pub fn content_digest(&self) -> &str {
        &self.content_digest
    }

    pub fn rendered_at(&self) -> u64 {
        self.rendered_at
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResponsePolicyFactsV1 {
    pub policy_evaluation_ref: ProductionReferenceV1,
    pub policy_revision_ref: ProductionReferenceV1,
    pub evaluated_response_ref: HumanCenteredContractRefV1,
    pub evaluated_content_digest: String,
    pub evaluated_at: u64,
    pub expires_at: u64,
    pub content_policy_allows: bool,
    pub evidence_traceable: bool,
    pub required_uncertainty_present: bool,
    pub prohibited_content_absent: bool,
    pub authority_scope_respected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseAdmissionOutcomeV1 {
    Allow,
    Block,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResponseAdmissionDecisionV1 {
    pub outcome: ResponseAdmissionOutcomeV1,
    pub reason_codes: Vec<String>,
    pub policy_evaluation_ref: ProductionReferenceV1,
    pub rule_version: u64,
}

pub fn evaluate_rendered_response_admission(
    response: &ResponseContractV1,
    rendered: &RenderedResponseEnvelopeV1,
    rendered_content: &str,
    facts: &ResponsePolicyFactsV1,
    valid_at: u64,
) -> ResponseAdmissionDecisionV1 {
    let mut reasons = Vec::new();
    if rendered.validate_for(response).is_err()
        || rendered.validate_content(rendered_content).is_err()
    {
        reasons.push("rendered_response_not_bound_to_contract".to_string());
    }
    if facts.policy_evaluation_ref.as_str().trim().is_empty() {
        reasons.push("policy_evaluation_reference_missing".to_string());
    }
    if facts.policy_revision_ref != *response.metadata().policy_revision_ref() {
        reasons.push("policy_revision_mismatch".to_string());
    }
    if &facts.evaluated_response_ref != rendered.response_ref()
        || facts.evaluated_content_digest != rendered.content_digest()
    {
        reasons.push("policy_evaluation_binding_mismatch".to_string());
    }
    if facts.evaluated_at < rendered.rendered_at()
        || facts.expires_at <= facts.evaluated_at
        || facts.evaluated_at == 0
        || facts.expires_at > response.metadata().valid_until()
    {
        reasons.push("policy_evaluation_time_invalid".to_string());
    }
    if valid_at < facts.evaluated_at
        || valid_at >= facts.expires_at
        || valid_at >= response.metadata().valid_until()
    {
        reasons.push("response_send_time_invalid".to_string());
    }
    let checks = [
        ("content_policy_denied", facts.content_policy_allows),
        ("evidence_not_traceable", facts.evidence_traceable),
        (
            "required_uncertainty_missing",
            facts.required_uncertainty_present,
        ),
        (
            "prohibited_content_present",
            facts.prohibited_content_absent,
        ),
        (
            "authority_scope_not_respected",
            facts.authority_scope_respected,
        ),
    ];
    reasons.extend(
        checks
            .into_iter()
            .filter(|(_, passed)| !passed)
            .map(|(reason, _)| reason.to_string()),
    );
    ResponseAdmissionDecisionV1 {
        outcome: if reasons.is_empty() {
            ResponseAdmissionOutcomeV1::Allow
        } else {
            ResponseAdmissionOutcomeV1::Block
        },
        reason_codes: reasons,
        policy_evaluation_ref: facts.policy_evaluation_ref.clone(),
        rule_version: HUMAN_CENTERED_RULE_VERSION,
    }
}
