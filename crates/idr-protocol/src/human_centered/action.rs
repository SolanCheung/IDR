use super::{
    canonical_references, contract_digest, validate_json_payload, validate_required_text,
    validate_text_collection, HumanCenteredContractKindV1, HumanCenteredContractMetadataV1,
    HumanCenteredContractRefV1, HumanCenteredProtocolError, ImpactLevelV1,
    HUMAN_CENTERED_RULE_VERSION, HUMAN_CENTERED_SCHEMA_VERSION,
};
use crate::ProductionReferenceV1;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionAuthorizationStateV1 {
    NotRequired,
    Pending,
    Authorized,
    Denied,
    Expired,
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionContractV1 {
    metadata: HumanCenteredContractMetadataV1,
    decision_ref: Option<HumanCenteredContractRefV1>,
    operation_ref: ProductionReferenceV1,
    parameters: Value,
    parameter_digest: String,
    preconditions: Vec<String>,
    required_authority_refs: Vec<ProductionReferenceV1>,
    authorization_state: ActionAuthorizationStateV1,
    impact_level: ImpactLevelV1,
    reversible: bool,
    idempotency_key: String,
    verification_requirements: Vec<String>,
    compensation_operations: Vec<ProductionReferenceV1>,
    record_digest: String,
}

impl ActionContractV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn issue(
        metadata: HumanCenteredContractMetadataV1,
        decision_ref: Option<HumanCenteredContractRefV1>,
        operation_ref: ProductionReferenceV1,
        parameters: Value,
        preconditions: Vec<String>,
        required_authority_refs: Vec<ProductionReferenceV1>,
        authorization_state: ActionAuthorizationStateV1,
        impact_level: ImpactLevelV1,
        reversible: bool,
        idempotency_key: impl Into<String>,
        verification_requirements: Vec<String>,
        compensation_operations: Vec<ProductionReferenceV1>,
    ) -> Result<Self, HumanCenteredProtocolError> {
        validate_json_payload(&parameters)?;
        let parameter_digest = contract_digest("action-parameters-v1", &parameters)?;
        let mut value = Self {
            metadata,
            decision_ref,
            operation_ref,
            parameters,
            parameter_digest,
            preconditions,
            required_authority_refs: canonical_references(required_authority_refs)?,
            authorization_state,
            impact_level,
            reversible,
            idempotency_key: idempotency_key.into(),
            verification_requirements,
            compensation_operations: canonical_references(compensation_operations)?,
            record_digest: String::new(),
        };
        value.record_digest = {
            let input = value.digest_input();
            contract_digest("action-contract-v1", &input)?
        };
        value.validate()?;
        Ok(value)
    }

    fn digest_input(&self) -> impl Serialize + '_ {
        (
            (
                &self.metadata,
                &self.decision_ref,
                &self.operation_ref,
                &self.parameters,
                &self.parameter_digest,
                &self.preconditions,
            ),
            (
                &self.required_authority_refs,
                self.authorization_state,
                self.impact_level,
                self.reversible,
                &self.idempotency_key,
                &self.verification_requirements,
                &self.compensation_operations,
            ),
        )
    }

    pub fn validate(&self) -> Result<(), HumanCenteredProtocolError> {
        self.metadata.validate()?;
        if self.metadata.contract_kind() != HumanCenteredContractKindV1::Action {
            return Err(HumanCenteredProtocolError::ContractKindMismatch);
        }
        validate_text_collection(&self.preconditions)?;
        validate_text_collection(&self.verification_requirements)?;
        validate_required_text(&self.idempotency_key)?;
        validate_json_payload(&self.parameters)?;
        if !self.parameters.is_object()
            || self.operation_ref.as_str().trim().is_empty()
            || self.verification_requirements.is_empty()
            || contract_digest("action-parameters-v1", &self.parameters)? != self.parameter_digest
        {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }
        if let Some(decision_ref) = &self.decision_ref {
            decision_ref.validate()?;
            if decision_ref.kind() != HumanCenteredContractKindV1::Decision
                || !self.metadata.dependency_refs().contains(decision_ref)
            {
                return Err(HumanCenteredProtocolError::InvalidContract);
            }
        }
        let high_impact = self.impact_level.rank() >= ImpactLevelV1::High.rank();
        let irreversible = !self.reversible;
        if (high_impact && self.authorization_state == ActionAuthorizationStateV1::NotRequired)
            || (high_impact || irreversible) && self.decision_ref.is_none()
        {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }
        if self.reversible && self.compensation_operations.is_empty() {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }
        if contract_digest("action-contract-v1", &self.digest_input())? != self.record_digest {
            return Err(HumanCenteredProtocolError::DigestMismatch);
        }
        Ok(())
    }

    pub fn record_ref(&self) -> Result<HumanCenteredContractRefV1, HumanCenteredProtocolError> {
        self.validate()?;
        HumanCenteredContractRefV1::new(
            HumanCenteredContractKindV1::Action,
            self.metadata.contract_id(),
            self.metadata.revision(),
            self.record_digest.clone(),
        )
    }

    pub fn metadata(&self) -> &HumanCenteredContractMetadataV1 {
        &self.metadata
    }

    pub fn parameter_digest(&self) -> &str {
        &self.parameter_digest
    }

    pub fn decision_ref(&self) -> Option<&HumanCenteredContractRefV1> {
        self.decision_ref.as_ref()
    }

    pub fn operation_ref(&self) -> &ProductionReferenceV1 {
        &self.operation_ref
    }

    pub fn required_authority_refs(&self) -> &[ProductionReferenceV1] {
        &self.required_authority_refs
    }

    pub fn impact_level(&self) -> ImpactLevelV1 {
        self.impact_level
    }

    pub fn authorization_state(&self) -> ActionAuthorizationStateV1 {
        self.authorization_state
    }

    pub fn idempotency_key(&self) -> &str {
        &self.idempotency_key
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorizationDecisionV1 {
    Approve,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExactActionAuthorizationV1 {
    authorization_id: Uuid,
    action_ref: HumanCenteredContractRefV1,
    parameter_digest: String,
    actor_ref: ProductionReferenceV1,
    decision: AuthorizationDecisionV1,
    scope_ref: ProductionReferenceV1,
    authority_context_digest: String,
    issued_at: u64,
    expires_at: u64,
    schema_version: u16,
}

impl ExactActionAuthorizationV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn issue(
        action: &ActionContractV1,
        actor_ref: ProductionReferenceV1,
        decision: AuthorizationDecisionV1,
        scope_ref: ProductionReferenceV1,
        issued_at: u64,
        expires_at: u64,
    ) -> Result<Self, HumanCenteredProtocolError> {
        action.validate()?;
        let authority_context_digest = contract_digest(
            "authority-context-v1",
            action.metadata().authority_context(),
        )?;
        let value = Self {
            authorization_id: Uuid::new_v4(),
            action_ref: action.record_ref()?,
            parameter_digest: action.parameter_digest.clone(),
            actor_ref,
            decision,
            scope_ref,
            authority_context_digest,
            issued_at,
            expires_at,
            schema_version: HUMAN_CENTERED_SCHEMA_VERSION,
        };
        value.validate_for(action, issued_at)?;
        Ok(value)
    }

    pub fn validate_for(
        &self,
        action: &ActionContractV1,
        valid_at: u64,
    ) -> Result<(), HumanCenteredProtocolError> {
        action.validate()?;
        self.action_ref.validate()?;
        if self.authorization_id.is_nil()
            || self.action_ref != action.record_ref()?
            || self.parameter_digest != action.parameter_digest
            || self.actor_ref != *action.metadata().authority_context().actor_ref()
            || self.scope_ref != *action.metadata().authority_context().scope_ref()
            || contract_digest(
                "authority-context-v1",
                action.metadata().authority_context(),
            )? != self.authority_context_digest
            || self.issued_at == 0
            || self.expires_at <= self.issued_at
            || valid_at < self.issued_at
            || valid_at >= self.expires_at
            || self.schema_version != HUMAN_CENTERED_SCHEMA_VERSION
        {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }
        Ok(())
    }

    pub fn authorization_id(&self) -> Uuid {
        self.authorization_id
    }

    pub fn action_ref(&self) -> &HumanCenteredContractRefV1 {
        &self.action_ref
    }

    pub fn decision(&self) -> AuthorizationDecisionV1 {
        self.decision
    }

    pub fn issued_at(&self) -> u64 {
        self.issued_at
    }

    pub fn expires_at(&self) -> u64 {
        self.expires_at
    }

    pub fn authorization_digest(&self) -> Result<String, HumanCenteredProtocolError> {
        contract_digest("exact-action-authorization-v1", self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionAdmissionFactsV1 {
    pub capability_registered: bool,
    pub capability_enabled: bool,
    pub actor_authority_valid: bool,
    pub agent_authority_valid: bool,
    pub parameters_valid: bool,
    pub preconditions_satisfied: bool,
    pub policy_allows: bool,
    pub verification_available: bool,
    pub compensation_available_if_required: bool,
    pub dependencies_current: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionAdmissionOutcomeV1 {
    Admit,
    WaitAuthorization,
    ExecutionPending,
    AlreadyExecuted,
    Reject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionAdmissionDecisionV1 {
    pub outcome: ActionAdmissionOutcomeV1,
    pub reason_codes: Vec<String>,
    pub rule_version: u64,
}

pub fn evaluate_action_admission(
    action: &ActionContractV1,
    facts: &ActionAdmissionFactsV1,
    authorization: Option<&ExactActionAuthorizationV1>,
    valid_at: u64,
) -> ActionAdmissionDecisionV1 {
    if action.validate().is_err() {
        return rejected_action_admission("action_contract_invalid");
    }
    if valid_at == 0 || valid_at >= action.metadata().valid_until() {
        return rejected_action_admission("action_contract_expired");
    }
    match action.authorization_state {
        ActionAuthorizationStateV1::Denied => {
            return rejected_action_admission("action_authorization_denied");
        }
        ActionAuthorizationStateV1::Expired => {
            return rejected_action_admission("action_authorization_expired");
        }
        ActionAuthorizationStateV1::Revoked => {
            return rejected_action_admission("action_authorization_revoked");
        }
        ActionAuthorizationStateV1::Pending | ActionAuthorizationStateV1::Authorized => {
            let Some(authorization) = authorization else {
                return ActionAdmissionDecisionV1 {
                    outcome: ActionAdmissionOutcomeV1::WaitAuthorization,
                    reason_codes: vec!["exact_action_authorization_required".to_string()],
                    rule_version: HUMAN_CENTERED_RULE_VERSION,
                };
            };
            if authorization.validate_for(action, valid_at).is_err()
                || authorization.decision != AuthorizationDecisionV1::Approve
            {
                return rejected_action_admission("exact_action_authorization_invalid");
            }
        }
        ActionAuthorizationStateV1::NotRequired => {}
    }
    let checks = [
        ("capability_unregistered", facts.capability_registered),
        ("capability_disabled", facts.capability_enabled),
        ("actor_authority_invalid", facts.actor_authority_valid),
        ("agent_authority_invalid", facts.agent_authority_valid),
        ("parameters_invalid", facts.parameters_valid),
        ("preconditions_unsatisfied", facts.preconditions_satisfied),
        ("policy_denied", facts.policy_allows),
        ("verification_unavailable", facts.verification_available),
        (
            "compensation_unavailable",
            facts.compensation_available_if_required,
        ),
        ("dependency_stale", facts.dependencies_current),
    ];
    let reasons: Vec<_> = checks
        .into_iter()
        .filter(|(_, passed)| !passed)
        .map(|(reason, _)| reason.to_string())
        .collect();
    ActionAdmissionDecisionV1 {
        outcome: if reasons.is_empty() {
            ActionAdmissionOutcomeV1::Admit
        } else {
            ActionAdmissionOutcomeV1::Reject
        },
        reason_codes: reasons,
        rule_version: HUMAN_CENTERED_RULE_VERSION,
    }
}

fn rejected_action_admission(reason: &str) -> ActionAdmissionDecisionV1 {
    ActionAdmissionDecisionV1 {
        outcome: ActionAdmissionOutcomeV1::Reject,
        reason_codes: vec![reason.to_string()],
        rule_version: HUMAN_CENTERED_RULE_VERSION,
    }
}
