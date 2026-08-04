use super::{
    canonical_references, contract_digest, validate_required_text, HumanCenteredContractKindV1,
    HumanCenteredContractMetadataV1, HumanCenteredContractRefV1, HumanCenteredProtocolError,
    UnitIntervalBasisPointsV1,
};
use crate::ProductionReferenceV1;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentResolutionMethodV1 {
    DeterministicFastPath,
    FullPath,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentHypothesisV1 {
    hypothesis_ref: ProductionReferenceV1,
    intent_name: String,
    confidence: UnitIntervalBasisPointsV1,
    supporting_evidence_refs: Vec<ProductionReferenceV1>,
    contradicting_evidence_refs: Vec<ProductionReferenceV1>,
}

impl IntentHypothesisV1 {
    pub fn new(
        hypothesis_ref: ProductionReferenceV1,
        intent_name: impl Into<String>,
        confidence: UnitIntervalBasisPointsV1,
        supporting_evidence_refs: Vec<ProductionReferenceV1>,
        contradicting_evidence_refs: Vec<ProductionReferenceV1>,
    ) -> Result<Self, HumanCenteredProtocolError> {
        let value = Self {
            hypothesis_ref,
            intent_name: intent_name.into(),
            confidence,
            supporting_evidence_refs: canonical_references(supporting_evidence_refs)?,
            contradicting_evidence_refs: canonical_references(contradicting_evidence_refs)?,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), HumanCenteredProtocolError> {
        validate_required_text(&self.intent_name)?;
        if self.hypothesis_ref.as_str().trim().is_empty()
            || self
                .supporting_evidence_refs
                .iter()
                .any(|reference| self.contradicting_evidence_refs.contains(reference))
        {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }
        Ok(())
    }

    pub fn hypothesis_ref(&self) -> &ProductionReferenceV1 {
        &self.hypothesis_ref
    }

    pub fn intent_name(&self) -> &str {
        &self.intent_name
    }

    pub fn confidence(&self) -> UnitIntervalBasisPointsV1 {
        self.confidence
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RankedIntentHypothesisV1 {
    hypothesis: IntentHypothesisV1,
    rank: u16,
}

impl RankedIntentHypothesisV1 {
    pub fn new(
        hypothesis: IntentHypothesisV1,
        rank: u16,
    ) -> Result<Self, HumanCenteredProtocolError> {
        hypothesis.validate()?;
        if rank == 0 {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }
        Ok(Self { hypothesis, rank })
    }

    pub fn hypothesis(&self) -> &IntentHypothesisV1 {
        &self.hypothesis
    }

    pub fn rank(&self) -> u16 {
        self.rank
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HumanModelEffectKindV1 {
    ReorderedBaseHypothesis,
    AddedPersonalizedHypothesis,
    ExposedModelExpressionConflict,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HumanModelIntentEffectV1 {
    assertion_ref: HumanCenteredContractRefV1,
    effect_kind: HumanModelEffectKindV1,
    affected_hypothesis_ref: ProductionReferenceV1,
    base_rank: Option<u16>,
    personalized_rank: u16,
    explanation_ref: ProductionReferenceV1,
}

impl HumanModelIntentEffectV1 {
    pub fn new(
        assertion_ref: HumanCenteredContractRefV1,
        effect_kind: HumanModelEffectKindV1,
        affected_hypothesis_ref: ProductionReferenceV1,
        base_rank: Option<u16>,
        personalized_rank: u16,
        explanation_ref: ProductionReferenceV1,
    ) -> Result<Self, HumanCenteredProtocolError> {
        assertion_ref.validate()?;
        if assertion_ref.kind() != HumanCenteredContractKindV1::HumanModelAssertion
            || personalized_rank == 0
            || base_rank == Some(0)
        {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }
        Ok(Self {
            assertion_ref,
            effect_kind,
            affected_hypothesis_ref,
            base_rank,
            personalized_rank,
            explanation_ref,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentContractV1 {
    metadata: HumanCenteredContractMetadataV1,
    user_stated_intent_ref: ProductionReferenceV1,
    base_hypotheses: Vec<RankedIntentHypothesisV1>,
    personalized_hypotheses: Vec<RankedIntentHypothesisV1>,
    primary_hypothesis_ref: ProductionReferenceV1,
    confidence: UnitIntervalBasisPointsV1,
    ambiguity: bool,
    clarification_required: bool,
    human_model_effects: Vec<HumanModelIntentEffectV1>,
    resolution_method: IntentResolutionMethodV1,
    record_digest: String,
}

impl IntentContractV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn issue(
        metadata: HumanCenteredContractMetadataV1,
        user_stated_intent_ref: ProductionReferenceV1,
        base_hypotheses: Vec<RankedIntentHypothesisV1>,
        personalized_hypotheses: Vec<RankedIntentHypothesisV1>,
        primary_hypothesis_ref: ProductionReferenceV1,
        confidence: UnitIntervalBasisPointsV1,
        ambiguity: bool,
        clarification_required: bool,
        human_model_effects: Vec<HumanModelIntentEffectV1>,
        resolution_method: IntentResolutionMethodV1,
    ) -> Result<Self, HumanCenteredProtocolError> {
        let mut value = Self {
            metadata,
            user_stated_intent_ref,
            base_hypotheses,
            personalized_hypotheses,
            primary_hypothesis_ref,
            confidence,
            ambiguity,
            clarification_required,
            human_model_effects,
            resolution_method,
            record_digest: String::new(),
        };
        value.record_digest = {
            let input = value.digest_input();
            contract_digest("intent-contract-v1", &input)?
        };
        value.validate()?;
        Ok(value)
    }

    fn digest_input(&self) -> impl Serialize + '_ {
        (
            &self.metadata,
            &self.user_stated_intent_ref,
            &self.base_hypotheses,
            &self.personalized_hypotheses,
            &self.primary_hypothesis_ref,
            self.confidence,
            self.ambiguity,
            self.clarification_required,
            &self.human_model_effects,
            self.resolution_method,
        )
    }

    pub fn validate(&self) -> Result<(), HumanCenteredProtocolError> {
        self.metadata.validate()?;
        if self.metadata.contract_kind() != HumanCenteredContractKindV1::Intent {
            return Err(HumanCenteredProtocolError::ContractKindMismatch);
        }
        validate_ranked_hypotheses(&self.base_hypotheses)?;
        validate_ranked_hypotheses(&self.personalized_hypotheses)?;
        if self.user_stated_intent_ref.as_str().trim().is_empty()
            || self.base_hypotheses.is_empty()
            || self.personalized_hypotheses.is_empty()
        {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }

        let base_refs: BTreeSet<_> = self
            .base_hypotheses
            .iter()
            .map(|value| value.hypothesis().hypothesis_ref().as_str())
            .collect();
        let personalized_refs: BTreeSet<_> = self
            .personalized_hypotheses
            .iter()
            .map(|value| value.hypothesis().hypothesis_ref().as_str())
            .collect();
        if !base_refs.is_subset(&personalized_refs)
            || !personalized_refs.contains(self.primary_hypothesis_ref.as_str())
            || (self.resolution_method == IntentResolutionMethodV1::DeterministicFastPath
                && (!self.human_model_effects.is_empty() || base_refs != personalized_refs))
            || self.human_model_effects.iter().any(|effect| {
                !self
                    .metadata
                    .dependency_refs()
                    .contains(&effect.assertion_ref)
            })
        {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }

        let expected = contract_digest("intent-contract-v1", &self.digest_input())?;
        if expected != self.record_digest {
            return Err(HumanCenteredProtocolError::DigestMismatch);
        }
        Ok(())
    }

    pub fn record_ref(&self) -> Result<HumanCenteredContractRefV1, HumanCenteredProtocolError> {
        self.validate()?;
        HumanCenteredContractRefV1::new(
            HumanCenteredContractKindV1::Intent,
            self.metadata.contract_id(),
            self.metadata.revision(),
            self.record_digest.clone(),
        )
    }

    pub fn base_hypotheses(&self) -> &[RankedIntentHypothesisV1] {
        &self.base_hypotheses
    }

    pub fn metadata(&self) -> &HumanCenteredContractMetadataV1 {
        &self.metadata
    }

    pub fn personalized_hypotheses(&self) -> &[RankedIntentHypothesisV1] {
        &self.personalized_hypotheses
    }

    pub fn primary_hypothesis_ref(&self) -> &ProductionReferenceV1 {
        &self.primary_hypothesis_ref
    }
}

fn validate_ranked_hypotheses(
    values: &[RankedIntentHypothesisV1],
) -> Result<(), HumanCenteredProtocolError> {
    if values.len() > 256 {
        return Err(HumanCenteredProtocolError::CollectionLimitExceeded);
    }
    let mut ranks = BTreeSet::new();
    let mut references = BTreeMap::new();
    for value in values {
        value.hypothesis.validate()?;
        if value.rank == 0
            || !ranks.insert(value.rank)
            || references
                .insert(value.hypothesis.hypothesis_ref.as_str(), value.rank)
                .is_some()
        {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }
    }
    if ranks
        .iter()
        .copied()
        .ne((1..=values.len()).map(|value| value as u16))
    {
        return Err(HumanCenteredProtocolError::InvalidContract);
    }
    Ok(())
}
