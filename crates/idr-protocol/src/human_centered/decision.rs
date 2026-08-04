use super::{
    canonical_references, contract_digest, validate_required_text, validate_text_collection,
    HumanCenteredContractKindV1, HumanCenteredContractMetadataV1, HumanCenteredContractRefV1,
    HumanCenteredProtocolError, ImpactLevelV1, UnitIntervalBasisPointsV1,
};
use crate::ProductionReferenceV1;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionStageV1 {
    Framing,
    GatheringEvidence,
    GeneratingOptions,
    ComparingOptions,
    AwaitingUserInput,
    RecommendationReady,
    UserDecided,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CriterionImportanceV1 {
    Required,
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionCriterionV1 {
    pub name: String,
    pub importance: CriterionImportanceV1,
    pub weight: Option<UnitIntervalBasisPointsV1>,
    pub evidence_refs: Vec<ProductionReferenceV1>,
}

impl DecisionCriterionV1 {
    pub fn new(
        name: impl Into<String>,
        importance: CriterionImportanceV1,
        weight: Option<UnitIntervalBasisPointsV1>,
        evidence_refs: Vec<ProductionReferenceV1>,
    ) -> Result<Self, HumanCenteredProtocolError> {
        let value = Self {
            name: name.into(),
            importance,
            weight,
            evidence_refs: canonical_references(evidence_refs)?,
        };
        validate_required_text(&value.name)?;
        Ok(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionOptionV1 {
    pub option_ref: ProductionReferenceV1,
    pub label: String,
    pub description: String,
    pub evidence_refs: Vec<ProductionReferenceV1>,
    pub reversible: bool,
}

impl DecisionOptionV1 {
    pub fn new(
        option_ref: ProductionReferenceV1,
        label: impl Into<String>,
        description: impl Into<String>,
        evidence_refs: Vec<ProductionReferenceV1>,
        reversible: bool,
    ) -> Result<Self, HumanCenteredProtocolError> {
        let value = Self {
            option_ref,
            label: label.into(),
            description: description.into(),
            evidence_refs: canonical_references(evidence_refs)?,
            reversible,
        };
        validate_required_text(&value.label)?;
        validate_required_text(&value.description)?;
        Ok(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionRiskV1 {
    pub summary: String,
    pub impact: ImpactLevelV1,
    pub likelihood: UnitIntervalBasisPointsV1,
    pub evidence_refs: Vec<ProductionReferenceV1>,
    pub mitigation: Option<String>,
}

impl DecisionRiskV1 {
    pub fn validate(&self) -> Result<(), HumanCenteredProtocolError> {
        validate_required_text(&self.summary)?;
        if let Some(mitigation) = &self.mitigation {
            validate_required_text(mitigation)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionRecommendationV1 {
    pub option_ref: ProductionReferenceV1,
    pub confidence: UnitIntervalBasisPointsV1,
    pub conditions: Vec<String>,
    pub evidence_refs: Vec<ProductionReferenceV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionAuthorizationBoundaryV1 {
    pub maximum_automatic_impact: ImpactLevelV1,
    pub user_confirmation_required: bool,
    pub required_authority_refs: Vec<ProductionReferenceV1>,
    pub prohibited_automatic_operations: Vec<ProductionReferenceV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionContractV1 {
    metadata: HumanCenteredContractMetadataV1,
    intent_ref: HumanCenteredContractRefV1,
    subject: String,
    stage: DecisionStageV1,
    objective: String,
    constraints: Vec<String>,
    criteria: Vec<DecisionCriterionV1>,
    options: Vec<DecisionOptionV1>,
    evidence_refs: Vec<ProductionReferenceV1>,
    conflicts: Vec<String>,
    risks: Vec<DecisionRiskV1>,
    uncertainty: Vec<String>,
    recommendation: Option<DecisionRecommendationV1>,
    selected_option_ref: Option<ProductionReferenceV1>,
    required_user_inputs: Vec<String>,
    authorization_boundary: DecisionAuthorizationBoundaryV1,
    record_digest: String,
}

impl DecisionContractV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn issue(
        metadata: HumanCenteredContractMetadataV1,
        intent_ref: HumanCenteredContractRefV1,
        subject: impl Into<String>,
        stage: DecisionStageV1,
        objective: impl Into<String>,
        constraints: Vec<String>,
        criteria: Vec<DecisionCriterionV1>,
        options: Vec<DecisionOptionV1>,
        evidence_refs: Vec<ProductionReferenceV1>,
        conflicts: Vec<String>,
        risks: Vec<DecisionRiskV1>,
        uncertainty: Vec<String>,
        recommendation: Option<DecisionRecommendationV1>,
        selected_option_ref: Option<ProductionReferenceV1>,
        required_user_inputs: Vec<String>,
        authorization_boundary: DecisionAuthorizationBoundaryV1,
    ) -> Result<Self, HumanCenteredProtocolError> {
        let mut value = Self {
            metadata,
            intent_ref,
            subject: subject.into(),
            stage,
            objective: objective.into(),
            constraints,
            criteria,
            options,
            evidence_refs: canonical_references(evidence_refs)?,
            conflicts,
            risks,
            uncertainty,
            recommendation,
            selected_option_ref,
            required_user_inputs,
            authorization_boundary,
            record_digest: String::new(),
        };
        value.record_digest = {
            let input = value.digest_input();
            contract_digest("decision-contract-v1", &input)?
        };
        value.validate()?;
        Ok(value)
    }

    fn digest_input(&self) -> impl Serialize + '_ {
        (
            (
                &self.metadata,
                &self.intent_ref,
                &self.subject,
                self.stage,
                &self.objective,
                &self.constraints,
                &self.criteria,
                &self.options,
            ),
            (
                &self.evidence_refs,
                &self.conflicts,
                &self.risks,
                &self.uncertainty,
                &self.recommendation,
                &self.selected_option_ref,
                &self.required_user_inputs,
                &self.authorization_boundary,
            ),
        )
    }

    pub fn validate(&self) -> Result<(), HumanCenteredProtocolError> {
        self.metadata.validate()?;
        if self.metadata.contract_kind() != HumanCenteredContractKindV1::Decision {
            return Err(HumanCenteredProtocolError::ContractKindMismatch);
        }
        self.intent_ref.validate()?;
        validate_required_text(&self.subject)?;
        validate_required_text(&self.objective)?;
        validate_text_collection(&self.constraints)?;
        validate_text_collection(&self.conflicts)?;
        validate_text_collection(&self.uncertainty)?;
        validate_text_collection(&self.required_user_inputs)?;
        if self.criteria.len() > 256 || self.options.len() > 256 || self.risks.len() > 256 {
            return Err(HumanCenteredProtocolError::CollectionLimitExceeded);
        }
        if self.intent_ref.kind() != HumanCenteredContractKindV1::Intent
            || !self.metadata.dependency_refs().contains(&self.intent_ref)
            || self
                .criteria
                .iter()
                .any(|criterion| validate_required_text(&criterion.name).is_err())
            || self.risks.iter().any(|risk| risk.validate().is_err())
        {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }

        let option_refs: BTreeSet<_> = self
            .options
            .iter()
            .map(|value| value.option_ref.as_str())
            .collect();
        if option_refs.len() != self.options.len()
            || (self.options.is_empty() && self.required_user_inputs.is_empty())
        {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }
        let weighted_criteria: Vec<_> = self
            .criteria
            .iter()
            .filter_map(|criterion| criterion.weight)
            .collect();
        if !weighted_criteria.is_empty()
            && (weighted_criteria.len() != self.criteria.len()
                || weighted_criteria
                    .iter()
                    .map(|weight| u32::from(weight.value()))
                    .sum::<u32>()
                    != u32::from(UnitIntervalBasisPointsV1::FULL.value()))
        {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }
        for option in &self.options {
            validate_required_text(&option.label)?;
            validate_required_text(&option.description)?;
        }
        if let Some(recommendation) = &self.recommendation {
            validate_text_collection(&recommendation.conditions)?;
            if !option_refs.contains(recommendation.option_ref.as_str()) {
                return Err(HumanCenteredProtocolError::InvalidContract);
            }
        }
        if self
            .selected_option_ref
            .as_ref()
            .is_some_and(|selected| !option_refs.contains(selected.as_str()))
        {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }
        match self.stage {
            DecisionStageV1::AwaitingUserInput if self.required_user_inputs.is_empty() => {
                return Err(HumanCenteredProtocolError::InvalidContract);
            }
            DecisionStageV1::RecommendationReady
                if self.recommendation.is_none() || !self.required_user_inputs.is_empty() =>
            {
                return Err(HumanCenteredProtocolError::InvalidContract);
            }
            DecisionStageV1::UserDecided
                if self.selected_option_ref.is_none() || !self.required_user_inputs.is_empty() =>
            {
                return Err(HumanCenteredProtocolError::InvalidContract);
            }
            stage
                if stage != DecisionStageV1::UserDecided && self.selected_option_ref.is_some() =>
            {
                return Err(HumanCenteredProtocolError::InvalidContract);
            }
            _ => {}
        }
        if self.authorization_boundary.maximum_automatic_impact.rank() >= ImpactLevelV1::High.rank()
        {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }
        if contract_digest("decision-contract-v1", &self.digest_input())? != self.record_digest {
            return Err(HumanCenteredProtocolError::DigestMismatch);
        }
        Ok(())
    }

    pub fn record_ref(&self) -> Result<HumanCenteredContractRefV1, HumanCenteredProtocolError> {
        self.validate()?;
        HumanCenteredContractRefV1::new(
            HumanCenteredContractKindV1::Decision,
            self.metadata.contract_id(),
            self.metadata.revision(),
            self.record_digest.clone(),
        )
    }

    pub fn metadata(&self) -> &HumanCenteredContractMetadataV1 {
        &self.metadata
    }

    pub fn intent_ref(&self) -> &HumanCenteredContractRefV1 {
        &self.intent_ref
    }

    pub fn authorization_boundary(&self) -> &DecisionAuthorizationBoundaryV1 {
        &self.authorization_boundary
    }

    pub fn selected_option_ref(&self) -> Option<&ProductionReferenceV1> {
        self.selected_option_ref.as_ref()
    }
}
