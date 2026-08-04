use super::{
    canonical_references, contract_digest, unique_strings, validate_required_text,
    HumanCenteredContractKindV1, HumanCenteredContractMetadataV1, HumanCenteredContractRefV1,
    HumanCenteredProtocolError, HumanModelAssertionIdV1, ImpactLevelV1, UnitIntervalBasisPointsV1,
    HUMAN_CENTERED_RULE_VERSION,
};
use crate::ProductionReferenceV1;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HumanModelAssertionTypeV1 {
    InteractionProfile,
    CognitiveInteractionProfile,
    PersonalDecisionModel,
    Goal,
    Constraint,
    DecisionHistory,
    OutcomeHistory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "data")]
pub enum HumanModelValueV1 {
    Boolean(bool),
    Integer(i64),
    BasisPoints(UnitIntervalBasisPointsV1),
    Text(String),
    Enum(String),
    Reference(ProductionReferenceV1),
}

impl HumanModelValueV1 {
    fn validate(&self) -> Result<(), HumanCenteredProtocolError> {
        match self {
            Self::Text(value) | Self::Enum(value) => validate_required_text(value),
            Self::Reference(value) if value.as_str().trim().is_empty() => {
                Err(HumanCenteredProtocolError::InvalidReference)
            }
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HumanModelScopeV1 {
    pub domains: Vec<String>,
    pub task_types: Vec<String>,
    pub global: bool,
}

impl HumanModelScopeV1 {
    fn validate(&self) -> Result<(), HumanCenteredProtocolError> {
        if self.domains.len() > 64
            || self.task_types.len() > 64
            || !unique_strings(&self.domains)
            || !unique_strings(&self.task_types)
            || (self.global && (!self.domains.is_empty() || !self.task_types.is_empty()))
            || (!self.global && self.domains.is_empty() && self.task_types.is_empty())
        {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }
        for value in self.domains.iter().chain(self.task_types.iter()) {
            validate_required_text(value)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HumanModelPersistenceScopeV1 {
    Turn,
    Session,
    LongTerm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HumanModelEpistemicStatusV1 {
    Candidate,
    Provisional,
    UserConfirmed,
    OutcomeSupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HumanModelLifecycleStatusV1 {
    Active,
    Rejected,
    Expired,
    Superseded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HumanModelSourceTypeV1 {
    ExplicitUserStatement,
    InferredFromInteraction,
    DecisionObservation,
    OutcomeObservation,
    UserCorrection,
    ImportedAuthorizedSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HumanModelCorrectionStateV1 {
    NotReviewed,
    Confirmed,
    Corrected,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HumanModelUsagePolicyV1 {
    pub allowed_uses: Vec<String>,
    pub forbidden_uses: Vec<String>,
    pub maximum_impact_level: ImpactLevelV1,
}

impl HumanModelUsagePolicyV1 {
    fn validate(&self) -> Result<(), HumanCenteredProtocolError> {
        if self.allowed_uses.len() > 64
            || self.forbidden_uses.len() > 64
            || self.allowed_uses.is_empty()
            || !unique_strings(&self.allowed_uses)
            || !unique_strings(&self.forbidden_uses)
            || self.maximum_impact_level.rank() >= ImpactLevelV1::High.rank()
            || self
                .allowed_uses
                .iter()
                .any(|value| self.forbidden_uses.contains(value))
        {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }
        for value in self.allowed_uses.iter().chain(self.forbidden_uses.iter()) {
            validate_required_text(value)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HumanModelAssertionV1 {
    metadata: HumanCenteredContractMetadataV1,
    assertion_id: HumanModelAssertionIdV1,
    subject_ref: ProductionReferenceV1,
    assertion_type: HumanModelAssertionTypeV1,
    predicate: String,
    value: HumanModelValueV1,
    claim_summary: String,
    scope: HumanModelScopeV1,
    persistence_scope: HumanModelPersistenceScopeV1,
    epistemic_status: HumanModelEpistemicStatusV1,
    lifecycle_status: HumanModelLifecycleStatusV1,
    confidence: UnitIntervalBasisPointsV1,
    source_type: HumanModelSourceTypeV1,
    source_refs: Vec<ProductionReferenceV1>,
    supporting_evidence_refs: Vec<ProductionReferenceV1>,
    contradicting_evidence_refs: Vec<ProductionReferenceV1>,
    correction_state: HumanModelCorrectionStateV1,
    corrected_value: Option<HumanModelValueV1>,
    first_observed_at: u64,
    last_observed_at: u64,
    expires_at: Option<u64>,
    decay_policy_ref: ProductionReferenceV1,
    usage_policy: HumanModelUsagePolicyV1,
    record_digest: String,
}

impl HumanModelAssertionV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn issue(
        metadata: HumanCenteredContractMetadataV1,
        subject_ref: ProductionReferenceV1,
        assertion_type: HumanModelAssertionTypeV1,
        predicate: impl Into<String>,
        value: HumanModelValueV1,
        claim_summary: impl Into<String>,
        scope: HumanModelScopeV1,
        persistence_scope: HumanModelPersistenceScopeV1,
        gate_decision: &HumanModelUpdateGateDecisionV1,
        lifecycle_status: HumanModelLifecycleStatusV1,
        confidence: UnitIntervalBasisPointsV1,
        source_type: HumanModelSourceTypeV1,
        source_refs: Vec<ProductionReferenceV1>,
        supporting_evidence_refs: Vec<ProductionReferenceV1>,
        contradicting_evidence_refs: Vec<ProductionReferenceV1>,
        correction_state: HumanModelCorrectionStateV1,
        corrected_value: Option<HumanModelValueV1>,
        first_observed_at: u64,
        last_observed_at: u64,
        expires_at: Option<u64>,
        decay_policy_ref: ProductionReferenceV1,
        usage_policy: HumanModelUsagePolicyV1,
    ) -> Result<Self, HumanCenteredProtocolError> {
        let epistemic_status = gate_decision.epistemic_status()?;
        let mut value = Self {
            metadata,
            assertion_id: HumanModelAssertionIdV1::mint(),
            subject_ref,
            assertion_type,
            predicate: predicate.into(),
            value,
            claim_summary: claim_summary.into(),
            scope,
            persistence_scope,
            epistemic_status,
            lifecycle_status,
            confidence,
            source_type,
            source_refs: canonical_references(source_refs)?,
            supporting_evidence_refs: canonical_references(supporting_evidence_refs)?,
            contradicting_evidence_refs: canonical_references(contradicting_evidence_refs)?,
            correction_state,
            corrected_value,
            first_observed_at,
            last_observed_at,
            expires_at,
            decay_policy_ref,
            usage_policy,
            record_digest: String::new(),
        };
        value.record_digest = {
            let input = value.digest_input();
            contract_digest("human-model-assertion-v1", &input)?
        };
        value.validate()?;
        Ok(value)
    }

    fn digest_input(&self) -> impl Serialize + '_ {
        (
            (
                &self.metadata,
                self.assertion_id,
                &self.subject_ref,
                self.assertion_type,
                &self.predicate,
                &self.value,
                &self.claim_summary,
                &self.scope,
                self.persistence_scope,
                self.epistemic_status,
                self.lifecycle_status,
            ),
            (
                self.confidence,
                self.source_type,
                &self.source_refs,
                &self.supporting_evidence_refs,
                &self.contradicting_evidence_refs,
                self.correction_state,
                &self.corrected_value,
                self.first_observed_at,
                self.last_observed_at,
                self.expires_at,
                &self.decay_policy_ref,
                &self.usage_policy,
            ),
        )
    }

    pub fn validate(&self) -> Result<(), HumanCenteredProtocolError> {
        self.metadata.validate()?;
        if self.metadata.contract_kind() != HumanCenteredContractKindV1::HumanModelAssertion {
            return Err(HumanCenteredProtocolError::ContractKindMismatch);
        }
        self.value.validate()?;
        self.scope.validate()?;
        self.usage_policy.validate()?;
        validate_required_text(&self.predicate)?;
        validate_required_text(&self.claim_summary)?;
        if self.assertion_id.is_empty()
            || self.subject_ref != *self.metadata.authority_context().subject_ref()
            || self.decay_policy_ref.as_str().trim().is_empty()
            || self.source_refs.is_empty()
            || self.first_observed_at == 0
            || self.last_observed_at < self.first_observed_at
            || self
                .expires_at
                .is_some_and(|expires| expires <= self.last_observed_at)
            || self
                .supporting_evidence_refs
                .iter()
                .any(|value| self.contradicting_evidence_refs.contains(value))
            || (self.correction_state == HumanModelCorrectionStateV1::Corrected)
                != self.corrected_value.is_some()
            || (self.persistence_scope == HumanModelPersistenceScopeV1::LongTerm
                && self.epistemic_status == HumanModelEpistemicStatusV1::Candidate)
        {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }
        if let Some(value) = &self.corrected_value {
            value.validate()?;
        }
        if contract_digest("human-model-assertion-v1", &self.digest_input())? != self.record_digest
        {
            return Err(HumanCenteredProtocolError::DigestMismatch);
        }
        Ok(())
    }

    pub fn record_ref(&self) -> Result<HumanCenteredContractRefV1, HumanCenteredProtocolError> {
        self.validate()?;
        HumanCenteredContractRefV1::new(
            HumanCenteredContractKindV1::HumanModelAssertion,
            self.metadata.contract_id(),
            self.metadata.revision(),
            self.record_digest.clone(),
        )
    }

    pub fn metadata(&self) -> &HumanCenteredContractMetadataV1 {
        &self.metadata
    }

    /// The only value consumers should use. Rejected, expired, superseded or
    /// correction-rejected assertions never produce an effective model value.
    pub fn effective_value(&self, trusted_now: u64) -> Option<&HumanModelValueV1> {
        if self.lifecycle_status != HumanModelLifecycleStatusV1::Active
            || self.correction_state == HumanModelCorrectionStateV1::Rejected
            || trusted_now < self.first_observed_at
            || self
                .expires_at
                .is_some_and(|expires_at| trusted_now >= expires_at)
        {
            return None;
        }
        Some(self.corrected_value.as_ref().unwrap_or(&self.value))
    }

    pub fn lifecycle_status(&self) -> HumanModelLifecycleStatusV1 {
        self.lifecycle_status
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HumanModelUpdateFactsV1 {
    pub proposed_by_cognitive_service: bool,
    pub explicit_user_confirmation: bool,
    pub independent_supporting_observations: u16,
    pub outcome_support_present: bool,
    pub contradiction_present: bool,
    pub inference_policy_allows: bool,
    pub sensitive_attribute_inference: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HumanModelUpdateGateOutcomeV1 {
    KeepSession,
    StoreCandidate,
    PromoteProvisional,
    PromoteUserConfirmed,
    PromoteOutcomeSupported,
    Reject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HumanModelUpdateGateDecisionV1 {
    outcome: HumanModelUpdateGateOutcomeV1,
    reason_codes: Vec<String>,
    rule_version: u64,
}

impl HumanModelUpdateGateDecisionV1 {
    pub fn outcome(&self) -> HumanModelUpdateGateOutcomeV1 {
        self.outcome
    }

    pub fn reason_codes(&self) -> &[String] {
        &self.reason_codes
    }

    pub fn rule_version(&self) -> u64 {
        self.rule_version
    }

    fn epistemic_status(&self) -> Result<HumanModelEpistemicStatusV1, HumanCenteredProtocolError> {
        if self.rule_version != HUMAN_CENTERED_RULE_VERSION {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }
        match self.outcome {
            HumanModelUpdateGateOutcomeV1::StoreCandidate => {
                Ok(HumanModelEpistemicStatusV1::Candidate)
            }
            HumanModelUpdateGateOutcomeV1::PromoteProvisional => {
                Ok(HumanModelEpistemicStatusV1::Provisional)
            }
            HumanModelUpdateGateOutcomeV1::PromoteUserConfirmed => {
                Ok(HumanModelEpistemicStatusV1::UserConfirmed)
            }
            HumanModelUpdateGateOutcomeV1::PromoteOutcomeSupported => {
                Ok(HumanModelEpistemicStatusV1::OutcomeSupported)
            }
            HumanModelUpdateGateOutcomeV1::KeepSession | HumanModelUpdateGateOutcomeV1::Reject => {
                Err(HumanCenteredProtocolError::InvalidContract)
            }
        }
    }
}

pub fn evaluate_human_model_update_gate(
    facts: &HumanModelUpdateFactsV1,
) -> HumanModelUpdateGateDecisionV1 {
    if !facts.inference_policy_allows
        || facts.sensitive_attribute_inference
        || facts.contradiction_present
    {
        let mut reasons = Vec::new();
        if !facts.inference_policy_allows {
            reasons.push("inference_policy_denied".to_string());
        }
        if facts.sensitive_attribute_inference {
            reasons.push("sensitive_attribute_inference".to_string());
        }
        if facts.contradiction_present {
            reasons.push("contradiction_requires_review".to_string());
        }
        return HumanModelUpdateGateDecisionV1 {
            outcome: HumanModelUpdateGateOutcomeV1::Reject,
            reason_codes: reasons,
            rule_version: HUMAN_CENTERED_RULE_VERSION,
        };
    }
    if facts.explicit_user_confirmation {
        return HumanModelUpdateGateDecisionV1 {
            outcome: HumanModelUpdateGateOutcomeV1::PromoteUserConfirmed,
            reason_codes: vec!["explicit_user_confirmation".to_string()],
            rule_version: HUMAN_CENTERED_RULE_VERSION,
        };
    }
    if facts.outcome_support_present && facts.independent_supporting_observations >= 1 {
        return HumanModelUpdateGateDecisionV1 {
            outcome: HumanModelUpdateGateOutcomeV1::PromoteOutcomeSupported,
            reason_codes: vec!["outcome_supported".to_string()],
            rule_version: HUMAN_CENTERED_RULE_VERSION,
        };
    }
    if facts.independent_supporting_observations >= 2 {
        return HumanModelUpdateGateDecisionV1 {
            outcome: HumanModelUpdateGateOutcomeV1::PromoteProvisional,
            reason_codes: vec!["multiple_independent_observations".to_string()],
            rule_version: HUMAN_CENTERED_RULE_VERSION,
        };
    }
    if facts.proposed_by_cognitive_service {
        return HumanModelUpdateGateDecisionV1 {
            outcome: HumanModelUpdateGateOutcomeV1::StoreCandidate,
            reason_codes: vec!["cognitive_candidate_only".to_string()],
            rule_version: HUMAN_CENTERED_RULE_VERSION,
        };
    }
    HumanModelUpdateGateDecisionV1 {
        outcome: HumanModelUpdateGateOutcomeV1::KeepSession,
        reason_codes: vec!["insufficient_long_term_evidence".to_string()],
        rule_version: HUMAN_CENTERED_RULE_VERSION,
    }
}
