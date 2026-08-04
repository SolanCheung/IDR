use super::{
    contract_digest, valid_digest, validate_json_payload, validate_required_text,
    validate_text_collection, ActionContractV1, AuthorizationDecisionV1,
    ExactActionAuthorizationV1, ExpectedProofBindingV1, HumanCenteredContractKindV1,
    HumanCenteredContractMetadataV1, HumanCenteredContractRefV1, HumanCenteredProofIdV1,
    HumanCenteredProtocolError, ProofKindV1, SignedProofEnvelopeV1, VerifiedProofV1,
};
use crate::ProductionReferenceV1;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStateV1 {
    Succeeded,
    Failed,
    PartiallySucceeded,
    Rejected,
    Compensated,
}

/// Complete request that an Execution Permit proof must sign.
///
/// The request is not an execution capability by itself. A Store may only turn
/// it into an executor-facing permit after verifying and durably consuming the
/// signed proof and reserving the Action-level exactly-once key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionPermitRequestV1 {
    request_ref: ProductionReferenceV1,
    action_ref: HumanCenteredContractRefV1,
    authorization_id: Uuid,
    authorization_digest: String,
    provider_ref: ProductionReferenceV1,
    owner_ref: ProductionReferenceV1,
    attempt: u32,
    lease_issued_at: u64,
    lease_until: u64,
    dispatch_nonce: String,
    idempotency_key: String,
    tenant_ref: ProductionReferenceV1,
    scope_ref: ProductionReferenceV1,
    purpose_ref: ProductionReferenceV1,
    policy_revision_ref: ProductionReferenceV1,
    request_digest: String,
}

impl ExecutionPermitRequestV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        action: &ActionContractV1,
        authorization: &ExactActionAuthorizationV1,
        provider_ref: ProductionReferenceV1,
        owner_ref: ProductionReferenceV1,
        attempt: u32,
        lease_issued_at: u64,
        lease_until: u64,
        dispatch_nonce: impl Into<String>,
    ) -> Result<Self, HumanCenteredProtocolError> {
        action.validate()?;
        let action_ref = action.record_ref()?;
        authorization.validate_for(action, lease_issued_at)?;
        let context = action.metadata().authority_context();
        let dispatch_nonce = dispatch_nonce.into();
        if authorization.decision() != AuthorizationDecisionV1::Approve
            || provider_ref.as_str().trim().is_empty()
            || owner_ref.as_str().trim().is_empty()
            || attempt == 0
            || lease_until <= lease_issued_at
            || lease_until > authorization.expires_at()
            || lease_until > action.metadata().valid_until()
            || dispatch_nonce.len() != 64
            || !valid_digest(&dispatch_nonce)
        {
            return Err(HumanCenteredProtocolError::ProofVerificationFailed);
        }
        let mut value = Self {
            request_ref: ProductionReferenceV1::new(format!(
                "execution-permit-request:{}",
                Uuid::new_v4()
            ))?,
            action_ref,
            authorization_id: authorization.authorization_id(),
            authorization_digest: authorization.authorization_digest()?,
            provider_ref,
            owner_ref,
            attempt,
            lease_issued_at,
            lease_until,
            dispatch_nonce,
            idempotency_key: action.idempotency_key().to_string(),
            tenant_ref: context.tenant_ref().clone(),
            scope_ref: context.scope_ref().clone(),
            purpose_ref: context.purpose_ref().clone(),
            policy_revision_ref: action.metadata().policy_revision_ref().clone(),
            request_digest: String::new(),
        };
        value.request_digest = {
            let input = value.digest_input();
            contract_digest("execution-permit-request-v1", &input)?
        };
        value.validate_for(action, authorization)?;
        Ok(value)
    }

    fn digest_input(&self) -> impl Serialize + '_ {
        (
            (
                &self.request_ref,
                &self.action_ref,
                self.authorization_id,
                &self.authorization_digest,
                &self.provider_ref,
                &self.owner_ref,
                self.attempt,
            ),
            (
                self.lease_issued_at,
                self.lease_until,
                &self.dispatch_nonce,
                &self.idempotency_key,
                &self.tenant_ref,
                &self.scope_ref,
                &self.purpose_ref,
                &self.policy_revision_ref,
            ),
        )
    }

    pub fn validate_for(
        &self,
        action: &ActionContractV1,
        authorization: &ExactActionAuthorizationV1,
    ) -> Result<(), HumanCenteredProtocolError> {
        action.validate()?;
        authorization.validate_for(action, self.lease_issued_at)?;
        let context = action.metadata().authority_context();
        if self.request_ref.as_str().trim().is_empty()
            || self.action_ref != action.record_ref()?
            || self.authorization_id != authorization.authorization_id()
            || self.authorization_digest != authorization.authorization_digest()?
            || authorization.decision() != AuthorizationDecisionV1::Approve
            || self.provider_ref.as_str().trim().is_empty()
            || self.owner_ref.as_str().trim().is_empty()
            || self.attempt == 0
            || self.lease_until <= self.lease_issued_at
            || self.lease_until > authorization.expires_at()
            || self.lease_until > action.metadata().valid_until()
            || self.dispatch_nonce.len() != 64
            || !valid_digest(&self.dispatch_nonce)
            || self.idempotency_key != action.idempotency_key()
            || self.tenant_ref != *context.tenant_ref()
            || self.scope_ref != *context.scope_ref()
            || self.purpose_ref != *context.purpose_ref()
            || self.policy_revision_ref != *action.metadata().policy_revision_ref()
            || contract_digest("execution-permit-request-v1", &self.digest_input())?
                != self.request_digest
        {
            return Err(HumanCenteredProtocolError::ProofVerificationFailed);
        }
        Ok(())
    }

    pub fn expected_proof_binding(
        &self,
        issuer_ref: ProductionReferenceV1,
    ) -> Result<ExpectedProofBindingV1, HumanCenteredProtocolError> {
        ExpectedProofBindingV1::new(
            ProofKindV1::ExecutionPermit,
            issuer_ref,
            self.request_ref.clone(),
            self.request_digest.clone(),
            self.tenant_ref.clone(),
            self.scope_ref.clone(),
            self.purpose_ref.clone(),
            self.policy_revision_ref.clone(),
        )
    }

    pub fn request_ref(&self) -> &ProductionReferenceV1 {
        &self.request_ref
    }

    pub fn action_ref(&self) -> &HumanCenteredContractRefV1 {
        &self.action_ref
    }

    pub fn authorization_id(&self) -> Uuid {
        self.authorization_id
    }

    pub fn provider_ref(&self) -> &ProductionReferenceV1 {
        &self.provider_ref
    }

    pub fn owner_ref(&self) -> &ProductionReferenceV1 {
        &self.owner_ref
    }

    pub fn attempt(&self) -> u32 {
        self.attempt
    }

    pub fn lease_issued_at(&self) -> u64 {
        self.lease_issued_at
    }

    pub fn lease_until(&self) -> u64 {
        self.lease_until
    }

    pub fn dispatch_nonce(&self) -> &str {
        &self.dispatch_nonce
    }

    pub fn idempotency_key(&self) -> &str {
        &self.idempotency_key
    }

    pub fn tenant_ref(&self) -> &ProductionReferenceV1 {
        &self.tenant_ref
    }

    pub fn request_digest(&self) -> &str {
        &self.request_digest
    }
}

/// Opaque result of binding one non-cloneable verified proof to one complete
/// Execution Permit request. A durable Store must still atomically consume its
/// proof ID and nonce before it can issue an executor-facing permit.
#[derive(Debug, PartialEq, Eq)]
pub struct VerifiedExecutionPermitRequestV1 {
    request: ExecutionPermitRequestV1,
    authorization: ExactActionAuthorizationV1,
    verified_proof: VerifiedProofV1,
}

pub fn verify_execution_permit_request(
    verified_proof: VerifiedProofV1,
    request: ExecutionPermitRequestV1,
    action: &ActionContractV1,
    authorization: &ExactActionAuthorizationV1,
) -> Result<VerifiedExecutionPermitRequestV1, HumanCenteredProtocolError> {
    request.validate_for(action, authorization)?;
    let claims = verified_proof.claims();
    let verified_at = verified_proof.verified_at();
    if claims.proof_kind() != ProofKindV1::ExecutionPermit
        || claims.subject_ref() != request.request_ref()
        || claims.subject_digest() != request.request_digest()
        || claims.tenant_ref() != request.tenant_ref()
        || claims.scope_ref() != &request.scope_ref
        || claims.purpose_ref() != &request.purpose_ref
        || claims.policy_revision_ref() != &request.policy_revision_ref
        || claims.issued_at() != request.lease_issued_at()
        || claims.expires_at() < request.lease_until()
        || claims.nonce() != request.dispatch_nonce()
        || verified_at < request.lease_issued_at()
        || verified_at >= request.lease_until()
    {
        return Err(HumanCenteredProtocolError::ProofVerificationFailed);
    }
    Ok(VerifiedExecutionPermitRequestV1 {
        request,
        authorization: authorization.clone(),
        verified_proof,
    })
}

impl VerifiedExecutionPermitRequestV1 {
    pub fn request(&self) -> &ExecutionPermitRequestV1 {
        &self.request
    }

    pub fn proof_id(&self) -> HumanCenteredProofIdV1 {
        self.verified_proof.claims().proof_id()
    }

    pub fn proof_nonce(&self) -> &str {
        self.verified_proof.claims().nonce()
    }

    pub fn proof_key_id(&self) -> &ProductionReferenceV1 {
        self.verified_proof.key_id()
    }

    pub fn authorization(&self) -> &ExactActionAuthorizationV1 {
        &self.authorization
    }

    pub fn proof_issuer_ref(&self) -> &ProductionReferenceV1 {
        self.verified_proof.claims().issuer_ref()
    }

    pub fn proof_expires_at(&self) -> u64 {
        self.verified_proof.claims().expires_at()
    }

    pub fn proof_envelope(&self) -> &SignedProofEnvelopeV1 {
        self.verified_proof.signed_envelope()
    }

    pub fn verified_at(&self) -> u64 {
        self.verified_proof.verified_at()
    }

    /// Re-checks every time-sensitive Action, Authorization, lease and signed
    /// proof invariant at the Store's trusted current time. This closes the
    /// verify-then-delay gap between proof verification and durable issuance.
    pub fn validate_at(
        &self,
        action: &ActionContractV1,
        trusted_now: u64,
    ) -> Result<(), HumanCenteredProtocolError> {
        self.request.validate_for(action, &self.authorization)?;
        self.authorization.validate_for(action, trusted_now)?;
        let claims = self.verified_proof.claims();
        if trusted_now < self.request.lease_issued_at()
            || trusted_now >= self.request.lease_until()
            || trusted_now < claims.issued_at()
            || trusted_now >= claims.expires_at()
            || trusted_now >= action.metadata().valid_until()
            || self.verified_at() > trusted_now
        {
            return Err(HumanCenteredProtocolError::ProofVerificationFailed);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionReceiptV1 {
    metadata: HumanCenteredContractMetadataV1,
    action_ref: HumanCenteredContractRefV1,
    permit_id: HumanCenteredProofIdV1,
    authorization_id: Uuid,
    execution_id: Uuid,
    provider_ref: ProductionReferenceV1,
    owner_ref: ProductionReferenceV1,
    dispatch_nonce: String,
    external_execution_ref: Option<ProductionReferenceV1>,
    idempotency_key: String,
    request_parameter_digest: String,
    attempt: u32,
    started_at: u64,
    completed_at: u64,
    state: ExecutionStateV1,
    result_payload: Option<Value>,
    result_payload_digest: Option<String>,
    applied_effects: Vec<String>,
    verification_results: Vec<String>,
    error_codes: Vec<String>,
    record_digest: String,
}

impl ExecutionReceiptV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn issue(
        metadata: HumanCenteredContractMetadataV1,
        action: &ActionContractV1,
        permit_id: HumanCenteredProofIdV1,
        authorization_id: Uuid,
        provider_ref: ProductionReferenceV1,
        owner_ref: ProductionReferenceV1,
        dispatch_nonce: impl Into<String>,
        external_execution_ref: Option<ProductionReferenceV1>,
        attempt: u32,
        started_at: u64,
        completed_at: u64,
        state: ExecutionStateV1,
        result_payload: Option<Value>,
        applied_effects: Vec<String>,
        verification_results: Vec<String>,
        error_codes: Vec<String>,
    ) -> Result<Self, HumanCenteredProtocolError> {
        action.validate()?;
        let action_ref = action.record_ref()?;
        if let Some(payload) = &result_payload {
            validate_json_payload(payload)?;
        }
        let result_payload_digest = result_payload
            .as_ref()
            .map(|payload| contract_digest("execution-result-v1", payload))
            .transpose()?;
        let dispatch_nonce = dispatch_nonce.into();
        let mut value = Self {
            metadata,
            action_ref,
            permit_id,
            authorization_id,
            execution_id: Uuid::new_v4(),
            provider_ref,
            owner_ref,
            dispatch_nonce,
            external_execution_ref,
            idempotency_key: action.idempotency_key().to_string(),
            request_parameter_digest: action.parameter_digest().to_string(),
            attempt,
            started_at,
            completed_at,
            state,
            result_payload,
            result_payload_digest,
            applied_effects,
            verification_results,
            error_codes,
            record_digest: String::new(),
        };
        value.record_digest = {
            let input = value.digest_input();
            contract_digest("execution-receipt-v1", &input)?
        };
        value.validate()?;
        Ok(value)
    }

    fn digest_input(&self) -> impl Serialize + '_ {
        (
            (
                &self.metadata,
                &self.action_ref,
                self.permit_id,
                self.authorization_id,
                self.execution_id,
                &self.provider_ref,
                &self.owner_ref,
                &self.dispatch_nonce,
                &self.external_execution_ref,
                &self.idempotency_key,
                &self.request_parameter_digest,
            ),
            (
                self.attempt,
                self.started_at,
                self.completed_at,
                self.state,
                &self.result_payload,
                &self.result_payload_digest,
                &self.applied_effects,
                &self.verification_results,
                &self.error_codes,
            ),
        )
    }

    pub fn validate(&self) -> Result<(), HumanCenteredProtocolError> {
        self.metadata.validate()?;
        if self.metadata.contract_kind() != HumanCenteredContractKindV1::ExecutionReceipt {
            return Err(HumanCenteredProtocolError::ContractKindMismatch);
        }
        self.action_ref.validate()?;
        validate_required_text(&self.idempotency_key)?;
        validate_text_collection(&self.applied_effects)?;
        validate_text_collection(&self.verification_results)?;
        validate_text_collection(&self.error_codes)?;
        if let Some(payload) = &self.result_payload {
            validate_json_payload(payload)?;
        }
        if self.action_ref.kind() != HumanCenteredContractKindV1::Action
            || !self.metadata.dependency_refs().contains(&self.action_ref)
            || self.permit_id.is_empty()
            || self.authorization_id.is_nil()
            || self.execution_id.is_nil()
            || self.provider_ref.as_str().trim().is_empty()
            || self.owner_ref.as_str().trim().is_empty()
            || self.dispatch_nonce.len() != 64
            || !valid_digest(&self.dispatch_nonce)
            || self.attempt == 0
            || self.started_at == 0
            || self.completed_at < self.started_at
            || !valid_digest(&self.request_parameter_digest)
        {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }
        let expected_result_digest = self
            .result_payload
            .as_ref()
            .map(|payload| contract_digest("execution-result-v1", payload))
            .transpose()?;
        if expected_result_digest != self.result_payload_digest {
            return Err(HumanCenteredProtocolError::DigestMismatch);
        }
        match self.state {
            ExecutionStateV1::Succeeded
                if self.result_payload.is_none()
                    || self.verification_results.is_empty()
                    || !self.error_codes.is_empty() =>
            {
                return Err(HumanCenteredProtocolError::InvalidContract);
            }
            ExecutionStateV1::Failed | ExecutionStateV1::Rejected
                if self.error_codes.is_empty() =>
            {
                return Err(HumanCenteredProtocolError::InvalidContract);
            }
            _ => {}
        }
        if contract_digest("execution-receipt-v1", &self.digest_input())? != self.record_digest {
            return Err(HumanCenteredProtocolError::DigestMismatch);
        }
        Ok(())
    }

    pub fn record_ref(&self) -> Result<HumanCenteredContractRefV1, HumanCenteredProtocolError> {
        self.validate()?;
        HumanCenteredContractRefV1::new(
            HumanCenteredContractKindV1::ExecutionReceipt,
            self.metadata.contract_id(),
            self.metadata.revision(),
            self.record_digest.clone(),
        )
    }

    pub fn metadata(&self) -> &HumanCenteredContractMetadataV1 {
        &self.metadata
    }

    pub fn action_ref(&self) -> &HumanCenteredContractRefV1 {
        &self.action_ref
    }

    pub fn permit_id(&self) -> HumanCenteredProofIdV1 {
        self.permit_id
    }

    pub fn authorization_id(&self) -> Uuid {
        self.authorization_id
    }

    pub fn provider_ref(&self) -> &ProductionReferenceV1 {
        &self.provider_ref
    }

    pub fn owner_ref(&self) -> &ProductionReferenceV1 {
        &self.owner_ref
    }

    pub fn dispatch_nonce(&self) -> &str {
        &self.dispatch_nonce
    }

    pub fn idempotency_key(&self) -> &str {
        &self.idempotency_key
    }

    pub fn request_parameter_digest(&self) -> &str {
        &self.request_parameter_digest
    }

    pub fn started_at(&self) -> u64 {
        self.started_at
    }

    pub fn completed_at(&self) -> u64 {
        self.completed_at
    }

    pub fn attempt(&self) -> u32 {
        self.attempt
    }

    pub fn state(&self) -> ExecutionStateV1 {
        self.state
    }

    pub fn expected_provider_proof_binding(
        &self,
    ) -> Result<ExpectedProofBindingV1, HumanCenteredProtocolError> {
        let context = self.metadata.authority_context();
        ExpectedProofBindingV1::new(
            ProofKindV1::ProviderExecutionReceipt,
            self.provider_ref.clone(),
            self.proof_subject_ref()?,
            self.record_digest.clone(),
            context.tenant_ref().clone(),
            context.scope_ref().clone(),
            context.purpose_ref().clone(),
            self.metadata.policy_revision_ref().clone(),
        )
    }

    pub fn proof_subject_ref(&self) -> Result<ProductionReferenceV1, HumanCenteredProtocolError> {
        ProductionReferenceV1::new(format!(
            "execution-receipt:{}:{}",
            self.metadata.contract_id(),
            self.metadata.revision()
        ))
        .map_err(|_| HumanCenteredProtocolError::InvalidContract)
    }
}

/// Opaque provider attestation over the complete Receipt digest and dispatch
/// nonce. The Store consumes this value instead of accepting a bare Receipt.
#[derive(Debug, PartialEq, Eq)]
pub struct VerifiedExecutionReceiptV1 {
    receipt: ExecutionReceiptV1,
    verified_proof: VerifiedProofV1,
}

pub fn verify_execution_receipt(
    verified_proof: VerifiedProofV1,
    receipt: ExecutionReceiptV1,
) -> Result<VerifiedExecutionReceiptV1, HumanCenteredProtocolError> {
    receipt.validate()?;
    let claims = verified_proof.claims();
    let expected = receipt.expected_provider_proof_binding()?;
    if claims.proof_kind() != ProofKindV1::ProviderExecutionReceipt
        || claims.issuer_ref() != receipt.provider_ref()
        || claims.subject_ref() != &receipt.proof_subject_ref()?
        || claims.subject_digest() != receipt.record_ref()?.record_digest()
        || claims.tenant_ref() != expected.tenant_ref()
        || claims.scope_ref() != expected.scope_ref()
        || claims.purpose_ref() != expected.purpose_ref()
        || claims.policy_revision_ref() != expected.policy_revision_ref()
        || claims.nonce() != receipt.dispatch_nonce()
        || claims.issued_at() < receipt.completed_at()
        || verified_proof.verified_at() < claims.issued_at()
    {
        return Err(HumanCenteredProtocolError::ProofVerificationFailed);
    }
    Ok(VerifiedExecutionReceiptV1 {
        receipt,
        verified_proof,
    })
}

impl VerifiedExecutionReceiptV1 {
    pub fn receipt(&self) -> &ExecutionReceiptV1 {
        &self.receipt
    }

    pub fn proof_envelope(&self) -> &SignedProofEnvelopeV1 {
        self.verified_proof.signed_envelope()
    }

    pub fn verified_at(&self) -> u64 {
        self.verified_proof.verified_at()
    }

    pub fn validate_at(&self, trusted_now: u64) -> Result<(), HumanCenteredProtocolError> {
        self.receipt.validate()?;
        let claims = self.verified_proof.claims();
        if trusted_now < self.receipt.completed_at()
            || trusted_now < claims.issued_at()
            || trusted_now >= claims.expires_at()
            || self.verified_at() > trusted_now
        {
            return Err(HumanCenteredProtocolError::ProofVerificationFailed);
        }
        Ok(())
    }

    pub fn into_parts(self) -> (ExecutionReceiptV1, SignedProofEnvelopeV1, u64) {
        (
            self.receipt,
            self.verified_proof.signed_envelope().clone(),
            self.verified_proof.verified_at(),
        )
    }
}
