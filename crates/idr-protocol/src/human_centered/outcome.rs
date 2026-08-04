use super::{
    canonical_references, contract_digest, validate_required_text, validate_text_collection,
    HumanCenteredContractKindV1, HumanCenteredContractMetadataV1, HumanCenteredContractRefV1,
    HumanCenteredProtocolError, UnitIntervalBasisPointsV1,
};
use crate::ProductionReferenceV1;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeStateV1 {
    Observed,
    PartiallyObserved,
    Unknown,
    Conflicted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeAttributionV1 {
    IntentResolution,
    DecisionQuality,
    ExecutionQuality,
    EnvironmentChange,
    Mixed,
    InsufficientEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutcomeRecordV1 {
    metadata: HumanCenteredContractMetadataV1,
    decision_ref: Option<HumanCenteredContractRefV1>,
    action_ref: Option<HumanCenteredContractRefV1>,
    execution_receipt_ref: Option<HumanCenteredContractRefV1>,
    objective: String,
    state: OutcomeStateV1,
    observed_results: Vec<String>,
    success_criteria_satisfied: Vec<String>,
    success_criteria_unsatisfied: Vec<String>,
    attribution: OutcomeAttributionV1,
    attribution_confidence: UnitIntervalBasisPointsV1,
    evidence_refs: Vec<ProductionReferenceV1>,
    follow_up_required: bool,
    record_digest: String,
}

impl OutcomeRecordV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn issue(
        metadata: HumanCenteredContractMetadataV1,
        decision_ref: Option<HumanCenteredContractRefV1>,
        action_ref: Option<HumanCenteredContractRefV1>,
        execution_receipt_ref: Option<HumanCenteredContractRefV1>,
        objective: impl Into<String>,
        state: OutcomeStateV1,
        observed_results: Vec<String>,
        success_criteria_satisfied: Vec<String>,
        success_criteria_unsatisfied: Vec<String>,
        attribution: OutcomeAttributionV1,
        attribution_confidence: UnitIntervalBasisPointsV1,
        evidence_refs: Vec<ProductionReferenceV1>,
        follow_up_required: bool,
    ) -> Result<Self, HumanCenteredProtocolError> {
        let mut value = Self {
            metadata,
            decision_ref,
            action_ref,
            execution_receipt_ref,
            objective: objective.into(),
            state,
            observed_results,
            success_criteria_satisfied,
            success_criteria_unsatisfied,
            attribution,
            attribution_confidence,
            evidence_refs: canonical_references(evidence_refs)?,
            follow_up_required,
            record_digest: String::new(),
        };
        value.record_digest = {
            let input = value.digest_input();
            contract_digest("outcome-record-v1", &input)?
        };
        value.validate()?;
        Ok(value)
    }

    fn digest_input(&self) -> impl Serialize + '_ {
        (
            &self.metadata,
            &self.decision_ref,
            &self.action_ref,
            &self.execution_receipt_ref,
            &self.objective,
            self.state,
            &self.observed_results,
            &self.success_criteria_satisfied,
            &self.success_criteria_unsatisfied,
            self.attribution,
            self.attribution_confidence,
            &self.evidence_refs,
            self.follow_up_required,
        )
    }

    pub fn validate(&self) -> Result<(), HumanCenteredProtocolError> {
        self.metadata.validate()?;
        if self.metadata.contract_kind() != HumanCenteredContractKindV1::Outcome {
            return Err(HumanCenteredProtocolError::ContractKindMismatch);
        }
        validate_required_text(&self.objective)?;
        validate_text_collection(&self.observed_results)?;
        validate_text_collection(&self.success_criteria_satisfied)?;
        validate_text_collection(&self.success_criteria_unsatisfied)?;
        if self.decision_ref.is_none() && self.action_ref.is_none() {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }
        if let Some(reference) = &self.decision_ref {
            reference.validate()?;
            if reference.kind() != HumanCenteredContractKindV1::Decision
                || !self.metadata.dependency_refs().contains(reference)
            {
                return Err(HumanCenteredProtocolError::InvalidContract);
            }
        }
        if let Some(reference) = &self.action_ref {
            reference.validate()?;
            if reference.kind() != HumanCenteredContractKindV1::Action
                || !self.metadata.dependency_refs().contains(reference)
            {
                return Err(HumanCenteredProtocolError::InvalidContract);
            }
        }
        if let Some(reference) = &self.execution_receipt_ref {
            reference.validate()?;
            if reference.kind() != HumanCenteredContractKindV1::ExecutionReceipt
                || !self.metadata.dependency_refs().contains(reference)
                || self.action_ref.is_none()
            {
                return Err(HumanCenteredProtocolError::InvalidContract);
            }
        }
        if self.state == OutcomeStateV1::Unknown
            && self.attribution != OutcomeAttributionV1::InsufficientEvidence
        {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }
        match self.state {
            OutcomeStateV1::Observed
                if self.observed_results.is_empty() || self.evidence_refs.is_empty() =>
            {
                return Err(HumanCenteredProtocolError::InvalidContract);
            }
            OutcomeStateV1::Unknown
                if !self.observed_results.is_empty()
                    || !self.success_criteria_satisfied.is_empty()
                    || !self.success_criteria_unsatisfied.is_empty() =>
            {
                return Err(HumanCenteredProtocolError::InvalidContract);
            }
            OutcomeStateV1::Conflicted
                if self.success_criteria_satisfied.is_empty()
                    || self.success_criteria_unsatisfied.is_empty() =>
            {
                return Err(HumanCenteredProtocolError::InvalidContract);
            }
            _ => {}
        }
        if (self.attribution == OutcomeAttributionV1::ExecutionQuality
            && self.execution_receipt_ref.is_none())
            || (self.attribution == OutcomeAttributionV1::DecisionQuality
                && self.decision_ref.is_none())
        {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }
        if contract_digest("outcome-record-v1", &self.digest_input())? != self.record_digest {
            return Err(HumanCenteredProtocolError::DigestMismatch);
        }
        Ok(())
    }

    pub fn record_ref(&self) -> Result<HumanCenteredContractRefV1, HumanCenteredProtocolError> {
        self.validate()?;
        HumanCenteredContractRefV1::new(
            HumanCenteredContractKindV1::Outcome,
            self.metadata.contract_id(),
            self.metadata.revision(),
            self.record_digest.clone(),
        )
    }

    pub fn metadata(&self) -> &HumanCenteredContractMetadataV1 {
        &self.metadata
    }

    pub fn decision_ref(&self) -> Option<&HumanCenteredContractRefV1> {
        self.decision_ref.as_ref()
    }

    pub fn action_ref(&self) -> Option<&HumanCenteredContractRefV1> {
        self.action_ref.as_ref()
    }

    pub fn execution_receipt_ref(&self) -> Option<&HumanCenteredContractRefV1> {
        self.execution_receipt_ref.as_ref()
    }

    pub fn success_criteria_satisfied(&self) -> &[String] {
        &self.success_criteria_satisfied
    }
}
