//! Runtime-sealed, command-driven IDR control plane.
//!
//! The public wire surface contains Candidates and Proof envelopes. This
//! module owns authoritative issuance. Its mutation engine is crate-private;
//! the only production entry is the concrete PostgreSQL Orchestrator.

#![cfg_attr(not(feature = "postgres-authority"), allow(dead_code))]

use idr_protocol::production::{
    canonical_digest_v1, CandidateKindV1, CandidateSubmissionV1, ProductionProofEnvelopeV1,
    ProductionProofKindV1, TrustDomainV1, VerifiedProductionProofV1,
};
use idr_protocol::ReferenceV1;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

const IDR_MAX_RENDERED_BYTES_V1: usize = 1_048_576;
const IDR_MAX_IDEMPOTENCY_KEY_BYTES_V1: usize = 256;
const IDR_RECEIPT_OUTCOME_OBSERVATION_WINDOW_SECONDS_V1: u64 = 86_400;
const IDR_OUTCOME_HUMAN_MODEL_WINDOW_SECONDS_V1: u64 = 604_800;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdrRunStateV1 {
    Pending,
    Running,
    WaitingInput,
    WaitingAuthorization,
    WaitingDependency,
    ReconciliationRequired,
    Succeeded,
    Rejected,
    Failed,
    Cancelled,
    TimedOut,
    Invalidated,
}

pub type IdrTrustDomainV1 = TrustDomainV1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionAggregateStateV1 {
    None,
    ReservationCreated,
    PermitIssued,
    PermitDelivered,
    DispatchStarted,
    AwaitingProvider,
    ReconciliationRequired,
    Succeeded,
    Failed,
    Rejected,
    Cancelled,
    Compensated,
    Expired,
}

impl ExecutionAggregateStateV1 {
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded
                | Self::Failed
                | Self::Rejected
                | Self::Cancelled
                | Self::Compensated
                | Self::Expired
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoritativeRecordRefV1 {
    record_id: Uuid,
    revision: u64,
    candidate_kind: CandidateKindV1,
    trust_domain: IdrTrustDomainV1,
    environment_ref: ReferenceV1,
    valid_from: u64,
    valid_until: u64,
    record_digest: String,
}

impl AuthoritativeRecordRefV1 {
    pub fn record_id(&self) -> Uuid {
        self.record_id
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn candidate_kind(&self) -> CandidateKindV1 {
        self.candidate_kind
    }

    pub fn trust_domain(&self) -> IdrTrustDomainV1 {
        self.trust_domain
    }

    pub fn environment_ref(&self) -> &ReferenceV1 {
        &self.environment_ref
    }

    pub fn valid_from(&self) -> u64 {
        self.valid_from
    }

    pub fn valid_until(&self) -> u64 {
        self.valid_until
    }

    pub fn record_digest(&self) -> &str {
        &self.record_digest
    }
}

/// Runtime-issued authoritative state. It is Serialize-only and all fields are
/// private. External crates cannot construct or deserialize this type.
///
/// ```compile_fail
/// use idr_runtime::AuthoritativeRecordV1;
/// let _: AuthoritativeRecordV1 =
///     serde_json::from_str(r#"{"record_ref":{}}"#).unwrap();
/// ```
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AuthoritativeRecordV1 {
    record_ref: AuthoritativeRecordRefV1,
    trust_domain: IdrTrustDomainV1,
    environment_ref: ReferenceV1,
    run_id: Uuid,
    turn_id: Uuid,
    tenant_ref: ReferenceV1,
    subject_ref: ReferenceV1,
    policy_revision_ref: ReferenceV1,
    candidate_id: Uuid,
    candidate_digest: String,
    payload: Value,
    issued_by_command_id: Uuid,
    issued_at: u64,
    valid_from: u64,
    valid_until: u64,
    proof_ids: Vec<Uuid>,
}

impl AuthoritativeRecordV1 {
    pub fn record_ref(&self) -> &AuthoritativeRecordRefV1 {
        &self.record_ref
    }

    pub(crate) fn trust_domain(&self) -> IdrTrustDomainV1 {
        self.trust_domain
    }

    pub(crate) fn environment_ref(&self) -> &ReferenceV1 {
        &self.environment_ref
    }

    pub fn run_id(&self) -> Uuid {
        self.run_id
    }

    pub fn tenant_ref(&self) -> &ReferenceV1 {
        &self.tenant_ref
    }

    pub fn subject_ref(&self) -> &ReferenceV1 {
        &self.subject_ref
    }

    pub fn candidate_digest(&self) -> &str {
        &self.candidate_digest
    }

    pub fn payload(&self) -> &Value {
        &self.payload
    }

    pub fn valid_from(&self) -> u64 {
        self.valid_from
    }

    pub fn valid_until(&self) -> u64 {
        self.valid_until
    }
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct AuthoritativeRecordDigestMaterialV1<'a> {
    record_id: Uuid,
    revision: u64,
    candidate_kind: CandidateKindV1,
    trust_domain: IdrTrustDomainV1,
    environment_ref: &'a ReferenceV1,
    valid_from: u64,
    valid_until: u64,
    run_id: Uuid,
    turn_id: Uuid,
    tenant_ref: &'a ReferenceV1,
    subject_ref: &'a ReferenceV1,
    policy_revision_ref: &'a ReferenceV1,
    candidate_id: Uuid,
    candidate_digest: &'a str,
    payload: &'a Value,
    issued_by_command_id: Uuid,
    issued_at: u64,
    proof_ids: &'a [Uuid],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "command", content = "payload")]
pub enum IdrCommandV1 {
    StartRun {
        input: CandidateSubmissionV1,
    },
    RecordContext {
        context: CandidateSubmissionV1,
    },
    RecordIntent {
        intent: CandidateSubmissionV1,
    },
    RecordDecision {
        decision: CandidateSubmissionV1,
    },
    RecordTurn {
        turn: CandidateSubmissionV1,
    },
    RecordResponse {
        response: CandidateSubmissionV1,
    },
    ConsumeResponseSend {
        response_ref: AuthoritativeRecordRefV1,
        rendered_bytes: Vec<u8>,
        channel_ref: ReferenceV1,
        audience_ref: ReferenceV1,
        send_nonce: String,
    },
    RecordAction {
        action: CandidateSubmissionV1,
    },
    AdmitAction {
        action_ref: AuthoritativeRecordRefV1,
        action_digest: String,
        provider_ref: ReferenceV1,
        owner_ref: ReferenceV1,
        idempotency_key: String,
    },
    ReserveExecution {
        action_admission_ref: AuthoritativeRecordRefV1,
        action_ref: AuthoritativeRecordRefV1,
        action_digest: String,
        provider_ref: ReferenceV1,
        owner_ref: ReferenceV1,
        idempotency_key: String,
        attempt: u32,
        lease_until: u64,
        dispatch_nonce: String,
    },
    RecoverPermit {
        reservation_id: Uuid,
    },
    DeliverPermit {
        reservation_id: Uuid,
        permit_id: Uuid,
    },
    StartDispatch {
        reservation_id: Uuid,
        permit_id: Uuid,
    },
    CommitExecutionReceipt {
        receipt: CandidateSubmissionV1,
        reservation_id: Uuid,
        permit_id: Uuid,
        provider_ref: ReferenceV1,
        dispatch_nonce: String,
        attempt: u32,
    },
    ReconcileExecution {
        reservation_id: Uuid,
        resolution: ReconciliationResolutionV1,
    },
    ExpireExecutionLease {
        reservation_id: Uuid,
    },
    InvalidateRecord {
        record_ref: AuthoritativeRecordRefV1,
        reason_ref: ReferenceV1,
    },
    RecordOutcome {
        outcome: CandidateSubmissionV1,
    },
    ProposeHumanModelCandidate {
        candidate: CandidateSubmissionV1,
    },
    RecordHumanModelPromotion {
        decision: CandidateSubmissionV1,
        source_candidate_ref: AuthoritativeRecordRefV1,
    },
    PromoteHumanModelAssertion {
        assertion: CandidateSubmissionV1,
        promotion_decision_ref: AuthoritativeRecordRefV1,
    },
    CorrectHumanModelAssertion {
        assertion_ref: AuthoritativeRecordRefV1,
        correction: CandidateSubmissionV1,
    },
    CancelRun {
        reason_ref: ReferenceV1,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconciliationResolutionV1 {
    Succeeded,
    Failed,
    Rejected,
    Compensated,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdrCommandEnvelopeV1 {
    command_id: Uuid,
    trust_domain: IdrTrustDomainV1,
    environment_ref: ReferenceV1,
    run_id: Uuid,
    expected_aggregate_version: u64,
    actor_ref: ReferenceV1,
    caller_ref: ReferenceV1,
    tenant_ref: ReferenceV1,
    scope_ref: ReferenceV1,
    purpose_ref: ReferenceV1,
    policy_revision_ref: ReferenceV1,
    correlation_ref: ReferenceV1,
    causation_ref: ReferenceV1,
    command: IdrCommandV1,
    proofs: Vec<ProductionProofEnvelopeV1>,
}

/// Canonical proof target for one exact command. Proof envelopes are excluded
/// to avoid a circular signature dependency; every command and routing field is
/// included so a valid proof cannot be redirected to another run, revision,
/// target record or causation chain.
#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct UnsignedCommandEnvelopeV1<'a> {
    command_id: Uuid,
    trust_domain: IdrTrustDomainV1,
    environment_ref: &'a ReferenceV1,
    run_id: Uuid,
    expected_aggregate_version: u64,
    actor_ref: &'a ReferenceV1,
    caller_ref: &'a ReferenceV1,
    tenant_ref: &'a ReferenceV1,
    scope_ref: &'a ReferenceV1,
    purpose_ref: &'a ReferenceV1,
    policy_revision_ref: &'a ReferenceV1,
    correlation_ref: &'a ReferenceV1,
    causation_ref: &'a ReferenceV1,
    command: &'a IdrCommandV1,
}

impl IdrCommandEnvelopeV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        trust_domain: IdrTrustDomainV1,
        environment_ref: ReferenceV1,
        run_id: Uuid,
        expected_aggregate_version: u64,
        actor_ref: ReferenceV1,
        caller_ref: ReferenceV1,
        tenant_ref: ReferenceV1,
        scope_ref: ReferenceV1,
        purpose_ref: ReferenceV1,
        policy_revision_ref: ReferenceV1,
        correlation_ref: ReferenceV1,
        causation_ref: ReferenceV1,
        command: IdrCommandV1,
        proofs: Vec<ProductionProofEnvelopeV1>,
    ) -> Result<Self, IdrRuntimeErrorV1> {
        let envelope = Self {
            command_id: Uuid::new_v4(),
            trust_domain,
            environment_ref,
            run_id,
            expected_aggregate_version,
            actor_ref,
            caller_ref,
            tenant_ref,
            scope_ref,
            purpose_ref,
            policy_revision_ref,
            correlation_ref,
            causation_ref,
            command,
            proofs,
        };
        envelope.validate()?;
        Ok(envelope)
    }

    pub fn validate(&self) -> Result<(), IdrRuntimeErrorV1> {
        if self.command_id.is_nil()
            || self.run_id.is_nil()
            || self.expected_aggregate_version > 9_007_199_254_740_991
            || self.proofs.len() > 32
        {
            return Err(IdrRuntimeErrorV1::InvalidCommand);
        }
        let mut proof_ids = BTreeSet::new();
        let mut nonces = BTreeSet::new();
        for proof in &self.proofs {
            proof.validate_shape()?;
            if proof.claims().trust_domain() != self.trust_domain
                || !proof_ids.insert(proof.claims().proof_id())
                || !nonces.insert(proof.claims().nonce())
            {
                return Err(IdrRuntimeErrorV1::ProofReplay);
            }
        }
        if let Some(candidate) = self.command.candidate() {
            candidate.validate()?;
            if candidate.run_id() != self.run_id
                || candidate.tenant_ref() != &self.tenant_ref
                || candidate.scope_ref() != &self.scope_ref
                || candidate.purpose_ref() != &self.purpose_ref
                || candidate.policy_revision_ref() != &self.policy_revision_ref
                || candidate.candidate_kind() != self.command.expected_candidate_kind().unwrap()
            {
                return Err(IdrRuntimeErrorV1::InvalidCommand);
            }
        }
        for digest in self.command.explicit_digests() {
            if !valid_digest(digest) {
                return Err(IdrRuntimeErrorV1::InvalidCommand);
            }
        }
        self.command.validate_budgets()?;
        Ok(())
    }

    /// Attaches untrusted proof envelopes without changing the command ID or
    /// unsigned proof target. The full storage/idempotency digest does change
    /// because it includes the attached proof envelopes.
    pub fn with_proofs(
        mut self,
        proofs: Vec<ProductionProofEnvelopeV1>,
    ) -> Result<Self, IdrRuntimeErrorV1> {
        self.proofs = proofs;
        self.validate()?;
        Ok(self)
    }

    pub fn canonical_digest(&self) -> Result<String, IdrRuntimeErrorV1> {
        canonical_digest_v1("idr-command-envelope-v1", self).map_err(Into::into)
    }

    pub fn command_id(&self) -> Uuid {
        self.command_id
    }

    pub fn trust_domain(&self) -> IdrTrustDomainV1 {
        self.trust_domain
    }

    pub fn environment_ref(&self) -> &ReferenceV1 {
        &self.environment_ref
    }

    pub fn run_id(&self) -> Uuid {
        self.run_id
    }

    pub fn expected_aggregate_version(&self) -> u64 {
        self.expected_aggregate_version
    }

    pub fn actor_ref(&self) -> &ReferenceV1 {
        &self.actor_ref
    }

    pub fn caller_ref(&self) -> &ReferenceV1 {
        &self.caller_ref
    }

    pub fn tenant_ref(&self) -> &ReferenceV1 {
        &self.tenant_ref
    }

    pub fn scope_ref(&self) -> &ReferenceV1 {
        &self.scope_ref
    }

    pub fn purpose_ref(&self) -> &ReferenceV1 {
        &self.purpose_ref
    }

    pub fn policy_revision_ref(&self) -> &ReferenceV1 {
        &self.policy_revision_ref
    }

    pub fn correlation_ref(&self) -> &ReferenceV1 {
        &self.correlation_ref
    }

    pub fn causation_ref(&self) -> &ReferenceV1 {
        &self.causation_ref
    }

    pub fn command(&self) -> &IdrCommandV1 {
        &self.command
    }

    pub fn proofs(&self) -> &[ProductionProofEnvelopeV1] {
        &self.proofs
    }

    pub fn proof_subject(&self) -> Result<(ReferenceV1, String), IdrRuntimeErrorV1> {
        let unsigned = UnsignedCommandEnvelopeV1 {
            command_id: self.command_id,
            trust_domain: self.trust_domain,
            environment_ref: &self.environment_ref,
            run_id: self.run_id,
            expected_aggregate_version: self.expected_aggregate_version,
            actor_ref: &self.actor_ref,
            caller_ref: &self.caller_ref,
            tenant_ref: &self.tenant_ref,
            scope_ref: &self.scope_ref,
            purpose_ref: &self.purpose_ref,
            policy_revision_ref: &self.policy_revision_ref,
            correlation_ref: &self.correlation_ref,
            causation_ref: &self.causation_ref,
            command: &self.command,
        };
        let digest = canonical_digest_v1("idr-unsigned-command-envelope-v1", &unsigned)?;
        Ok((
            ReferenceV1::new(format!("command:{}", self.command_id))?,
            digest,
        ))
    }

    pub fn required_proof_kinds(&self) -> BTreeSet<ProductionProofKindV1> {
        let mut required = self.command.required_proof_kinds();
        // Every mutation and every idempotent Receipt replay must carry a
        // current, tenant/scope/purpose-bound caller authentication proof in
        // addition to any command-specific admission/authorization proof.
        required.insert(ProductionProofKindV1::CallerAuthentication);
        required
    }
}

impl IdrCommandV1 {
    fn validate_budgets(&self) -> Result<(), IdrRuntimeErrorV1> {
        match self {
            Self::ConsumeResponseSend { rendered_bytes, .. }
                if rendered_bytes.len() > IDR_MAX_RENDERED_BYTES_V1 =>
            {
                Err(IdrRuntimeErrorV1::InvalidCommand)
            }
            Self::AdmitAction {
                idempotency_key, ..
            }
            | Self::ReserveExecution {
                idempotency_key, ..
            } if idempotency_key.is_empty()
                || idempotency_key.len() > IDR_MAX_IDEMPOTENCY_KEY_BYTES_V1
                || idempotency_key.chars().any(char::is_control) =>
            {
                Err(IdrRuntimeErrorV1::InvalidCommand)
            }
            _ => Ok(()),
        }
    }

    pub(crate) fn candidate(&self) -> Option<&CandidateSubmissionV1> {
        match self {
            Self::StartRun { input } => Some(input),
            Self::RecordContext { context } => Some(context),
            Self::RecordIntent { intent } => Some(intent),
            Self::RecordDecision { decision } => Some(decision),
            Self::RecordTurn { turn } => Some(turn),
            Self::RecordResponse { response } => Some(response),
            Self::RecordAction { action } => Some(action),
            Self::CommitExecutionReceipt { receipt, .. } => Some(receipt),
            Self::RecordOutcome { outcome } => Some(outcome),
            Self::ProposeHumanModelCandidate { candidate } => Some(candidate),
            Self::RecordHumanModelPromotion { decision, .. } => Some(decision),
            Self::PromoteHumanModelAssertion { assertion, .. } => Some(assertion),
            Self::CorrectHumanModelAssertion { correction, .. } => Some(correction),
            _ => None,
        }
    }

    pub(crate) fn expected_candidate_kind(&self) -> Option<CandidateKindV1> {
        match self {
            Self::StartRun { .. } => Some(CandidateKindV1::CanonicalInput),
            Self::RecordContext { .. } => Some(CandidateKindV1::ContextSnapshot),
            Self::RecordIntent { .. } => Some(CandidateKindV1::Intent),
            Self::RecordDecision { .. } => Some(CandidateKindV1::Decision),
            Self::RecordTurn { .. } => Some(CandidateKindV1::TurnCoordination),
            Self::RecordResponse { .. } => Some(CandidateKindV1::Response),
            Self::RecordAction { .. } => Some(CandidateKindV1::Action),
            Self::CommitExecutionReceipt { .. } => Some(CandidateKindV1::ExecutionReceipt),
            Self::RecordOutcome { .. } => Some(CandidateKindV1::Outcome),
            Self::ProposeHumanModelCandidate { .. } => Some(CandidateKindV1::HumanModelCandidate),
            Self::RecordHumanModelPromotion { .. } => {
                Some(CandidateKindV1::HumanModelPromotionDecision)
            }
            Self::PromoteHumanModelAssertion { .. } => Some(CandidateKindV1::HumanModelAssertion),
            Self::CorrectHumanModelAssertion { .. } => Some(CandidateKindV1::HumanModelAssertion),
            _ => None,
        }
    }

    fn required_proof_kinds(&self) -> BTreeSet<ProductionProofKindV1> {
        use ProductionProofKindV1::*;
        match self {
            Self::StartRun { .. } => BTreeSet::from([InputAdmission]),
            Self::RecordContext { .. } => BTreeSet::from([ContextSnapshot]),
            Self::RecordIntent { .. } => BTreeSet::from([IntentFastPathAdmission]),
            Self::RecordDecision { .. } => BTreeSet::from([DecisionNecessityAdmission]),
            Self::RecordTurn { .. } => BTreeSet::from([TurnCoordinationAdmission]),
            Self::RecordResponse { .. } => BTreeSet::new(),
            Self::ConsumeResponseSend { .. } => BTreeSet::from([ResponsePolicy, ResponseAdmission]),
            Self::RecordAction { .. } => BTreeSet::new(),
            Self::AdmitAction { .. } => BTreeSet::from([
                Capability,
                Authority,
                Policy,
                ExactAuthorization,
                ActionAdmission,
            ]),
            Self::ReserveExecution { .. }
            | Self::RecoverPermit { .. }
            | Self::DeliverPermit { .. }
            | Self::StartDispatch { .. }
            | Self::ExpireExecutionLease { .. } => BTreeSet::from([ExecutionPermit]),
            Self::CancelRun { .. } => BTreeSet::from([Authority]),
            Self::CommitExecutionReceipt { .. } => BTreeSet::from([ProviderReceipt]),
            Self::ReconcileExecution { .. } => BTreeSet::from([ProviderReceipt]),
            Self::InvalidateRecord { .. } => BTreeSet::from([Policy]),
            Self::RecordOutcome { .. } => BTreeSet::from([OutcomeObservation]),
            Self::ProposeHumanModelCandidate { .. } => BTreeSet::from([OutcomeObservation]),
            Self::RecordHumanModelPromotion { decision, .. } => {
                let mut required = BTreeSet::from([HumanModelPromotion]);
                if decision.payload().get("outcome").and_then(Value::as_str)
                    == Some("promote_user_confirmed")
                {
                    required.insert(HumanModelUserConfirmation);
                }
                if decision.payload().get("outcome").and_then(Value::as_str)
                    == Some("promote_outcome_supported")
                {
                    required.insert(OutcomeObservation);
                }
                required
            }
            Self::PromoteHumanModelAssertion { .. } => BTreeSet::from([HumanModelPromotion]),
            Self::CorrectHumanModelAssertion { .. } => BTreeSet::from([HumanModelCorrection]),
        }
    }

    fn explicit_digests(&self) -> Vec<&str> {
        match self {
            Self::ConsumeResponseSend { send_nonce, .. } => vec![send_nonce],
            Self::AdmitAction { action_digest, .. } => vec![action_digest],
            Self::ReserveExecution {
                action_digest,
                dispatch_nonce,
                ..
            } => vec![action_digest, dispatch_nonce],
            Self::CommitExecutionReceipt { dispatch_nonce, .. } => vec![dispatch_nonce],
            _ => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExecutionProjectionV1 {
    pub reservation_id: Uuid,
    pub action_admission_ref: AuthoritativeRecordRefV1,
    pub action_ref: AuthoritativeRecordRefV1,
    pub provider_ref: ReferenceV1,
    pub owner_ref: ReferenceV1,
    pub operation_ref: ReferenceV1,
    pub parameter_digest: String,
    pub request_digest: String,
    pub idempotency_key: String,
    pub attempt: u32,
    pub permit_id: Uuid,
    pub exact_authorization_proof_id: Uuid,
    pub action_valid_until: u64,
    pub admission_valid_until: u64,
    pub authorization_valid_until: u64,
    pub permit_valid_until: u64,
    pub lease_until: u64,
    pub dispatch_nonce: String,
    pub state: ExecutionAggregateStateV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResponseBindingProjectionV1 {
    pub response_ref: AuthoritativeRecordRefV1,
    pub rendered_content_digest: String,
    pub channel_ref: ReferenceV1,
    pub audience_ref: ReferenceV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HumanModelPromotionProjectionV1 {
    pub decision_ref: AuthoritativeRecordRefV1,
    pub source_candidate_ref: AuthoritativeRecordRefV1,
    pub outcome_ref: AuthoritativeRecordRefV1,
    pub outcome: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HumanModelCandidateProjectionV1 {
    pub candidate_ref: AuthoritativeRecordRefV1,
    pub outcome_ref: AuthoritativeRecordRefV1,
    pub materialized_payload: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ActionAdmissionProjectionV1 {
    pub admission_ref: AuthoritativeRecordRefV1,
    pub action_ref: AuthoritativeRecordRefV1,
    pub provider_ref: ReferenceV1,
    pub owner_ref: ReferenceV1,
    pub operation_ref: ReferenceV1,
    pub parameter_digest: String,
    pub request_digest: String,
    pub idempotency_key: String,
    pub proof_bindings: BTreeMap<ProductionProofKindV1, ActionAdmissionProofBindingV1>,
    pub exact_authorization_proof_id: Uuid,
    pub authorization_valid_until: u64,
    pub valid_until: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ActionAdmissionProofBindingV1 {
    pub proof_id: Uuid,
    pub valid_until: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DecisionBindingProjectionV1 {
    pub decision_ref: AuthoritativeRecordRefV1,
    pub selected_option_ref: ReferenceV1,
    pub operation_ref: ReferenceV1,
    pub parameter_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ActionBindingProjectionV1 {
    pub action_ref: AuthoritativeRecordRefV1,
    pub selected_option_ref: ReferenceV1,
    pub operation_ref: ReferenceV1,
    pub parameter_digest: String,
    pub request_digest: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IdrRunProjectionV1 {
    pub trust_domain: IdrTrustDomainV1,
    pub environment_ref: ReferenceV1,
    pub run_id: Uuid,
    pub tenant_ref: ReferenceV1,
    pub subject_ref: Option<ReferenceV1>,
    pub turn_id: Option<Uuid>,
    pub aggregate_version: u64,
    pub state: IdrRunStateV1,
    pub records: BTreeMap<CandidateKindV1, AuthoritativeRecordRefV1>,
    pub invalidated_records: BTreeSet<AuthoritativeRecordRefV1>,
    pub response_send_nonces: BTreeSet<String>,
    pub response_binding: Option<ResponseBindingProjectionV1>,
    pub outcome_binding: Option<AuthoritativeRecordRefV1>,
    pub human_model_candidate: Option<HumanModelCandidateProjectionV1>,
    pub human_model_promotion: Option<HumanModelPromotionProjectionV1>,
    pub decision_binding: Option<DecisionBindingProjectionV1>,
    pub action_binding: Option<ActionBindingProjectionV1>,
    pub action_admission: Option<ActionAdmissionProjectionV1>,
    pub execution: Option<ExecutionProjectionV1>,
    pub last_event_sequence: u64,
}

impl IdrRunProjectionV1 {
    pub fn pending(
        trust_domain: IdrTrustDomainV1,
        environment_ref: ReferenceV1,
        run_id: Uuid,
        tenant_ref: ReferenceV1,
    ) -> Self {
        Self {
            trust_domain,
            environment_ref,
            run_id,
            tenant_ref,
            subject_ref: None,
            turn_id: None,
            aggregate_version: 0,
            state: IdrRunStateV1::Pending,
            records: BTreeMap::new(),
            invalidated_records: BTreeSet::new(),
            response_send_nonces: BTreeSet::new(),
            response_binding: None,
            outcome_binding: None,
            human_model_candidate: None,
            human_model_promotion: None,
            decision_binding: None,
            action_binding: None,
            action_admission: None,
            execution: None,
            last_event_sequence: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IdrTransitionV1 {
    projection: IdrRunProjectionV1,
    authoritative_record: Option<AuthoritativeRecordV1>,
    event_type: String,
    event_payload: Value,
    consumed_proof_ids: Vec<Uuid>,
    consumed_nonces: Vec<String>,
}

impl IdrTransitionV1 {
    pub(crate) fn projection(&self) -> &IdrRunProjectionV1 {
        &self.projection
    }

    pub(crate) fn authoritative_record(&self) -> Option<&AuthoritativeRecordV1> {
        self.authoritative_record.as_ref()
    }

    pub(crate) fn event_type(&self) -> &str {
        &self.event_type
    }

    pub(crate) fn event_payload(&self) -> &Value {
        &self.event_payload
    }

    pub(crate) fn consumed_proof_ids(&self) -> &[Uuid] {
        &self.consumed_proof_ids
    }

    pub(crate) fn action_admission_proof_failure(
        _authority: &OrchestratorAuthorityV1,
        mut projection: IdrRunProjectionV1,
        command: &IdrCommandEnvelopeV1,
        verified_proofs: &[VerifiedProductionProofV1],
    ) -> Result<Self, IdrRuntimeErrorV1> {
        let admission_ref = projection
            .action_admission
            .as_ref()
            .map(|admission| admission.admission_ref.clone())
            .ok_or(IdrRuntimeErrorV1::ActionNotAdmitted)?;
        projection.aggregate_version = projection
            .aggregate_version
            .checked_add(1)
            .ok_or(IdrRuntimeErrorV1::Serialization)?;
        projection.last_event_sequence = projection
            .last_event_sequence
            .checked_add(1)
            .ok_or(IdrRuntimeErrorV1::Serialization)?;
        let crossed_dispatch_boundary = projection.execution.as_ref().is_some_and(|execution| {
            matches!(
                execution.state,
                ExecutionAggregateStateV1::DispatchStarted
                    | ExecutionAggregateStateV1::AwaitingProvider
                    | ExecutionAggregateStateV1::ReconciliationRequired
                    | ExecutionAggregateStateV1::Succeeded
                    | ExecutionAggregateStateV1::Failed
                    | ExecutionAggregateStateV1::Rejected
                    | ExecutionAggregateStateV1::Compensated
            )
        });
        if let Some(execution) = &mut projection.execution {
            execution.state = if crossed_dispatch_boundary {
                ExecutionAggregateStateV1::ReconciliationRequired
            } else {
                ExecutionAggregateStateV1::Cancelled
            };
        }
        projection.state = if crossed_dispatch_boundary {
            IdrRunStateV1::ReconciliationRequired
        } else {
            IdrRunStateV1::WaitingAuthorization
        };
        Ok(Self {
            projection,
            authoritative_record: None,
            event_type: if crossed_dispatch_boundary {
                "action_admission_proof_revoked_reconciliation_required".to_string()
            } else {
                "action_admission_proof_revoked_cancelled".to_string()
            },
            event_payload: serde_json::json!({
                "requested_command": command.command(),
                "failed_admission_ref": admission_ref,
                "governance_disposition": if crossed_dispatch_boundary {
                    "reconciliation_required"
                } else {
                    "cancelled"
                },
            }),
            consumed_proof_ids: verified_proofs
                .iter()
                .map(VerifiedProductionProofV1::proof_id)
                .collect(),
            consumed_nonces: verified_proofs
                .iter()
                .map(|proof| proof.nonce().to_string())
                .collect(),
        })
    }

    /// Repository-only invalidation closure hook. The capability token keeps
    /// downstream callers from manufacturing authoritative invalidations.
    pub(crate) fn apply_repository_invalidation_closure(
        &mut self,
        _authority: &OrchestratorAuthorityV1,
        records: impl IntoIterator<
            Item = (
                Uuid,
                u64,
                CandidateKindV1,
                IdrTrustDomainV1,
                ReferenceV1,
                u64,
                u64,
                String,
            ),
        >,
        invalidate_run: bool,
    ) {
        for (
            record_id,
            revision,
            candidate_kind,
            trust_domain,
            environment_ref,
            valid_from,
            valid_until,
            record_digest,
        ) in records
        {
            let record = AuthoritativeRecordRefV1 {
                record_id,
                revision,
                candidate_kind,
                trust_domain,
                environment_ref,
                valid_from,
                valid_until,
                record_digest,
            };
            self.projection.invalidated_records.insert(record.clone());
            match record.candidate_kind() {
                CandidateKindV1::Decision
                    if self
                        .projection
                        .decision_binding
                        .as_ref()
                        .is_some_and(|binding| binding.decision_ref == record) =>
                {
                    self.projection.decision_binding = None;
                }
                CandidateKindV1::Response
                    if self
                        .projection
                        .response_binding
                        .as_ref()
                        .is_some_and(|binding| binding.response_ref == record) =>
                {
                    self.projection.response_binding = None;
                }
                CandidateKindV1::Action => {
                    if self
                        .projection
                        .action_binding
                        .as_ref()
                        .is_some_and(|binding| binding.action_ref == record)
                    {
                        self.projection.action_binding = None;
                    }
                    if self
                        .projection
                        .action_admission
                        .as_ref()
                        .is_some_and(|admission| admission.action_ref == record)
                    {
                        self.projection.action_admission = None;
                    }
                }
                CandidateKindV1::ActionAdmissionDecision
                    if self
                        .projection
                        .action_admission
                        .as_ref()
                        .is_some_and(|admission| admission.admission_ref == record) =>
                {
                    self.projection.action_admission = None;
                }
                CandidateKindV1::Outcome
                    if self.projection.outcome_binding.as_ref() == Some(&record) =>
                {
                    self.projection.outcome_binding = None;
                    if self
                        .projection
                        .human_model_candidate
                        .as_ref()
                        .is_some_and(|candidate| candidate.outcome_ref == record)
                    {
                        self.projection.human_model_candidate = None;
                    }
                    if self
                        .projection
                        .human_model_promotion
                        .as_ref()
                        .is_some_and(|promotion| promotion.outcome_ref == record)
                    {
                        self.projection.human_model_promotion = None;
                    }
                }
                CandidateKindV1::HumanModelCandidate
                    if self
                        .projection
                        .human_model_candidate
                        .as_ref()
                        .is_some_and(|candidate| candidate.candidate_ref == record) =>
                {
                    self.projection.human_model_candidate = None;
                    if self
                        .projection
                        .human_model_promotion
                        .as_ref()
                        .is_some_and(|promotion| promotion.source_candidate_ref == record)
                    {
                        self.projection.human_model_promotion = None;
                    }
                }
                CandidateKindV1::HumanModelPromotionDecision
                    if self
                        .projection
                        .human_model_promotion
                        .as_ref()
                        .is_some_and(|promotion| promotion.decision_ref == record) =>
                {
                    self.projection.human_model_promotion = None;
                }
                _ => {}
            }
            if let Some(execution) = &mut self.projection.execution {
                if execution.action_ref == record && !execution.state.is_terminal() {
                    execution.state = if matches!(
                        execution.state,
                        ExecutionAggregateStateV1::DispatchStarted
                            | ExecutionAggregateStateV1::AwaitingProvider
                            | ExecutionAggregateStateV1::ReconciliationRequired
                    ) {
                        ExecutionAggregateStateV1::ReconciliationRequired
                    } else {
                        ExecutionAggregateStateV1::Cancelled
                    };
                }
            }
        }
        if invalidate_run {
            self.projection.state = IdrRunStateV1::Invalidated;
        } else if self.projection.execution.as_ref().is_some_and(|execution| {
            execution.state == ExecutionAggregateStateV1::ReconciliationRequired
        }) {
            self.projection.state = IdrRunStateV1::ReconciliationRequired;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdrCommandReceiptV1 {
    pub command_id: Uuid,
    pub attestation_id: Uuid,
    pub trust_domain: IdrTrustDomainV1,
    pub environment_ref: ReferenceV1,
    pub run_id: Uuid,
    pub aggregate_version: u64,
    pub event_sequence: u64,
    pub state: IdrRunStateV1,
    pub record_ref: Option<AuthoritativeRecordRefV1>,
    pub idempotent_replay: bool,
}

/// Crate-private capability owned only by the concrete PostgreSQL
/// Orchestrator. It can never be loaned to a downstream Repository.
#[derive(Debug, Clone, Copy)]
pub(crate) struct OrchestratorAuthorityV1 {
    _private: (),
}

impl OrchestratorAuthorityV1 {
    pub(crate) const fn new() -> Self {
        Self { _private: () }
    }
}

/// Pure semantic transition used by the transactional repository after it has
/// loaded the aggregate, DB time and current Trust Root and verified all
/// required proofs. The unforgeable authority token prevents it from becoming
/// an alternate public mutation path.
pub(crate) fn evaluate_authoritative_transition_v1(
    _authority: &OrchestratorAuthorityV1,
    mut projection: IdrRunProjectionV1,
    command: &IdrCommandEnvelopeV1,
    trusted_now: u64,
    verified_proofs: &[VerifiedProductionProofV1],
) -> Result<IdrTransitionV1, IdrRuntimeErrorV1> {
    command.validate()?;
    if projection.run_id != command.run_id()
        || projection.trust_domain != command.trust_domain()
        || projection.environment_ref != *command.environment_ref()
        || projection.tenant_ref != *command.tenant_ref()
        || projection.aggregate_version != command.expected_aggregate_version()
    {
        return Err(IdrRuntimeErrorV1::AggregateVersionConflict);
    }
    require_command_state(projection.state, command.command())?;
    if !matches!(command.command(), IdrCommandV1::StartRun { .. }) {
        if let Some(candidate) = command.command().candidate() {
            if projection.subject_ref.as_ref() != Some(candidate.subject_ref())
                || projection.turn_id != Some(candidate.turn_id())
                || trusted_now < candidate.valid_from()
                || trusted_now >= candidate.valid_until()
            {
                return Err(IdrRuntimeErrorV1::LineageMismatch);
            }
        }
    }
    let required = command.command.required_proof_kinds();
    let found: BTreeSet<_> = verified_proofs
        .iter()
        .filter(|proof| proof.proof_kind() != ProductionProofKindV1::CallerAuthentication)
        .map(VerifiedProductionProofV1::proof_kind)
        .collect();
    #[cfg(not(test))]
    if found != required {
        return Err(IdrRuntimeErrorV1::MissingRequiredProof);
    }
    // Unit tests exercise the private state machine directly. Production builds
    // always execute the exact proof-set comparison above after the PostgreSQL
    // authority has cryptographically verified every proof.
    #[cfg(test)]
    if !verified_proofs.is_empty() && found != required {
        return Err(IdrRuntimeErrorV1::MissingRequiredProof);
    }

    let previous_version = projection.aggregate_version;
    projection.aggregate_version = projection
        .aggregate_version
        .checked_add(1)
        .ok_or(IdrRuntimeErrorV1::AggregateVersionConflict)?;
    projection.last_event_sequence = projection
        .last_event_sequence
        .checked_add(1)
        .ok_or(IdrRuntimeErrorV1::AggregateVersionConflict)?;
    let mut authoritative_record = None;
    let event_type: &str;

    match command.command() {
        IdrCommandV1::StartRun { input } => {
            require_state(projection.state, &[IdrRunStateV1::Pending])?;
            require_no_record(&projection, CandidateKindV1::CanonicalInput)?;
            projection.state = IdrRunStateV1::Running;
            projection.subject_ref = Some(input.subject_ref().clone());
            projection.turn_id = Some(input.turn_id());
            authoritative_record = Some(issue_record(
                input,
                command,
                trusted_now,
                verified_proofs,
                projection.trust_domain,
                record_id_for(&projection, CandidateKindV1::CanonicalInput),
                1,
            )?);
            event_type = "input_admitted";
        }
        IdrCommandV1::RecordContext { context } => {
            require_record_at(&projection, CandidateKindV1::CanonicalInput, trusted_now)?;
            authoritative_record = Some(issue_record(
                context,
                command,
                trusted_now,
                verified_proofs,
                projection.trust_domain,
                record_id_for(&projection, CandidateKindV1::ContextSnapshot),
                next_revision(&projection, CandidateKindV1::ContextSnapshot)?,
            )?);
            event_type = "context_snapshotted";
        }
        IdrCommandV1::RecordIntent { intent } => {
            require_record_at(&projection, CandidateKindV1::ContextSnapshot, trusted_now)?;
            if intent
                .payload()
                .get("fast_path_outcome")
                .and_then(Value::as_str)
                == Some("block")
            {
                projection.state = IdrRunStateV1::Rejected;
                event_type = "intent_blocked";
            } else {
                validate_intent_candidate(intent.payload())?;
                authoritative_record = Some(issue_record(
                    intent,
                    command,
                    trusted_now,
                    verified_proofs,
                    projection.trust_domain,
                    record_id_for(&projection, CandidateKindV1::Intent),
                    next_revision(&projection, CandidateKindV1::Intent)?,
                )?);
                event_type = "intent_admitted";
            }
        }
        IdrCommandV1::RecordDecision { decision } => {
            require_record_at(&projection, CandidateKindV1::Intent, trusted_now)?;
            let (selected_option_ref, operation_ref, parameter_digest) =
                decision_binding_values(decision.payload())?;
            let decision_record = issue_record(
                decision,
                command,
                trusted_now,
                verified_proofs,
                projection.trust_domain,
                record_id_for(&projection, CandidateKindV1::Decision),
                next_revision(&projection, CandidateKindV1::Decision)?,
            )?;
            projection.decision_binding = Some(DecisionBindingProjectionV1 {
                decision_ref: decision_record.record_ref().clone(),
                selected_option_ref,
                operation_ref,
                parameter_digest,
            });
            authoritative_record = Some(decision_record);
            event_type = "decision_admitted";
        }
        IdrCommandV1::RecordTurn { turn } => {
            require_record_at(&projection, CandidateKindV1::Intent, trusted_now)?;
            require_record_at(&projection, CandidateKindV1::Decision, trusted_now)?;
            validate_turn_candidate(turn.payload())?;
            authoritative_record = Some(issue_record(
                turn,
                command,
                trusted_now,
                verified_proofs,
                projection.trust_domain,
                record_id_for(&projection, CandidateKindV1::TurnCoordination),
                next_revision(&projection, CandidateKindV1::TurnCoordination)?,
            )?);
            event_type = "turn_admitted";
        }
        IdrCommandV1::RecordResponse { response } => {
            require_record_at(&projection, CandidateKindV1::TurnCoordination, trusted_now)?;
            let rendered_content_digest =
                required_payload_digest(response.payload(), "rendered_content_digest")?;
            let channel_ref =
                ReferenceV1::new(required_payload_string(response.payload(), "channel_ref")?)?;
            let audience_ref =
                ReferenceV1::new(required_payload_string(response.payload(), "audience_ref")?)?;
            let response_record = issue_record(
                response,
                command,
                trusted_now,
                verified_proofs,
                projection.trust_domain,
                record_id_for(&projection, CandidateKindV1::Response),
                next_revision(&projection, CandidateKindV1::Response)?,
            )?;
            projection.response_binding = Some(ResponseBindingProjectionV1 {
                response_ref: response_record.record_ref().clone(),
                rendered_content_digest,
                channel_ref,
                audience_ref,
            });
            authoritative_record = Some(response_record);
            event_type = "response_recorded";
        }
        IdrCommandV1::ConsumeResponseSend {
            response_ref,
            rendered_bytes,
            channel_ref,
            audience_ref,
            send_nonce,
            ..
        } => {
            require_exact_current_record_at(&projection, response_ref, trusted_now)?;
            let rendered_digest = format!("{:x}", Sha256::digest(rendered_bytes));
            let binding = projection
                .response_binding
                .as_ref()
                .ok_or(IdrRuntimeErrorV1::MissingDependency)?;
            if binding.response_ref != *response_ref
                || binding.rendered_content_digest != rendered_digest
                || binding.channel_ref != *channel_ref
                || binding.audience_ref != *audience_ref
            {
                return Err(IdrRuntimeErrorV1::BindingMismatch);
            }
            if !projection.response_send_nonces.insert(send_nonce.clone()) {
                return Err(IdrRuntimeErrorV1::ProofReplay);
            }
            event_type = "response_send_permit_consumed";
        }
        IdrCommandV1::RecordAction { action } => {
            require_record_at(&projection, CandidateKindV1::TurnCoordination, trusted_now)?;
            require_record_at(&projection, CandidateKindV1::Decision, trusted_now)?;
            let (selected_option_ref, operation_ref, parameter_digest) =
                action_binding_values(action.payload())?;
            let decision = projection
                .decision_binding
                .as_ref()
                .filter(|decision| {
                    projection.records.get(&CandidateKindV1::Decision)
                        == Some(&decision.decision_ref)
                })
                .ok_or(IdrRuntimeErrorV1::MissingDependency)?;
            if decision.selected_option_ref != selected_option_ref
                || decision.operation_ref != operation_ref
                || decision.parameter_digest != parameter_digest
            {
                return Err(IdrRuntimeErrorV1::ActionDerivationMismatch);
            }
            let action_record = issue_record(
                action,
                command,
                trusted_now,
                verified_proofs,
                projection.trust_domain,
                record_id_for(&projection, CandidateKindV1::Action),
                next_revision(&projection, CandidateKindV1::Action)?,
            )?;
            let request_digest = canonical_digest_v1(
                "idr-execution-request-v1",
                &(
                    action_record.record_ref(),
                    &operation_ref,
                    &parameter_digest,
                ),
            )?;
            projection.action_binding = Some(ActionBindingProjectionV1 {
                action_ref: action_record.record_ref().clone(),
                selected_option_ref,
                operation_ref,
                parameter_digest,
                request_digest,
            });
            authoritative_record = Some(action_record);
            if let Some(execution) = &mut projection.execution {
                if !execution.state.is_terminal() {
                    execution.state = if matches!(
                        execution.state,
                        ExecutionAggregateStateV1::DispatchStarted
                            | ExecutionAggregateStateV1::AwaitingProvider
                    ) {
                        ExecutionAggregateStateV1::ReconciliationRequired
                    } else {
                        ExecutionAggregateStateV1::Cancelled
                    };
                }
            }
            projection.action_admission = None;
            event_type = "action_recorded";
        }
        IdrCommandV1::AdmitAction {
            action_ref,
            action_digest,
            provider_ref,
            owner_ref,
            idempotency_key,
        } => {
            require_exact_current_record_at(&projection, action_ref, trusted_now)?;
            let proof_bindings = verified_action_admission_proof_bindings(
                verified_proofs,
                projection.trust_domain,
                action_ref.valid_until(),
            )?;
            let authorization = proof_bindings
                .get(&ProductionProofKindV1::ExactAuthorization)
                .ok_or(IdrRuntimeErrorV1::MissingRequiredProof)?;
            let authorization_proof_id = authorization.proof_id;
            let authorization_valid_until = authorization.valid_until;
            let admission_valid_until = verified_proofs
                .iter()
                .map(VerifiedProductionProofV1::expires_at)
                .chain([action_ref.valid_until(), authorization_valid_until])
                .min()
                .ok_or(IdrRuntimeErrorV1::ExpiredAuthority)?;
            if action_ref.record_digest() != action_digest
                || trusted_now < action_ref.valid_from()
                || trusted_now >= action_ref.valid_until()
                || admission_valid_until <= trusted_now
                || idempotency_key.is_empty()
            {
                return Err(IdrRuntimeErrorV1::BindingMismatch);
            }
            let action_binding = projection
                .action_binding
                .as_ref()
                .filter(|binding| binding.action_ref == *action_ref)
                .ok_or(IdrRuntimeErrorV1::ActionDerivationMismatch)?;
            let admission = issue_internal_record(
                CandidateKindV1::ActionAdmissionDecision,
                serde_json::json!({
                    "action_ref": action_ref,
                    "action_digest": action_digest,
                    "provider_ref": provider_ref,
                    "owner_ref": owner_ref,
                    "operation_ref": action_binding.operation_ref,
                    "parameter_digest": action_binding.parameter_digest,
                    "request_digest": action_binding.request_digest,
                    "idempotency_key": idempotency_key,
                    "outcome": "admit",
                    "action_valid_from": action_ref.valid_from(),
                    "action_valid_until": action_ref.valid_until(),
                    "proof_bindings": &proof_bindings,
                    "exact_authorization_proof_id": authorization_proof_id,
                    "authorization_valid_until": authorization_valid_until,
                    "admission_valid_from": trusted_now,
                    "admission_valid_until": admission_valid_until,
                    "trust_root_versions": verified_proofs
                        .iter()
                        .map(VerifiedProductionProofV1::trust_root_version)
                        .collect::<BTreeSet<_>>(),
                }),
                command,
                &projection,
                trusted_now,
                admission_valid_until,
                verified_proofs,
                record_id_for(&projection, CandidateKindV1::ActionAdmissionDecision),
                next_revision(&projection, CandidateKindV1::ActionAdmissionDecision)?,
            )?;
            projection.action_admission = Some(ActionAdmissionProjectionV1 {
                admission_ref: admission.record_ref().clone(),
                action_ref: action_ref.clone(),
                provider_ref: provider_ref.clone(),
                owner_ref: owner_ref.clone(),
                operation_ref: action_binding.operation_ref.clone(),
                parameter_digest: action_binding.parameter_digest.clone(),
                request_digest: action_binding.request_digest.clone(),
                idempotency_key: idempotency_key.clone(),
                proof_bindings,
                exact_authorization_proof_id: authorization_proof_id,
                authorization_valid_until,
                valid_until: admission_valid_until,
            });
            authoritative_record = Some(admission);
            event_type = "action_admitted";
        }
        IdrCommandV1::ReserveExecution {
            action_admission_ref,
            action_ref,
            action_digest,
            provider_ref,
            owner_ref,
            idempotency_key,
            attempt,
            lease_until,
            dispatch_nonce,
        } => {
            require_exact_current_record_at(&projection, action_ref, trusted_now)?;
            require_exact_current_record_at(&projection, action_admission_ref, trusted_now)?;
            require_caller(command, owner_ref)?;
            let (_, execution_permit_valid_until) = verified_proof_binding(
                verified_proofs,
                ProductionProofKindV1::ExecutionPermit,
                projection.trust_domain,
                *lease_until,
            )?;
            let admission = projection
                .action_admission
                .as_ref()
                .ok_or(IdrRuntimeErrorV1::ActionNotAdmitted)?;
            let permit_valid_until = [
                *lease_until,
                action_ref.valid_until(),
                action_admission_ref.valid_until(),
                admission.authorization_valid_until,
                execution_permit_valid_until,
            ]
            .into_iter()
            .min()
            .ok_or(IdrRuntimeErrorV1::ExpiredAuthority)?;
            if action_admission_ref.candidate_kind() != CandidateKindV1::ActionAdmissionDecision
                || admission.admission_ref != *action_admission_ref
                || admission.action_ref != *action_ref
                || admission.provider_ref != *provider_ref
                || admission.owner_ref != *owner_ref
                || admission.idempotency_key != *idempotency_key
                || admission.valid_until != action_admission_ref.valid_until()
                || action_ref.record_digest() != action_digest
                || trusted_now < action_ref.valid_from()
                || trusted_now < action_admission_ref.valid_from()
                || trusted_now >= action_ref.valid_until()
                || trusted_now >= action_admission_ref.valid_until()
                || trusted_now >= admission.authorization_valid_until
                || *attempt == 0
                || *lease_until <= trusted_now
                || *lease_until > permit_valid_until
                || idempotency_key.is_empty()
            {
                return Err(IdrRuntimeErrorV1::ActionNotAdmitted);
            }
            let reservation_id = if let Some(execution) = &projection.execution {
                if execution.action_ref == *action_ref {
                    if !matches!(
                        execution.state,
                        ExecutionAggregateStateV1::Failed
                            | ExecutionAggregateStateV1::Rejected
                            | ExecutionAggregateStateV1::Expired
                    ) || Some(*attempt) != execution.attempt.checked_add(1)
                        || execution.idempotency_key != *idempotency_key
                    {
                        return Err(IdrRuntimeErrorV1::ExecutionAlreadyReserved);
                    }
                    execution.reservation_id
                } else {
                    if execution.state != ExecutionAggregateStateV1::Cancelled || *attempt != 1 {
                        return Err(IdrRuntimeErrorV1::ExecutionAlreadyReserved);
                    }
                    Uuid::new_v4()
                }
            } else if *attempt != 1 {
                return Err(IdrRuntimeErrorV1::InvalidExecutionTransition);
            } else {
                Uuid::new_v4()
            };
            projection.execution = Some(ExecutionProjectionV1 {
                reservation_id,
                action_admission_ref: action_admission_ref.clone(),
                action_ref: action_ref.clone(),
                provider_ref: provider_ref.clone(),
                owner_ref: owner_ref.clone(),
                operation_ref: projection
                    .action_admission
                    .as_ref()
                    .ok_or(IdrRuntimeErrorV1::ActionNotAdmitted)?
                    .operation_ref
                    .clone(),
                parameter_digest: projection
                    .action_admission
                    .as_ref()
                    .ok_or(IdrRuntimeErrorV1::ActionNotAdmitted)?
                    .parameter_digest
                    .clone(),
                request_digest: projection
                    .action_admission
                    .as_ref()
                    .ok_or(IdrRuntimeErrorV1::ActionNotAdmitted)?
                    .request_digest
                    .clone(),
                idempotency_key: idempotency_key.clone(),
                attempt: *attempt,
                permit_id: Uuid::new_v4(),
                exact_authorization_proof_id: admission.exact_authorization_proof_id,
                action_valid_until: action_ref.valid_until(),
                admission_valid_until: action_admission_ref.valid_until(),
                authorization_valid_until: admission.authorization_valid_until,
                permit_valid_until,
                lease_until: *lease_until,
                dispatch_nonce: dispatch_nonce.clone(),
                state: ExecutionAggregateStateV1::PermitIssued,
            });
            event_type = "execution_permit_issued";
        }
        IdrCommandV1::RecoverPermit { reservation_id } => {
            let execution = projection
                .execution
                .as_mut()
                .filter(|execution| execution.reservation_id == *reservation_id)
                .ok_or(IdrRuntimeErrorV1::InvalidExecutionTransition)?;
            require_caller(command, &execution.owner_ref)?;
            if matches!(
                execution.state,
                ExecutionAggregateStateV1::DispatchStarted
                    | ExecutionAggregateStateV1::AwaitingProvider
                    | ExecutionAggregateStateV1::ReconciliationRequired
            ) {
                return Err(IdrRuntimeErrorV1::ExecutionRequiresReconciliation);
            }
            if !matches!(
                execution.state,
                ExecutionAggregateStateV1::PermitIssued
                    | ExecutionAggregateStateV1::PermitDelivered
            ) {
                return Err(IdrRuntimeErrorV1::InvalidExecutionTransition);
            }
            if execution_authority_expired(execution, trusted_now) {
                execution.state = ExecutionAggregateStateV1::Expired;
                event_type = "execution_lease_expired";
            } else {
                event_type = "execution_permit_recovered";
            }
        }
        IdrCommandV1::DeliverPermit {
            reservation_id,
            permit_id,
        } => {
            require_caller(
                command,
                &current_execution(&mut projection, *reservation_id, *permit_id)?.provider_ref,
            )?;
            let authority_is_current = ensure_current_execution_action(
                &mut projection,
                *reservation_id,
                *permit_id,
                trusted_now,
            )?;
            let execution = current_execution(&mut projection, *reservation_id, *permit_id)?;
            if !authority_is_current {
                event_type = match execution.state {
                    ExecutionAggregateStateV1::Cancelled => "execution_cancelled_invalid_authority",
                    ExecutionAggregateStateV1::ReconciliationRequired => {
                        "execution_reconciliation_required_invalid_authority"
                    }
                    _ => return Err(IdrRuntimeErrorV1::InvalidExecutionTransition),
                };
            } else {
                require_execution_state(execution, ExecutionAggregateStateV1::PermitIssued)?;
                execution.state = ExecutionAggregateStateV1::PermitDelivered;
                event_type = "execution_permit_delivered";
            }
        }
        IdrCommandV1::StartDispatch {
            reservation_id,
            permit_id,
        } => {
            require_caller(
                command,
                &current_execution(&mut projection, *reservation_id, *permit_id)?.provider_ref,
            )?;
            let authority_is_current = ensure_current_execution_action(
                &mut projection,
                *reservation_id,
                *permit_id,
                trusted_now,
            )?;
            let execution = current_execution(&mut projection, *reservation_id, *permit_id)?;
            if !authority_is_current {
                event_type = match execution.state {
                    ExecutionAggregateStateV1::Cancelled => "execution_cancelled_invalid_authority",
                    ExecutionAggregateStateV1::ReconciliationRequired => {
                        "execution_reconciliation_required_invalid_authority"
                    }
                    _ => return Err(IdrRuntimeErrorV1::InvalidExecutionTransition),
                };
            } else {
                require_execution_state(execution, ExecutionAggregateStateV1::PermitDelivered)?;
                execution.state = ExecutionAggregateStateV1::DispatchStarted;
                event_type = "execution_dispatch_started";
            }
        }
        IdrCommandV1::CommitExecutionReceipt {
            receipt,
            reservation_id,
            permit_id,
            provider_ref,
            dispatch_nonce,
            attempt,
        } => {
            let receipt_revision = next_revision(&projection, CandidateKindV1::ExecutionReceipt)?;
            let receipt_record_id = record_id_for(&projection, CandidateKindV1::ExecutionReceipt);
            let trust_domain = projection.trust_domain;
            let action_was_invalidated = projection
                .execution
                .as_ref()
                .filter(|execution| {
                    execution.reservation_id == *reservation_id && execution.permit_id == *permit_id
                })
                .is_some_and(|execution| {
                    projection
                        .invalidated_records
                        .contains(&execution.action_ref)
                });
            let execution = current_execution(&mut projection, *reservation_id, *permit_id)?;
            require_caller(command, &execution.provider_ref)?;
            if !matches!(
                execution.state,
                ExecutionAggregateStateV1::DispatchStarted
                    | ExecutionAggregateStateV1::AwaitingProvider
                    | ExecutionAggregateStateV1::ReconciliationRequired
            ) || execution.provider_ref != *provider_ref
                || execution.dispatch_nonce != *dispatch_nonce
                || execution.attempt != *attempt
            {
                return Err(IdrRuntimeErrorV1::BindingMismatch);
            }
            validate_execution_receipt_candidate(
                receipt.payload(),
                *permit_id,
                provider_ref,
                dispatch_nonce,
                *attempt,
                action_was_invalidated,
                &execution.request_digest,
            )?;
            authoritative_record = Some(issue_record(
                receipt,
                command,
                trusted_now,
                verified_proofs,
                trust_domain,
                receipt_record_id,
                receipt_revision,
            )?);
            execution.state = receipt_terminal_state(receipt.payload())?;
            projection.state = match execution.state {
                ExecutionAggregateStateV1::Succeeded | ExecutionAggregateStateV1::Compensated => {
                    IdrRunStateV1::Succeeded
                }
                ExecutionAggregateStateV1::Failed => IdrRunStateV1::Failed,
                ExecutionAggregateStateV1::Rejected => IdrRunStateV1::Rejected,
                _ => return Err(IdrRuntimeErrorV1::InvalidExecutionTransition),
            };
            event_type = "execution_receipt_committed";
        }
        IdrCommandV1::ReconcileExecution {
            reservation_id,
            resolution,
        } => {
            let execution = projection
                .execution
                .as_mut()
                .filter(|execution| execution.reservation_id == *reservation_id)
                .ok_or(IdrRuntimeErrorV1::InvalidExecutionTransition)?;
            require_caller(command, &execution.provider_ref)?;
            require_execution_state(execution, ExecutionAggregateStateV1::ReconciliationRequired)?;
            if *resolution != ReconciliationResolutionV1::Unknown {
                // A terminal reconciliation is evidence-bearing state and must
                // arrive through CommitExecutionReceipt, bound to the exact
                // permit, provider, dispatch nonce and attempt.
                return Err(IdrRuntimeErrorV1::BindingMismatch);
            }
            execution.state = ExecutionAggregateStateV1::ReconciliationRequired;
            event_type = "execution_reconciliation_still_unknown";
        }
        IdrCommandV1::ExpireExecutionLease { reservation_id } => {
            let execution = projection
                .execution
                .as_mut()
                .filter(|execution| execution.reservation_id == *reservation_id)
                .ok_or(IdrRuntimeErrorV1::InvalidExecutionTransition)?;
            require_caller(command, &execution.owner_ref)?;
            if !execution_authority_expired(execution, trusted_now) {
                return Err(IdrRuntimeErrorV1::InvalidExecutionTransition);
            }
            if matches!(
                execution.state,
                ExecutionAggregateStateV1::PermitIssued
                    | ExecutionAggregateStateV1::PermitDelivered
            ) {
                execution.state = ExecutionAggregateStateV1::Expired;
                event_type = "execution_lease_expired_before_dispatch";
            } else if matches!(
                execution.state,
                ExecutionAggregateStateV1::DispatchStarted
                    | ExecutionAggregateStateV1::AwaitingProvider
            ) {
                execution.state = ExecutionAggregateStateV1::ReconciliationRequired;
                event_type = "execution_lease_expired_after_dispatch";
            } else {
                return Err(IdrRuntimeErrorV1::InvalidExecutionTransition);
            }
        }
        IdrCommandV1::InvalidateRecord { record_ref, .. } => {
            require_exact_governance_target(&projection, record_ref)?;
            projection.invalidated_records.insert(record_ref.clone());
            if let Some(execution) = &mut projection.execution {
                if execution.action_ref.record_id() == record_ref.record_id()
                    && !execution.state.is_terminal()
                {
                    execution.state = if matches!(
                        execution.state,
                        ExecutionAggregateStateV1::DispatchStarted
                            | ExecutionAggregateStateV1::AwaitingProvider
                    ) {
                        ExecutionAggregateStateV1::ReconciliationRequired
                    } else {
                        ExecutionAggregateStateV1::Cancelled
                    };
                }
            }
            projection.state = IdrRunStateV1::Invalidated;
            event_type = "record_invalidated";
        }
        IdrCommandV1::RecordOutcome { outcome } => {
            let receipt_ref = projection
                .records
                .get(&CandidateKindV1::ExecutionReceipt)
                .cloned()
                .ok_or(IdrRuntimeErrorV1::MissingDependency)?;
            require_historical_evidence_record(
                &projection,
                &receipt_ref,
                trusted_now,
                IDR_RECEIPT_OUTCOME_OBSERVATION_WINDOW_SECONDS_V1,
            )?;
            validate_outcome_candidate(outcome.payload(), receipt_ref.record_digest())?;
            let outcome_record = issue_record(
                outcome,
                command,
                trusted_now,
                verified_proofs,
                projection.trust_domain,
                record_id_for(&projection, CandidateKindV1::Outcome),
                next_revision(&projection, CandidateKindV1::Outcome)?,
            )?;
            projection.outcome_binding = Some(outcome_record.record_ref().clone());
            authoritative_record = Some(outcome_record);
            event_type = "outcome_observed";
        }
        IdrCommandV1::ProposeHumanModelCandidate { candidate } => {
            let outcome_ref = projection
                .outcome_binding
                .as_ref()
                .ok_or(IdrRuntimeErrorV1::MissingDependency)?
                .clone();
            require_historical_evidence_record(
                &projection,
                &outcome_ref,
                trusted_now,
                IDR_OUTCOME_HUMAN_MODEL_WINDOW_SECONDS_V1,
            )?;
            require_exact_record_payload(candidate.payload(), "outcome", &outcome_ref)?;
            validate_human_model_candidate(candidate.payload())?;
            let candidate_record = issue_record(
                candidate,
                command,
                trusted_now,
                verified_proofs,
                projection.trust_domain,
                record_id_for(&projection, CandidateKindV1::HumanModelCandidate),
                next_revision(&projection, CandidateKindV1::HumanModelCandidate)?,
            )?;
            projection.human_model_candidate = Some(HumanModelCandidateProjectionV1 {
                candidate_ref: candidate_record.record_ref().clone(),
                outcome_ref,
                materialized_payload: candidate.payload().clone(),
            });
            authoritative_record = Some(candidate_record);
            event_type = "human_model_candidate_recorded";
        }
        IdrCommandV1::RecordHumanModelPromotion {
            decision,
            source_candidate_ref,
        } => {
            require_exact_current_record_at(&projection, source_candidate_ref, trusted_now)?;
            if source_candidate_ref.candidate_kind() != CandidateKindV1::HumanModelCandidate {
                return Err(IdrRuntimeErrorV1::BindingMismatch);
            }
            let source_candidate = projection
                .human_model_candidate
                .as_ref()
                .filter(|candidate| candidate.candidate_ref == *source_candidate_ref)
                .ok_or(IdrRuntimeErrorV1::MissingDependency)?;
            let outcome = validate_human_model_promotion(
                decision.payload(),
                source_candidate_ref.record_digest(),
                &source_candidate.materialized_payload,
            )?;
            require_exact_record_payload(
                decision.payload(),
                "outcome",
                &source_candidate.outcome_ref,
            )?;
            let promotion_record = issue_record(
                decision,
                command,
                trusted_now,
                verified_proofs,
                projection.trust_domain,
                record_id_for(&projection, CandidateKindV1::HumanModelPromotionDecision),
                next_revision(&projection, CandidateKindV1::HumanModelPromotionDecision)?,
            )?;
            projection.human_model_promotion = Some(HumanModelPromotionProjectionV1 {
                decision_ref: promotion_record.record_ref().clone(),
                source_candidate_ref: source_candidate_ref.clone(),
                outcome_ref: source_candidate.outcome_ref.clone(),
                outcome: outcome.to_string(),
            });
            authoritative_record = Some(promotion_record);
            event_type = "human_model_promotion_decided";
        }
        IdrCommandV1::PromoteHumanModelAssertion {
            assertion,
            promotion_decision_ref,
        } => {
            require_exact_current_record_at(&projection, promotion_decision_ref, trusted_now)?;
            if promotion_decision_ref.candidate_kind()
                != CandidateKindV1::HumanModelPromotionDecision
            {
                return Err(IdrRuntimeErrorV1::BindingMismatch);
            }
            let promotion = projection
                .human_model_promotion
                .as_ref()
                .filter(|promotion| {
                    promotion.decision_ref == *promotion_decision_ref
                        && matches!(
                            promotion.outcome.as_str(),
                            "promote_provisional"
                                | "promote_user_confirmed"
                                | "promote_outcome_supported"
                        )
                })
                .ok_or(IdrRuntimeErrorV1::BindingMismatch)?;
            let source_candidate = projection
                .human_model_candidate
                .as_ref()
                .filter(|candidate| candidate.candidate_ref == promotion.source_candidate_ref)
                .ok_or(IdrRuntimeErrorV1::MissingDependency)?;
            validate_human_model_assertion_request(
                assertion.payload(),
                promotion.source_candidate_ref.record_digest(),
            )?;
            let lifecycle_state = match promotion.outcome.as_str() {
                "promote_provisional" => "provisional",
                "promote_user_confirmed" => "user_confirmed",
                "promote_outcome_supported" => "outcome_supported",
                _ => return Err(IdrRuntimeErrorV1::BindingMismatch),
            };
            let maximum_impact_basis_points = source_candidate
                .materialized_payload
                .get("maximum_impact_basis_points")
                .and_then(Value::as_u64)
                .filter(|value| *value <= 10_000)
                .ok_or(IdrRuntimeErrorV1::BindingMismatch)?;
            let mut materialized_payload = source_candidate.materialized_payload.clone();
            let materialized = materialized_payload
                .as_object_mut()
                .ok_or(IdrRuntimeErrorV1::BindingMismatch)?;
            materialized.insert(
                "source_candidate_digest".to_string(),
                Value::String(promotion.source_candidate_ref.record_digest().to_string()),
            );
            materialized.insert(
                "lifecycle_state".to_string(),
                Value::String(lifecycle_state.to_string()),
            );
            materialized.insert(
                "maximum_impact_basis_points".to_string(),
                Value::from(maximum_impact_basis_points),
            );
            materialized.insert(
                "promotion_decision_record_id".to_string(),
                Value::String(promotion.decision_ref.record_id().to_string()),
            );
            materialized.insert(
                "promotion_decision_revision".to_string(),
                Value::from(promotion.decision_ref.revision()),
            );
            materialized.insert(
                "promotion_decision_record_digest".to_string(),
                Value::String(promotion.decision_ref.record_digest().to_string()),
            );
            let assertion_valid_until = verified_proofs
                .iter()
                .map(VerifiedProductionProofV1::expires_at)
                .chain([
                    assertion.valid_until(),
                    promotion.decision_ref.valid_until(),
                    promotion.source_candidate_ref.valid_until(),
                ])
                .min()
                .ok_or(IdrRuntimeErrorV1::ExpiredAuthority)?;
            authoritative_record = Some(issue_internal_record(
                CandidateKindV1::HumanModelAssertion,
                materialized_payload,
                command,
                &projection,
                trusted_now,
                assertion_valid_until,
                verified_proofs,
                record_id_for(&projection, CandidateKindV1::HumanModelAssertion),
                next_revision(&projection, CandidateKindV1::HumanModelAssertion)?,
            )?);
            event_type = "human_model_assertion_promoted";
        }
        IdrCommandV1::CorrectHumanModelAssertion {
            assertion_ref,
            correction,
        } => {
            require_exact_governance_target(&projection, assertion_ref)?;
            validate_human_model_correction(correction.payload())?;
            authoritative_record = Some(issue_record(
                correction,
                command,
                trusted_now,
                verified_proofs,
                projection.trust_domain,
                assertion_ref.record_id(),
                assertion_ref
                    .revision()
                    .checked_add(1)
                    .ok_or(IdrRuntimeErrorV1::AggregateVersionConflict)?,
            )?);
            projection.invalidated_records.insert(assertion_ref.clone());
            event_type = "human_model_assertion_corrected";
        }
        IdrCommandV1::CancelRun { .. } => {
            if matches!(
                projection.state,
                IdrRunStateV1::Succeeded | IdrRunStateV1::Rejected | IdrRunStateV1::Cancelled
            ) {
                return Err(IdrRuntimeErrorV1::InvalidRunTransition);
            }
            projection.state = IdrRunStateV1::Cancelled;
            event_type = "run_cancelled";
        }
    }

    if let Some(record) = &authoritative_record {
        projection.records.insert(
            record.record_ref().candidate_kind(),
            record.record_ref().clone(),
        );
    }
    if projection.aggregate_version != previous_version + 1 {
        return Err(IdrRuntimeErrorV1::AggregateVersionConflict);
    }
    let consumed_proof_ids = verified_proofs
        .iter()
        .map(VerifiedProductionProofV1::proof_id)
        .collect();
    let consumed_nonces = verified_proofs
        .iter()
        .map(|proof| proof.nonce().to_string())
        .collect();
    Ok(IdrTransitionV1 {
        event_payload: serde_json::to_value(command.command())
            .map_err(|_| IdrRuntimeErrorV1::Serialization)?,
        projection,
        authoritative_record,
        event_type: event_type.to_string(),
        consumed_proof_ids,
        consumed_nonces,
    })
}

fn issue_record(
    candidate: &CandidateSubmissionV1,
    command: &IdrCommandEnvelopeV1,
    trusted_now: u64,
    proofs: &[VerifiedProductionProofV1],
    trust_domain: IdrTrustDomainV1,
    record_id: Uuid,
    revision: u64,
) -> Result<AuthoritativeRecordV1, IdrRuntimeErrorV1> {
    candidate.validate()?;
    if trusted_now < candidate.valid_from() || trusted_now >= candidate.valid_until() {
        return Err(IdrRuntimeErrorV1::ExpiredAuthority);
    }
    let candidate_digest = candidate.canonical_digest()?;
    let proof_ids: Vec<_> = proofs
        .iter()
        .map(VerifiedProductionProofV1::proof_id)
        .collect();
    let record_digest = canonical_digest_v1(
        "idr-authoritative-record-v1",
        &AuthoritativeRecordDigestMaterialV1 {
            record_id,
            revision,
            candidate_kind: candidate.candidate_kind(),
            trust_domain,
            environment_ref: command.environment_ref(),
            valid_from: candidate.valid_from(),
            valid_until: candidate.valid_until(),
            run_id: candidate.run_id(),
            turn_id: candidate.turn_id(),
            tenant_ref: candidate.tenant_ref(),
            subject_ref: candidate.subject_ref(),
            policy_revision_ref: candidate.policy_revision_ref(),
            candidate_id: candidate.candidate_id(),
            candidate_digest: &candidate_digest,
            payload: candidate.payload(),
            issued_by_command_id: command.command_id(),
            issued_at: trusted_now,
            proof_ids: &proof_ids,
        },
    )?;
    Ok(AuthoritativeRecordV1 {
        record_ref: AuthoritativeRecordRefV1 {
            record_id,
            revision,
            candidate_kind: candidate.candidate_kind(),
            trust_domain,
            environment_ref: command.environment_ref().clone(),
            valid_from: candidate.valid_from(),
            valid_until: candidate.valid_until(),
            record_digest,
        },
        trust_domain,
        environment_ref: command.environment_ref().clone(),
        run_id: candidate.run_id(),
        turn_id: candidate.turn_id(),
        tenant_ref: candidate.tenant_ref().clone(),
        subject_ref: candidate.subject_ref().clone(),
        policy_revision_ref: candidate.policy_revision_ref().clone(),
        candidate_id: candidate.candidate_id(),
        candidate_digest,
        payload: candidate.payload().clone(),
        issued_by_command_id: command.command_id(),
        issued_at: trusted_now,
        valid_from: candidate.valid_from(),
        valid_until: candidate.valid_until(),
        proof_ids,
    })
}

#[allow(clippy::too_many_arguments)]
fn issue_internal_record(
    candidate_kind: CandidateKindV1,
    payload: Value,
    command: &IdrCommandEnvelopeV1,
    projection: &IdrRunProjectionV1,
    trusted_now: u64,
    valid_until: u64,
    proofs: &[VerifiedProductionProofV1],
    record_id: Uuid,
    revision: u64,
) -> Result<AuthoritativeRecordV1, IdrRuntimeErrorV1> {
    if valid_until <= trusted_now {
        return Err(IdrRuntimeErrorV1::ExpiredAuthority);
    }
    let subject_ref = projection
        .subject_ref
        .clone()
        .ok_or(IdrRuntimeErrorV1::MissingDependency)?;
    let turn_id = projection
        .turn_id
        .ok_or(IdrRuntimeErrorV1::MissingDependency)?;
    let candidate_id = Uuid::new_v4();
    let candidate_digest = canonical_digest_v1(
        "idr-internal-candidate-v1",
        &(
            candidate_id,
            candidate_kind,
            projection.run_id,
            turn_id,
            &subject_ref,
            command.tenant_ref(),
            command.scope_ref(),
            command.purpose_ref(),
            command.policy_revision_ref(),
            &payload,
        ),
    )?;
    let proof_ids: Vec<_> = proofs
        .iter()
        .map(VerifiedProductionProofV1::proof_id)
        .collect();
    let record_digest = canonical_digest_v1(
        "idr-authoritative-record-v1",
        &AuthoritativeRecordDigestMaterialV1 {
            record_id,
            revision,
            candidate_kind,
            trust_domain: projection.trust_domain,
            environment_ref: &projection.environment_ref,
            valid_from: trusted_now,
            valid_until,
            run_id: projection.run_id,
            turn_id,
            tenant_ref: command.tenant_ref(),
            subject_ref: &subject_ref,
            policy_revision_ref: command.policy_revision_ref(),
            candidate_id,
            candidate_digest: &candidate_digest,
            payload: &payload,
            issued_by_command_id: command.command_id(),
            issued_at: trusted_now,
            proof_ids: &proof_ids,
        },
    )?;
    Ok(AuthoritativeRecordV1 {
        record_ref: AuthoritativeRecordRefV1 {
            record_id,
            revision,
            candidate_kind,
            trust_domain: projection.trust_domain,
            environment_ref: projection.environment_ref.clone(),
            valid_from: trusted_now,
            valid_until,
            record_digest,
        },
        trust_domain: projection.trust_domain,
        environment_ref: projection.environment_ref.clone(),
        run_id: projection.run_id,
        turn_id,
        tenant_ref: command.tenant_ref().clone(),
        subject_ref,
        policy_revision_ref: command.policy_revision_ref().clone(),
        candidate_id,
        candidate_digest,
        payload,
        issued_by_command_id: command.command_id(),
        issued_at: trusted_now,
        valid_from: trusted_now,
        valid_until,
        proof_ids,
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredAuthoritativeRecordV1 {
    record_ref: AuthoritativeRecordRefV1,
    trust_domain: IdrTrustDomainV1,
    environment_ref: ReferenceV1,
    run_id: Uuid,
    turn_id: Uuid,
    tenant_ref: ReferenceV1,
    subject_ref: ReferenceV1,
    policy_revision_ref: ReferenceV1,
    candidate_id: Uuid,
    candidate_digest: String,
    payload: Value,
    issued_by_command_id: Uuid,
    issued_at: u64,
    valid_from: u64,
    valid_until: u64,
    proof_ids: Vec<Uuid>,
}

pub(crate) fn verify_authoritative_record_value_v1(
    value: &Value,
) -> Result<AuthoritativeRecordRefV1, IdrRuntimeErrorV1> {
    let stored: StoredAuthoritativeRecordV1 =
        serde_json::from_value(value.clone()).map_err(|_| IdrRuntimeErrorV1::IntegrityViolation)?;
    if stored.record_ref.trust_domain != stored.trust_domain
        || stored.record_ref.environment_ref != stored.environment_ref
        || stored.record_ref.valid_from != stored.valid_from
        || stored.record_ref.valid_until != stored.valid_until
        || stored.valid_from == 0
        || stored.valid_until <= stored.valid_from
        || stored.issued_at < stored.valid_from
        || stored.issued_at >= stored.valid_until
        || !valid_digest(&stored.candidate_digest)
    {
        return Err(IdrRuntimeErrorV1::IntegrityViolation);
    }
    let expected_digest = canonical_digest_v1(
        "idr-authoritative-record-v1",
        &AuthoritativeRecordDigestMaterialV1 {
            record_id: stored.record_ref.record_id,
            revision: stored.record_ref.revision,
            candidate_kind: stored.record_ref.candidate_kind,
            trust_domain: stored.trust_domain,
            environment_ref: &stored.environment_ref,
            valid_from: stored.valid_from,
            valid_until: stored.valid_until,
            run_id: stored.run_id,
            turn_id: stored.turn_id,
            tenant_ref: &stored.tenant_ref,
            subject_ref: &stored.subject_ref,
            policy_revision_ref: &stored.policy_revision_ref,
            candidate_id: stored.candidate_id,
            candidate_digest: &stored.candidate_digest,
            payload: &stored.payload,
            issued_by_command_id: stored.issued_by_command_id,
            issued_at: stored.issued_at,
            proof_ids: &stored.proof_ids,
        },
    )?;
    if expected_digest != stored.record_ref.record_digest {
        return Err(IdrRuntimeErrorV1::IntegrityViolation);
    }
    Ok(stored.record_ref)
}

fn require_state(
    current: IdrRunStateV1,
    allowed: &[IdrRunStateV1],
) -> Result<(), IdrRuntimeErrorV1> {
    if allowed.contains(&current) {
        Ok(())
    } else {
        Err(IdrRuntimeErrorV1::InvalidRunTransition)
    }
}

fn require_caller(
    command: &IdrCommandEnvelopeV1,
    expected_identity: &ReferenceV1,
) -> Result<(), IdrRuntimeErrorV1> {
    if command.caller_ref() == expected_identity {
        Ok(())
    } else {
        Err(IdrRuntimeErrorV1::PrincipalMismatch)
    }
}

fn verified_proof_binding(
    proofs: &[VerifiedProductionProofV1],
    proof_kind: ProductionProofKindV1,
    trust_domain: IdrTrustDomainV1,
    _test_valid_until: u64,
) -> Result<(Uuid, u64), IdrRuntimeErrorV1> {
    if let Some(proof) = proofs
        .iter()
        .find(|proof| proof.proof_kind() == proof_kind && proof.trust_domain() == trust_domain)
    {
        return Ok((proof.proof_id(), proof.expires_at()));
    }
    #[cfg(test)]
    if proofs.is_empty() {
        return Ok((Uuid::nil(), _test_valid_until));
    }
    Err(IdrRuntimeErrorV1::MissingRequiredProof)
}

pub(crate) const ACTION_ADMISSION_PROOF_KINDS_V1: [ProductionProofKindV1; 5] = [
    ProductionProofKindV1::Capability,
    ProductionProofKindV1::Authority,
    ProductionProofKindV1::Policy,
    ProductionProofKindV1::ExactAuthorization,
    ProductionProofKindV1::ActionAdmission,
];

fn verified_action_admission_proof_bindings(
    proofs: &[VerifiedProductionProofV1],
    trust_domain: IdrTrustDomainV1,
    test_valid_until: u64,
) -> Result<BTreeMap<ProductionProofKindV1, ActionAdmissionProofBindingV1>, IdrRuntimeErrorV1> {
    ACTION_ADMISSION_PROOF_KINDS_V1
        .into_iter()
        .map(|proof_kind| {
            let (proof_id, valid_until) =
                verified_proof_binding(proofs, proof_kind, trust_domain, test_valid_until)?;
            Ok((
                proof_kind,
                ActionAdmissionProofBindingV1 {
                    proof_id,
                    valid_until,
                },
            ))
        })
        .collect()
}

fn require_command_state(
    current: IdrRunStateV1,
    command: &IdrCommandV1,
) -> Result<(), IdrRuntimeErrorV1> {
    use IdrCommandV1::*;
    use IdrRunStateV1::*;
    let allowed = match command {
        StartRun { .. } => matches!(current, Pending),
        CommitExecutionReceipt { .. } | ReconcileExecution { .. } | ExpireExecutionLease { .. } => {
            matches!(current, Running | ReconciliationRequired | Invalidated)
        }
        RecordOutcome { .. } => matches!(
            current,
            Running | ReconciliationRequired | Invalidated | Succeeded | Rejected | Failed
        ),
        ProposeHumanModelCandidate { .. }
        | RecordHumanModelPromotion { .. }
        | PromoteHumanModelAssertion { .. }
        | CorrectHumanModelAssertion { .. } => matches!(
            current,
            Running | ReconciliationRequired | Invalidated | Succeeded | Rejected | Failed
        ),
        CancelRun { .. } => matches!(
            current,
            Running
                | WaitingInput
                | WaitingAuthorization
                | WaitingDependency
                | ReconciliationRequired
                | Invalidated
        ),
        _ => matches!(
            current,
            Running | WaitingInput | WaitingAuthorization | WaitingDependency
        ),
    };
    if allowed {
        Ok(())
    } else {
        Err(IdrRuntimeErrorV1::InvalidRunTransition)
    }
}

fn require_record_at(
    projection: &IdrRunProjectionV1,
    kind: CandidateKindV1,
    trusted_now: u64,
) -> Result<(), IdrRuntimeErrorV1> {
    projection
        .records
        .get(&kind)
        .filter(|record| {
            record.valid_from() <= trusted_now
                && trusted_now < record.valid_until()
                && !projection.invalidated_records.contains(*record)
        })
        .map(|_| ())
        .ok_or(IdrRuntimeErrorV1::MissingDependency)
}

fn require_no_record(
    projection: &IdrRunProjectionV1,
    kind: CandidateKindV1,
) -> Result<(), IdrRuntimeErrorV1> {
    if projection.records.contains_key(&kind) {
        Err(IdrRuntimeErrorV1::InvalidRunTransition)
    } else {
        Ok(())
    }
}

fn require_exact_current_record_at(
    projection: &IdrRunProjectionV1,
    expected: &AuthoritativeRecordRefV1,
    trusted_now: u64,
) -> Result<(), IdrRuntimeErrorV1> {
    if projection.records.get(&expected.candidate_kind()) == Some(expected)
        && expected.trust_domain() == projection.trust_domain
        && expected.valid_from() <= trusted_now
        && trusted_now < expected.valid_until()
        && !projection.invalidated_records.contains(expected)
    {
        Ok(())
    } else {
        Err(IdrRuntimeErrorV1::MissingDependency)
    }
}

/// Historical evidence is not live behavioral authority. It may be consumed
/// only by an exact, purpose-specific transition during a bounded observation
/// window, and never after invalidation or replacement.
fn require_historical_evidence_record(
    projection: &IdrRunProjectionV1,
    expected: &AuthoritativeRecordRefV1,
    trusted_now: u64,
    maximum_delay_seconds: u64,
) -> Result<(), IdrRuntimeErrorV1> {
    let observation_deadline = expected
        .valid_until()
        .checked_add(maximum_delay_seconds)
        .ok_or(IdrRuntimeErrorV1::ExpiredAuthority)?;
    if projection.records.get(&expected.candidate_kind()) == Some(expected)
        && expected.trust_domain() == projection.trust_domain
        && expected.valid_from() <= trusted_now
        && trusted_now < observation_deadline
        && !projection.invalidated_records.contains(expected)
    {
        Ok(())
    } else {
        Err(IdrRuntimeErrorV1::MissingDependency)
    }
}

/// Governance commands may invalidate or correct an exact historical record
/// after its behavioral validity window has elapsed. They still require the
/// exact current identity and explicit proof, but deliberately do not consume
/// the record as live behavioral authority.
fn require_exact_governance_target(
    projection: &IdrRunProjectionV1,
    expected: &AuthoritativeRecordRefV1,
) -> Result<(), IdrRuntimeErrorV1> {
    if projection.records.get(&expected.candidate_kind()) == Some(expected)
        && expected.trust_domain() == projection.trust_domain
        && !projection.invalidated_records.contains(expected)
    {
        Ok(())
    } else {
        Err(IdrRuntimeErrorV1::MissingDependency)
    }
}

fn next_revision(
    projection: &IdrRunProjectionV1,
    kind: CandidateKindV1,
) -> Result<u64, IdrRuntimeErrorV1> {
    projection.records.get(&kind).map_or(Ok(1), |record| {
        record
            .revision()
            .checked_add(1)
            .ok_or(IdrRuntimeErrorV1::AggregateVersionConflict)
    })
}

fn record_id_for(projection: &IdrRunProjectionV1, kind: CandidateKindV1) -> Uuid {
    projection
        .records
        .get(&kind)
        .map_or_else(Uuid::new_v4, AuthoritativeRecordRefV1::record_id)
}

fn current_execution(
    projection: &mut IdrRunProjectionV1,
    reservation_id: Uuid,
    permit_id: Uuid,
) -> Result<&mut ExecutionProjectionV1, IdrRuntimeErrorV1> {
    projection
        .execution
        .as_mut()
        .filter(|execution| {
            execution.reservation_id == reservation_id && execution.permit_id == permit_id
        })
        .ok_or(IdrRuntimeErrorV1::InvalidExecutionTransition)
}

fn ensure_current_execution_action(
    projection: &mut IdrRunProjectionV1,
    reservation_id: Uuid,
    permit_id: Uuid,
    trusted_now: u64,
) -> Result<bool, IdrRuntimeErrorV1> {
    let action_ref = projection
        .execution
        .as_ref()
        .filter(|execution| {
            execution.reservation_id == reservation_id && execution.permit_id == permit_id
        })
        .map(|execution| execution.action_ref.clone())
        .ok_or(IdrRuntimeErrorV1::InvalidExecutionTransition)?;
    let authority_is_current = projection.records.get(&CandidateKindV1::Action)
        == Some(&action_ref)
        && !projection.invalidated_records.contains(&action_ref)
        && projection
            .execution
            .as_ref()
            .is_some_and(|execution| !execution_authority_expired(execution, trusted_now));
    if !authority_is_current {
        let execution = projection
            .execution
            .as_mut()
            .ok_or(IdrRuntimeErrorV1::InvalidExecutionTransition)?;
        execution.state = if matches!(
            execution.state,
            ExecutionAggregateStateV1::DispatchStarted
                | ExecutionAggregateStateV1::AwaitingProvider
        ) {
            ExecutionAggregateStateV1::ReconciliationRequired
        } else {
            ExecutionAggregateStateV1::Cancelled
        };
        return Ok(false);
    }
    Ok(true)
}

fn execution_authority_expired(execution: &ExecutionProjectionV1, trusted_now: u64) -> bool {
    trusted_now >= execution.lease_until
        || trusted_now >= execution.action_valid_until
        || trusted_now >= execution.admission_valid_until
        || trusted_now >= execution.authorization_valid_until
        || trusted_now >= execution.permit_valid_until
}

fn require_execution_state(
    execution: &ExecutionProjectionV1,
    expected: ExecutionAggregateStateV1,
) -> Result<(), IdrRuntimeErrorV1> {
    if execution.state == expected {
        Ok(())
    } else {
        Err(IdrRuntimeErrorV1::InvalidExecutionTransition)
    }
}

fn receipt_terminal_state(payload: &Value) -> Result<ExecutionAggregateStateV1, IdrRuntimeErrorV1> {
    match payload.get("state").and_then(Value::as_str) {
        Some("succeeded") => Ok(ExecutionAggregateStateV1::Succeeded),
        Some("failed") => Ok(ExecutionAggregateStateV1::Failed),
        Some("rejected") => Ok(ExecutionAggregateStateV1::Rejected),
        Some("compensated") => Ok(ExecutionAggregateStateV1::Compensated),
        _ => Err(IdrRuntimeErrorV1::BindingMismatch),
    }
}

fn required_payload_string<'a>(
    payload: &'a Value,
    field: &str,
) -> Result<&'a str, IdrRuntimeErrorV1> {
    payload
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(IdrRuntimeErrorV1::BindingMismatch)
}

fn required_payload_digest(payload: &Value, field: &str) -> Result<String, IdrRuntimeErrorV1> {
    let value = required_payload_string(payload, field)?;
    if valid_digest(value) {
        Ok(value.to_string())
    } else {
        Err(IdrRuntimeErrorV1::BindingMismatch)
    }
}

fn validate_intent_candidate(payload: &Value) -> Result<(), IdrRuntimeErrorV1> {
    let outcome = required_payload_string(payload, "fast_path_outcome")?;
    let resolution = required_payload_string(payload, "resolution_method")?;
    if matches!(
        (outcome, resolution),
        ("allow_fast_path", "deterministic_fast_path") | ("unknown", "full_path")
    ) {
        Ok(())
    } else {
        Err(IdrRuntimeErrorV1::BindingMismatch)
    }
}

fn decision_binding_values(
    payload: &Value,
) -> Result<(ReferenceV1, ReferenceV1, String), IdrRuntimeErrorV1> {
    let necessity = required_payload_string(payload, "necessity_outcome")?;
    if !matches!(necessity, "required" | "unknown") {
        return Err(IdrRuntimeErrorV1::BindingMismatch);
    }
    let selected_option =
        ReferenceV1::new(required_payload_string(payload, "selected_option_ref")?)?;
    let operation = ReferenceV1::new(required_payload_string(
        payload,
        "selected_action_operation_ref",
    )?)?;
    let parameter_digest = required_payload_digest(payload, "selected_action_parameter_digest")?;
    Ok((selected_option, operation, parameter_digest))
}

fn action_binding_values(
    payload: &Value,
) -> Result<(ReferenceV1, ReferenceV1, String), IdrRuntimeErrorV1> {
    let selected_option =
        ReferenceV1::new(required_payload_string(payload, "selected_option_ref")?)?;
    let operation = ReferenceV1::new(required_payload_string(payload, "operation_ref")?)?;
    let parameter_digest = required_payload_digest(payload, "parameter_digest")?;
    Ok((selected_option, operation, parameter_digest))
}

fn validate_turn_candidate(payload: &Value) -> Result<(), IdrRuntimeErrorV1> {
    let mode = required_payload_string(payload, "mode")?;
    let nodes = payload
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or(IdrRuntimeErrorV1::BindingMismatch)?;
    let node_names: BTreeSet<_> = nodes
        .iter()
        .map(|node| {
            node.as_str()
                .filter(|node| !node.is_empty())
                .ok_or(IdrRuntimeErrorV1::BindingMismatch)
        })
        .collect::<Result<_, _>>()?;
    if node_names.len() != nodes.len() || node_names.is_empty() {
        return Err(IdrRuntimeErrorV1::BindingMismatch);
    }
    let edges = payload
        .get("edges")
        .and_then(Value::as_array)
        .ok_or(IdrRuntimeErrorV1::BindingMismatch)?;
    let mut adjacency: BTreeMap<&str, Vec<&str>> = node_names
        .iter()
        .copied()
        .map(|node| (node, Vec::new()))
        .collect();
    for edge in edges {
        let edge = edge
            .as_array()
            .filter(|edge| edge.len() == 2)
            .ok_or(IdrRuntimeErrorV1::BindingMismatch)?;
        let from = edge[0].as_str().ok_or(IdrRuntimeErrorV1::BindingMismatch)?;
        let to = edge[1].as_str().ok_or(IdrRuntimeErrorV1::BindingMismatch)?;
        if from == to || !node_names.contains(from) || !node_names.contains(to) {
            return Err(IdrRuntimeErrorV1::BindingMismatch);
        }
        adjacency
            .get_mut(from)
            .ok_or(IdrRuntimeErrorV1::BindingMismatch)?
            .push(to);
    }
    fn visit<'a>(
        node: &'a str,
        adjacency: &BTreeMap<&'a str, Vec<&'a str>>,
        visiting: &mut BTreeSet<&'a str>,
        visited: &mut BTreeSet<&'a str>,
    ) -> bool {
        if visited.contains(node) {
            return true;
        }
        if !visiting.insert(node) {
            return false;
        }
        if adjacency[node]
            .iter()
            .any(|child| !visit(child, adjacency, visiting, visited))
        {
            return false;
        }
        visiting.remove(node);
        visited.insert(node);
        true
    }
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    if node_names
        .iter()
        .any(|node| !visit(node, &adjacency, &mut visiting, &mut visited))
    {
        return Err(IdrRuntimeErrorV1::BindingMismatch);
    }
    let expected_mode = match (
        node_names.contains("response"),
        node_names.contains("action"),
        edges.as_slice(),
    ) {
        (true, false, _) => "respond_only",
        (true, true, [edge])
            if edge.as_array().is_some_and(|edge| {
                edge == &vec![Value::from("action"), Value::from("response")]
            }) =>
        {
            "act_then_respond"
        }
        (true, true, [edge])
            if edge.as_array().is_some_and(|edge| {
                edge == &vec![Value::from("response"), Value::from("action")]
            }) =>
        {
            "respond_then_act"
        }
        _ => return Err(IdrRuntimeErrorV1::BindingMismatch),
    };
    if mode == expected_mode {
        Ok(())
    } else {
        Err(IdrRuntimeErrorV1::BindingMismatch)
    }
}

fn validate_human_model_candidate(payload: &Value) -> Result<(), IdrRuntimeErrorV1> {
    let predicate = required_payload_string(payload, "predicate")?;
    let scope = required_payload_string(payload, "scope_ref")?;
    required_payload_digest(payload, "value_digest")?;
    let evidence = payload
        .get("evidence_refs")
        .and_then(Value::as_array)
        .ok_or(IdrRuntimeErrorV1::BindingMismatch)?;
    let evidence: BTreeSet<_> = evidence
        .iter()
        .map(|reference| {
            reference
                .as_str()
                .filter(|reference| ReferenceV1::new(*reference).is_ok())
                .ok_or(IdrRuntimeErrorV1::BindingMismatch)
        })
        .collect::<Result<_, _>>()?;
    let usages = payload
        .get("allowed_purposes")
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty())
        .ok_or(IdrRuntimeErrorV1::BindingMismatch)?;
    let maximum_impact = payload
        .get("maximum_impact_basis_points")
        .and_then(Value::as_u64)
        .filter(|value| *value <= 10_000)
        .ok_or(IdrRuntimeErrorV1::BindingMismatch)?;
    let _ = maximum_impact;
    if predicate.is_empty()
        || ReferenceV1::new(scope).is_err()
        || evidence.is_empty()
        || usages.iter().any(|usage| {
            usage
                .as_str()
                .is_none_or(|usage| ReferenceV1::new(usage).is_err())
        })
    {
        return Err(IdrRuntimeErrorV1::BindingMismatch);
    }
    Ok(())
}

fn require_exact_record_payload(
    payload: &Value,
    prefix: &str,
    expected: &AuthoritativeRecordRefV1,
) -> Result<(), IdrRuntimeErrorV1> {
    let record_id = required_payload_string(payload, &format!("{prefix}_record_id"))?
        .parse::<Uuid>()
        .map_err(|_| IdrRuntimeErrorV1::BindingMismatch)?;
    let revision = payload
        .get(format!("{prefix}_revision"))
        .and_then(Value::as_u64)
        .ok_or(IdrRuntimeErrorV1::BindingMismatch)?;
    let digest = required_payload_digest(payload, &format!("{prefix}_record_digest"))?;
    if record_id == expected.record_id()
        && revision == expected.revision()
        && digest == expected.record_digest()
    {
        Ok(())
    } else {
        Err(IdrRuntimeErrorV1::BindingMismatch)
    }
}

fn validate_execution_receipt_candidate(
    payload: &Value,
    permit_id: Uuid,
    provider_ref: &ReferenceV1,
    dispatch_nonce: &str,
    attempt: u32,
    action_was_invalidated: bool,
    expected_request_digest: &str,
) -> Result<(), IdrRuntimeErrorV1> {
    let payload_permit = required_payload_string(payload, "permit_id")?;
    let payload_provider = required_payload_string(payload, "provider_ref")?;
    let payload_dispatch_nonce = required_payload_string(payload, "dispatch_nonce")?;
    let payload_attempt = payload
        .get("attempt")
        .and_then(Value::as_u64)
        .and_then(|attempt| u32::try_from(attempt).ok())
        .ok_or(IdrRuntimeErrorV1::BindingMismatch)?;
    let payload_invalidated = payload
        .get("action_was_invalidated")
        .and_then(Value::as_bool)
        .ok_or(IdrRuntimeErrorV1::BindingMismatch)?;
    let request_digest = required_payload_digest(payload, "request_digest")?;
    required_payload_digest(payload, "result_digest")?;
    receipt_terminal_state(payload)?;
    if payload_permit == permit_id.to_string()
        && payload_provider == provider_ref.as_str()
        && payload_dispatch_nonce == dispatch_nonce
        && payload_attempt == attempt
        && payload_invalidated == action_was_invalidated
        && request_digest == expected_request_digest
    {
        Ok(())
    } else {
        Err(IdrRuntimeErrorV1::BindingMismatch)
    }
}

fn validate_outcome_candidate(
    payload: &Value,
    receipt_record_digest: &str,
) -> Result<(), IdrRuntimeErrorV1> {
    if required_payload_string(payload, "receipt_record_digest")? != receipt_record_digest {
        return Err(IdrRuntimeErrorV1::BindingMismatch);
    }
    required_payload_digest(payload, "observed_value_digest")?;
    required_payload_string(payload, "observation")?;
    Ok(())
}

fn validate_human_model_promotion<'a>(
    payload: &'a Value,
    candidate_digest: &str,
    source_candidate: &Value,
) -> Result<&'a str, IdrRuntimeErrorV1> {
    if required_payload_string(payload, "candidate_digest")? != candidate_digest {
        return Err(IdrRuntimeErrorV1::BindingMismatch);
    }
    let outcome = required_payload_string(payload, "outcome")?;
    let claimed_evidence_count = payload
        .get("independent_evidence_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let evidence_roots: BTreeSet<_> = source_candidate
        .get("evidence_refs")
        .and_then(Value::as_array)
        .ok_or(IdrRuntimeErrorV1::BindingMismatch)?
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|reference| ReferenceV1::new(*reference).is_ok())
                .ok_or(IdrRuntimeErrorV1::BindingMismatch)
        })
        .collect::<Result<_, _>>()?;
    let independent_evidence_count =
        u64::try_from(evidence_roots.len()).map_err(|_| IdrRuntimeErrorV1::BindingMismatch)?;
    if claimed_evidence_count != independent_evidence_count {
        return Err(IdrRuntimeErrorV1::BindingMismatch);
    }
    let valid = match outcome {
        "keep_session" | "store_candidate" | "reject" => true,
        "promote_provisional" => independent_evidence_count >= 2,
        "promote_user_confirmed" | "promote_outcome_supported" => true,
        _ => false,
    };
    if valid {
        Ok(outcome)
    } else {
        Err(IdrRuntimeErrorV1::BindingMismatch)
    }
}

fn validate_human_model_assertion_request(
    payload: &Value,
    source_candidate_digest: &str,
) -> Result<(), IdrRuntimeErrorV1> {
    let object = payload
        .as_object()
        .ok_or(IdrRuntimeErrorV1::BindingMismatch)?;
    if object.len() != 1 || !object.contains_key("source_candidate_digest") {
        return Err(IdrRuntimeErrorV1::BindingMismatch);
    }
    if required_payload_string(payload, "source_candidate_digest")? != source_candidate_digest {
        return Err(IdrRuntimeErrorV1::BindingMismatch);
    }
    Ok(())
}

fn validate_human_model_correction(payload: &Value) -> Result<(), IdrRuntimeErrorV1> {
    if matches!(
        required_payload_string(payload, "lifecycle_state")?,
        "corrected" | "rejected" | "deleted"
    ) {
        validate_human_model_candidate(payload)
    } else {
        Err(IdrRuntimeErrorV1::BindingMismatch)
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(crate) fn hash_event_v1(
    previous_hash: &str,
    event: &Value,
) -> Result<String, IdrRuntimeErrorV1> {
    let bytes = idr_protocol::production::canonical_json_bytes_v1(&(
        "idr-audit-event-v1",
        previous_hash,
        event,
    ))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

#[derive(Debug, thiserror::Error)]
pub enum IdrRuntimeErrorV1 {
    #[error("invalid Orchestrator command")]
    InvalidCommand,
    #[error("aggregate version compare-and-swap failed")]
    AggregateVersionConflict,
    #[error("the command ID is already bound to different canonical command bytes")]
    IdempotencyConflict,
    #[error("authoritative projection or audit integrity verification failed")]
    IntegrityViolation,
    #[error("required proof is missing")]
    MissingRequiredProof,
    #[error("proof ID or nonce replay detected")]
    ProofReplay,
    #[error("a required authoritative dependency is missing or invalidated")]
    MissingDependency,
    #[error("cross-object proof or record binding mismatch")]
    BindingMismatch,
    #[error("Action has not passed the proof-only Admission boundary")]
    ActionNotAdmitted,
    #[error("Action does not exactly derive from the selected Decision option")]
    ActionDerivationMismatch,
    #[error("execution scope is already reserved")]
    ExecutionAlreadyReserved,
    #[error("execution transition is invalid")]
    InvalidExecutionTransition,
    #[error("execution lease expired")]
    ExecutionLeaseExpired,
    #[error("authoritative contract or authorization validity window expired")]
    ExpiredAuthority,
    #[error("execution has crossed the dispatch boundary and requires reconciliation")]
    ExecutionRequiresReconciliation,
    #[error("bound Action was invalidated")]
    ActionInvalidated,
    #[error("run transition is invalid")]
    InvalidRunTransition,
    #[error("candidate subject or turn does not match the run lineage")]
    LineageMismatch,
    #[error("authenticated caller is not the bound owner or provider")]
    PrincipalMismatch,
    #[error("serialization failed")]
    Serialization,
    #[error("repository failure: {0}")]
    Repository(String),
    #[error(transparent)]
    Protocol(#[from] idr_protocol::production::ProductionProtocolErrorV1),
    #[error(transparent)]
    InvalidReference(#[from] idr_protocol::ReferenceValidationErrorV1),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference(value: &str) -> ReferenceV1 {
        ReferenceV1::new(value).unwrap()
    }

    fn record_ref(
        kind: CandidateKindV1,
        id: Uuid,
        revision: u64,
        digest_byte: char,
    ) -> AuthoritativeRecordRefV1 {
        AuthoritativeRecordRefV1 {
            record_id: id,
            revision,
            candidate_kind: kind,
            trust_domain: IdrTrustDomainV1::Shadow,
            environment_ref: reference("environment:idr:shadow:test"),
            valid_from: 1,
            valid_until: 10_000,
            record_digest: digest_byte.to_string().repeat(64),
        }
    }

    fn command(run_id: Uuid, expected_version: u64, command: IdrCommandV1) -> IdrCommandEnvelopeV1 {
        IdrCommandEnvelopeV1::new(
            IdrTrustDomainV1::Shadow,
            reference("environment:idr:shadow:test"),
            run_id,
            expected_version,
            reference("actor:test"),
            reference("caller:test"),
            reference("tenant:test"),
            reference("scope:test"),
            reference("purpose:test"),
            reference("policy:test:v1"),
            reference("correlation:test"),
            reference("causation:test"),
            command,
            vec![],
        )
        .unwrap()
    }

    fn admission_proof_bindings(
        valid_until: u64,
    ) -> BTreeMap<ProductionProofKindV1, ActionAdmissionProofBindingV1> {
        ACTION_ADMISSION_PROOF_KINDS_V1
            .into_iter()
            .map(|proof_kind| {
                (
                    proof_kind,
                    ActionAdmissionProofBindingV1 {
                        proof_id: Uuid::new_v4(),
                        valid_until,
                    },
                )
            })
            .collect()
    }

    fn candidate(
        kind: CandidateKindV1,
        run_id: Uuid,
        turn_id: Uuid,
        payload: Value,
    ) -> CandidateSubmissionV1 {
        CandidateSubmissionV1::new(
            kind,
            run_id,
            turn_id,
            reference("subject:test"),
            reference("tenant:test"),
            reference("scope:test"),
            reference("purpose:test"),
            reference("policy:test:v1"),
            101,
            500,
            payload,
        )
        .unwrap()
    }

    fn running_projection(run_id: Uuid, turn_id: Uuid) -> IdrRunProjectionV1 {
        let mut projection = IdrRunProjectionV1::pending(
            IdrTrustDomainV1::Shadow,
            reference("environment:idr:shadow:test"),
            run_id,
            reference("tenant:test"),
        );
        projection.subject_ref = Some(reference("subject:test"));
        projection.turn_id = Some(turn_id);
        projection.aggregate_version = 1;
        projection.last_event_sequence = 1;
        projection.state = IdrRunStateV1::Running;
        projection
    }

    fn execution_projection(
        state: ExecutionAggregateStateV1,
        lease_until: u64,
    ) -> (
        IdrRunProjectionV1,
        AuthoritativeRecordRefV1,
        AuthoritativeRecordRefV1,
        Uuid,
        Uuid,
    ) {
        let run_id = Uuid::new_v4();
        let action_ref = record_ref(CandidateKindV1::Action, Uuid::new_v4(), 1, 'a');
        let admission_ref = record_ref(
            CandidateKindV1::ActionAdmissionDecision,
            Uuid::new_v4(),
            1,
            'b',
        );
        let reservation_id = Uuid::new_v4();
        let permit_id = Uuid::new_v4();
        let mut projection = IdrRunProjectionV1::pending(
            IdrTrustDomainV1::Shadow,
            reference("environment:idr:shadow:test"),
            run_id,
            reference("tenant:test"),
        );
        projection.subject_ref = Some(reference("subject:test"));
        projection.turn_id = Some(Uuid::new_v4());
        projection.aggregate_version = 10;
        projection.last_event_sequence = 10;
        projection.state = IdrRunStateV1::Running;
        projection
            .records
            .insert(CandidateKindV1::Action, action_ref.clone());
        projection.records.insert(
            CandidateKindV1::ActionAdmissionDecision,
            admission_ref.clone(),
        );
        projection.action_admission = Some(ActionAdmissionProjectionV1 {
            admission_ref: admission_ref.clone(),
            action_ref: action_ref.clone(),
            provider_ref: reference("provider:test"),
            owner_ref: reference("caller:test"),
            operation_ref: reference("operation:test"),
            parameter_digest: "d".repeat(64),
            request_digest: "e".repeat(64),
            idempotency_key: "idempotency:test".to_string(),
            proof_bindings: admission_proof_bindings(10_000),
            exact_authorization_proof_id: Uuid::new_v4(),
            authorization_valid_until: 10_000,
            valid_until: 10_000,
        });
        projection.execution = Some(ExecutionProjectionV1 {
            reservation_id,
            action_admission_ref: admission_ref.clone(),
            action_ref: action_ref.clone(),
            provider_ref: reference("provider:test"),
            owner_ref: reference("caller:test"),
            operation_ref: reference("operation:test"),
            parameter_digest: "d".repeat(64),
            request_digest: "e".repeat(64),
            idempotency_key: "idempotency:test".to_string(),
            attempt: 1,
            permit_id,
            exact_authorization_proof_id: Uuid::new_v4(),
            action_valid_until: 10_000,
            admission_valid_until: 10_000,
            authorization_valid_until: 10_000,
            permit_valid_until: 10_000,
            lease_until,
            dispatch_nonce: "c".repeat(64),
            state,
        });
        (
            projection,
            action_ref,
            admission_ref,
            reservation_id,
            permit_id,
        )
    }

    #[test]
    fn proof_subject_binds_every_routing_field_and_exact_human_model_target() {
        let run_id = Uuid::new_v4();
        let base = command(
            run_id,
            7,
            IdrCommandV1::CancelRun {
                reason_ref: reference("reason:test"),
            },
        );
        let (base_subject, base_digest) = base.proof_subject().unwrap();
        for (field, replacement) in [
            ("trust_domain", Value::String("production".to_string())),
            (
                "environment_ref",
                Value::String("environment:idr:shadow:redirected".to_string()),
            ),
            ("run_id", Value::String(Uuid::new_v4().to_string())),
            ("expected_aggregate_version", Value::from(8)),
            ("actor_ref", Value::String("actor:redirected".to_string())),
            ("caller_ref", Value::String("caller:redirected".to_string())),
            (
                "correlation_ref",
                Value::String("correlation:redirected".to_string()),
            ),
            (
                "causation_ref",
                Value::String("causation:redirected".to_string()),
            ),
        ] {
            let mut redirected = serde_json::to_value(&base).unwrap();
            redirected[field] = replacement;
            let redirected: IdrCommandEnvelopeV1 = serde_json::from_value(redirected).unwrap();
            let (subject, digest) = redirected.proof_subject().unwrap();
            assert_eq!(subject, base_subject);
            assert_ne!(digest, base_digest, "{field} was not proof-bound");
        }

        let turn_id = Uuid::new_v4();
        let assertion = CandidateSubmissionV1::new(
            CandidateKindV1::HumanModelAssertion,
            run_id,
            turn_id,
            reference("subject:test"),
            reference("tenant:test"),
            reference("scope:test"),
            reference("purpose:test"),
            reference("policy:test:v1"),
            1,
            10_000,
            serde_json::json!({"source_candidate_digest": "a".repeat(64)}),
        )
        .unwrap();
        let promotion_ref = record_ref(
            CandidateKindV1::HumanModelPromotionDecision,
            Uuid::new_v4(),
            3,
            'b',
        );
        let promotion = command(
            run_id,
            7,
            IdrCommandV1::PromoteHumanModelAssertion {
                assertion,
                promotion_decision_ref: promotion_ref,
            },
        );
        let (_, promotion_digest) = promotion.proof_subject().unwrap();
        let mut redirected = serde_json::to_value(&promotion).unwrap();
        redirected["command"]["payload"]["promotion_decision_ref"]["record_id"] =
            Value::String(Uuid::new_v4().to_string());
        let redirected: IdrCommandEnvelopeV1 = serde_json::from_value(redirected).unwrap();
        assert_ne!(
            redirected.proof_subject().unwrap().1,
            promotion_digest,
            "Human Model exact promotion target was not proof-bound"
        );
    }

    #[test]
    fn expired_action_admission_blocks_delayed_reservation() {
        let (mut projection, action_ref, mut admission_ref, _, _) =
            execution_projection(ExecutionAggregateStateV1::Expired, 90);
        let run_id = projection.run_id;
        admission_ref.valid_until = 100;
        projection.records.insert(
            CandidateKindV1::ActionAdmissionDecision,
            admission_ref.clone(),
        );
        let admission = projection.action_admission.as_mut().unwrap();
        admission.admission_ref = admission_ref.clone();
        admission.authorization_valid_until = 100;
        admission.valid_until = 100;
        let result = evaluate_authoritative_transition_v1(
            &OrchestratorAuthorityV1 { _private: () },
            projection,
            &command(
                run_id,
                10,
                IdrCommandV1::ReserveExecution {
                    action_admission_ref: admission_ref,
                    action_ref: action_ref.clone(),
                    action_digest: action_ref.record_digest().to_string(),
                    provider_ref: reference("provider:test"),
                    owner_ref: reference("caller:test"),
                    idempotency_key: "idempotency:test".to_string(),
                    attempt: 2,
                    lease_until: 200,
                    dispatch_nonce: "9".repeat(64),
                },
            ),
            101,
            &[],
        );
        assert!(matches!(result, Err(IdrRuntimeErrorV1::MissingDependency)));
    }

    #[test]
    fn expired_context_rejects_intent() {
        let run_id = Uuid::new_v4();
        let turn_id = Uuid::new_v4();
        let mut projection = running_projection(run_id, turn_id);
        let mut context = record_ref(CandidateKindV1::ContextSnapshot, Uuid::new_v4(), 1, '1');
        context.valid_until = 100;
        projection
            .records
            .insert(CandidateKindV1::ContextSnapshot, context);
        let result = evaluate_authoritative_transition_v1(
            &OrchestratorAuthorityV1 { _private: () },
            projection,
            &command(
                run_id,
                1,
                IdrCommandV1::RecordIntent {
                    intent: candidate(
                        CandidateKindV1::Intent,
                        run_id,
                        turn_id,
                        serde_json::json!({
                            "fast_path_outcome": "unknown",
                            "resolution_method": "full_path"
                        }),
                    ),
                },
            ),
            101,
            &[],
        );
        assert!(matches!(result, Err(IdrRuntimeErrorV1::MissingDependency)));
    }

    #[test]
    fn expired_intent_rejects_decision() {
        let run_id = Uuid::new_v4();
        let turn_id = Uuid::new_v4();
        let mut projection = running_projection(run_id, turn_id);
        let mut intent = record_ref(CandidateKindV1::Intent, Uuid::new_v4(), 1, '2');
        intent.valid_until = 100;
        projection.records.insert(CandidateKindV1::Intent, intent);
        let result = evaluate_authoritative_transition_v1(
            &OrchestratorAuthorityV1 { _private: () },
            projection,
            &command(
                run_id,
                1,
                IdrCommandV1::RecordDecision {
                    decision: candidate(
                        CandidateKindV1::Decision,
                        run_id,
                        turn_id,
                        serde_json::json!({
                            "necessity_outcome": "required",
                            "selected_option_ref": "option:test",
                            "selected_action_operation_ref": "operation:test",
                            "selected_action_parameter_digest": "3".repeat(64)
                        }),
                    ),
                },
            ),
            101,
            &[],
        );
        assert!(matches!(result, Err(IdrRuntimeErrorV1::MissingDependency)));
    }

    fn expired_action_dependency_result(
        expire_decision: bool,
    ) -> Result<IdrTransitionV1, IdrRuntimeErrorV1> {
        let run_id = Uuid::new_v4();
        let turn_id = Uuid::new_v4();
        let mut projection = running_projection(run_id, turn_id);
        let mut decision = record_ref(CandidateKindV1::Decision, Uuid::new_v4(), 1, '4');
        let mut turn = record_ref(CandidateKindV1::TurnCoordination, Uuid::new_v4(), 1, '5');
        if expire_decision {
            decision.valid_until = 100;
        } else {
            turn.valid_until = 100;
        }
        projection
            .records
            .insert(CandidateKindV1::Decision, decision.clone());
        projection
            .records
            .insert(CandidateKindV1::TurnCoordination, turn);
        projection.decision_binding = Some(DecisionBindingProjectionV1 {
            decision_ref: decision,
            selected_option_ref: reference("option:test"),
            operation_ref: reference("operation:test"),
            parameter_digest: "3".repeat(64),
        });
        evaluate_authoritative_transition_v1(
            &OrchestratorAuthorityV1 { _private: () },
            projection,
            &command(
                run_id,
                1,
                IdrCommandV1::RecordAction {
                    action: candidate(
                        CandidateKindV1::Action,
                        run_id,
                        turn_id,
                        serde_json::json!({
                            "selected_option_ref": "option:test",
                            "operation_ref": "operation:test",
                            "parameter_digest": "3".repeat(64)
                        }),
                    ),
                },
            ),
            101,
            &[],
        )
    }

    #[test]
    fn expired_decision_rejects_action() {
        assert!(matches!(
            expired_action_dependency_result(true),
            Err(IdrRuntimeErrorV1::MissingDependency)
        ));
    }

    #[test]
    fn expired_turn_rejects_action() {
        assert!(matches!(
            expired_action_dependency_result(false),
            Err(IdrRuntimeErrorV1::MissingDependency)
        ));
    }

    #[test]
    fn expired_response_cannot_send() {
        let run_id = Uuid::new_v4();
        let turn_id = Uuid::new_v4();
        let mut projection = running_projection(run_id, turn_id);
        let mut response = record_ref(CandidateKindV1::Response, Uuid::new_v4(), 1, '6');
        response.valid_until = 100;
        let rendered_bytes = b"expired response".to_vec();
        projection
            .records
            .insert(CandidateKindV1::Response, response.clone());
        projection.response_binding = Some(ResponseBindingProjectionV1 {
            response_ref: response.clone(),
            rendered_content_digest: format!("{:x}", Sha256::digest(&rendered_bytes)),
            channel_ref: reference("channel:test"),
            audience_ref: reference("audience:test"),
        });
        let result = evaluate_authoritative_transition_v1(
            &OrchestratorAuthorityV1 { _private: () },
            projection,
            &command(
                run_id,
                1,
                IdrCommandV1::ConsumeResponseSend {
                    response_ref: response,
                    rendered_bytes,
                    channel_ref: reference("channel:test"),
                    audience_ref: reference("audience:test"),
                    send_nonce: "7".repeat(64),
                },
            ),
            101,
            &[],
        );
        assert!(matches!(result, Err(IdrRuntimeErrorV1::MissingDependency)));
    }

    #[test]
    fn cancelled_old_action_does_not_block_new_action_reservation() {
        let (mut projection, old_action, _, old_reservation, _) =
            execution_projection(ExecutionAggregateStateV1::Cancelled, 500);
        let run_id = projection.run_id;
        let new_action = record_ref(CandidateKindV1::Action, Uuid::new_v4(), 1, '7');
        let new_admission = record_ref(
            CandidateKindV1::ActionAdmissionDecision,
            Uuid::new_v4(),
            1,
            '8',
        );
        projection
            .records
            .insert(CandidateKindV1::Action, new_action.clone());
        projection.records.insert(
            CandidateKindV1::ActionAdmissionDecision,
            new_admission.clone(),
        );
        projection.action_admission = Some(ActionAdmissionProjectionV1 {
            admission_ref: new_admission.clone(),
            action_ref: new_action.clone(),
            provider_ref: reference("provider:test"),
            owner_ref: reference("caller:test"),
            operation_ref: reference("operation:test"),
            parameter_digest: "d".repeat(64),
            request_digest: "e".repeat(64),
            idempotency_key: "idempotency:new".to_string(),
            proof_bindings: admission_proof_bindings(500),
            exact_authorization_proof_id: Uuid::new_v4(),
            authorization_valid_until: 500,
            valid_until: new_admission.valid_until(),
        });
        let transition = evaluate_authoritative_transition_v1(
            &OrchestratorAuthorityV1 { _private: () },
            projection,
            &command(
                run_id,
                10,
                IdrCommandV1::ReserveExecution {
                    action_admission_ref: new_admission,
                    action_ref: new_action.clone(),
                    action_digest: new_action.record_digest().to_string(),
                    provider_ref: reference("provider:test"),
                    owner_ref: reference("caller:test"),
                    idempotency_key: "idempotency:new".to_string(),
                    attempt: 1,
                    lease_until: 300,
                    dispatch_nonce: "6".repeat(64),
                },
            ),
            101,
            &[],
        )
        .unwrap();
        let reservation = transition.projection().execution.as_ref().unwrap();
        assert_ne!(reservation.reservation_id, old_reservation);
        assert_ne!(reservation.action_ref, old_action);
        assert_eq!(reservation.action_ref, new_action);
        assert_eq!(reservation.attempt, 1);
    }

    #[test]
    fn command_resource_budgets_fail_closed() {
        let run_id = Uuid::new_v4();
        let oversized = IdrCommandEnvelopeV1::new(
            IdrTrustDomainV1::Shadow,
            reference("environment:idr:shadow:test"),
            run_id,
            1,
            reference("actor:test"),
            reference("caller:test"),
            reference("tenant:test"),
            reference("scope:test"),
            reference("purpose:test"),
            reference("policy:test:v1"),
            reference("correlation:test"),
            reference("causation:test"),
            IdrCommandV1::ConsumeResponseSend {
                response_ref: record_ref(CandidateKindV1::Response, Uuid::new_v4(), 1, 'a'),
                rendered_bytes: vec![0; IDR_MAX_RENDERED_BYTES_V1 + 1],
                channel_ref: reference("channel:test"),
                audience_ref: reference("audience:test"),
                send_nonce: "b".repeat(64),
            },
            vec![],
        );
        assert!(matches!(oversized, Err(IdrRuntimeErrorV1::InvalidCommand)));

        let (_, action, admission, _, _) =
            execution_projection(ExecutionAggregateStateV1::Expired, 100);
        let oversized_key = IdrCommandEnvelopeV1::new(
            IdrTrustDomainV1::Shadow,
            reference("environment:idr:shadow:test"),
            run_id,
            1,
            reference("actor:test"),
            reference("caller:test"),
            reference("tenant:test"),
            reference("scope:test"),
            reference("purpose:test"),
            reference("policy:test:v1"),
            reference("correlation:test"),
            reference("causation:test"),
            IdrCommandV1::ReserveExecution {
                action_admission_ref: admission,
                action_ref: action.clone(),
                action_digest: action.record_digest().to_string(),
                provider_ref: reference("provider:test"),
                owner_ref: reference("caller:test"),
                idempotency_key: "x".repeat(IDR_MAX_IDEMPOTENCY_KEY_BYTES_V1 + 1),
                attempt: 2,
                lease_until: 200,
                dispatch_nonce: "c".repeat(64),
            },
            vec![],
        );
        assert!(matches!(
            oversized_key,
            Err(IdrRuntimeErrorV1::InvalidCommand)
        ));
    }

    #[test]
    fn lost_permit_recovery_persists_expiry_then_allows_contiguous_retry() {
        let (projection, action_ref, _admission_ref, reservation_id, _first_permit_id) =
            execution_projection(ExecutionAggregateStateV1::PermitIssued, 100);
        let authority = OrchestratorAuthorityV1 { _private: () };
        let expired = evaluate_authoritative_transition_v1(
            &authority,
            projection,
            &command(
                action_ref.record_id(),
                10,
                IdrCommandV1::RecoverPermit { reservation_id },
            ),
            101,
            &[],
        );
        // The run ID is deliberately different above and must fail closed.
        assert!(matches!(
            expired,
            Err(IdrRuntimeErrorV1::AggregateVersionConflict)
        ));

        let (projection, action_ref, admission_ref, reservation_id, first_permit_id) =
            execution_projection(ExecutionAggregateStateV1::PermitIssued, 100);
        let run_id = projection.run_id;
        let expired = evaluate_authoritative_transition_v1(
            &authority,
            projection,
            &command(run_id, 10, IdrCommandV1::RecoverPermit { reservation_id }),
            101,
            &[],
        )
        .unwrap();
        assert_eq!(
            expired.projection().execution.as_ref().unwrap().state,
            ExecutionAggregateStateV1::Expired
        );
        let retried = evaluate_authoritative_transition_v1(
            &authority,
            expired.projection().clone(),
            &command(
                run_id,
                11,
                IdrCommandV1::ReserveExecution {
                    action_admission_ref: admission_ref,
                    action_ref: action_ref.clone(),
                    action_digest: action_ref.record_digest().to_string(),
                    provider_ref: reference("provider:test"),
                    owner_ref: reference("caller:test"),
                    idempotency_key: "idempotency:test".to_string(),
                    attempt: 2,
                    lease_until: 200,
                    dispatch_nonce: "d".repeat(64),
                },
            ),
            102,
            &[],
        )
        .unwrap();
        let attempt = retried.projection().execution.as_ref().unwrap();
        assert_eq!(attempt.reservation_id, reservation_id);
        assert_eq!(attempt.attempt, 2);
        assert_ne!(attempt.permit_id, first_permit_id);
    }

    #[test]
    fn lease_expiry_after_dispatch_requires_reconciliation_not_retry() {
        let (projection, _, _, reservation_id, _) =
            execution_projection(ExecutionAggregateStateV1::DispatchStarted, 100);
        let run_id = projection.run_id;
        let transition = evaluate_authoritative_transition_v1(
            &OrchestratorAuthorityV1 { _private: () },
            projection,
            &command(
                run_id,
                10,
                IdrCommandV1::ExpireExecutionLease { reservation_id },
            ),
            101,
            &[],
        )
        .unwrap();
        assert_eq!(
            transition.projection().execution.as_ref().unwrap().state,
            ExecutionAggregateStateV1::ReconciliationRequired
        );
    }

    #[test]
    fn cancelled_run_is_a_global_execution_gate() {
        let (mut projection, action_ref, admission_ref, reservation_id, permit_id) =
            execution_projection(ExecutionAggregateStateV1::PermitIssued, 200);
        let run_id = projection.run_id;
        projection.state = IdrRunStateV1::Cancelled;
        let authority = OrchestratorAuthorityV1 { _private: () };
        let reserve = evaluate_authoritative_transition_v1(
            &authority,
            projection.clone(),
            &command(
                run_id,
                10,
                IdrCommandV1::ReserveExecution {
                    action_admission_ref: admission_ref,
                    action_ref: action_ref.clone(),
                    action_digest: action_ref.record_digest().to_string(),
                    provider_ref: reference("provider:test"),
                    owner_ref: reference("caller:test"),
                    idempotency_key: "idempotency:test".to_string(),
                    attempt: 2,
                    lease_until: 300,
                    dispatch_nonce: "f".repeat(64),
                },
            ),
            101,
            &[],
        );
        assert!(matches!(
            reserve,
            Err(IdrRuntimeErrorV1::InvalidRunTransition)
        ));
        let deliver = evaluate_authoritative_transition_v1(
            &authority,
            projection,
            &command(
                run_id,
                10,
                IdrCommandV1::DeliverPermit {
                    reservation_id,
                    permit_id,
                },
            ),
            101,
            &[],
        );
        assert!(matches!(
            deliver,
            Err(IdrRuntimeErrorV1::InvalidRunTransition)
        ));
    }

    #[test]
    fn recursive_action_invalidation_cancels_before_dispatch() {
        let (projection, action_ref, _, _, _) =
            execution_projection(ExecutionAggregateStateV1::PermitIssued, 200);
        let mut transition = IdrTransitionV1 {
            projection,
            authoritative_record: None,
            event_type: "test".to_string(),
            event_payload: Value::Null,
            consumed_proof_ids: vec![],
            consumed_nonces: vec![],
        };
        transition.apply_repository_invalidation_closure(
            &OrchestratorAuthorityV1 { _private: () },
            [(
                action_ref.record_id(),
                action_ref.revision(),
                action_ref.candidate_kind(),
                action_ref.trust_domain(),
                action_ref.environment_ref().clone(),
                action_ref.valid_from(),
                action_ref.valid_until(),
                action_ref.record_digest().to_string(),
            )],
            true,
        );
        assert_eq!(
            transition.projection().execution.as_ref().unwrap().state,
            ExecutionAggregateStateV1::Cancelled
        );
    }

    #[test]
    fn historical_evidence_windows_are_explicit_bounded_and_fail_closed() {
        let run_id = Uuid::new_v4();
        let turn_id = Uuid::new_v4();
        let mut projection = running_projection(run_id, turn_id);

        let mut receipt_ref = record_ref(CandidateKindV1::ExecutionReceipt, Uuid::new_v4(), 1, 'c');
        receipt_ref.valid_until = 100;
        projection
            .records
            .insert(CandidateKindV1::ExecutionReceipt, receipt_ref.clone());
        assert!(require_historical_evidence_record(
            &projection,
            &receipt_ref,
            100 + IDR_RECEIPT_OUTCOME_OBSERVATION_WINDOW_SECONDS_V1 - 1,
            IDR_RECEIPT_OUTCOME_OBSERVATION_WINDOW_SECONDS_V1,
        )
        .is_ok());
        assert!(matches!(
            require_historical_evidence_record(
                &projection,
                &receipt_ref,
                100 + IDR_RECEIPT_OUTCOME_OBSERVATION_WINDOW_SECONDS_V1,
                IDR_RECEIPT_OUTCOME_OBSERVATION_WINDOW_SECONDS_V1,
            ),
            Err(IdrRuntimeErrorV1::MissingDependency)
        ));

        let mut outcome_ref = record_ref(CandidateKindV1::Outcome, Uuid::new_v4(), 1, 'd');
        outcome_ref.valid_until = 200;
        projection
            .records
            .insert(CandidateKindV1::Outcome, outcome_ref.clone());
        assert!(require_historical_evidence_record(
            &projection,
            &outcome_ref,
            200 + IDR_OUTCOME_HUMAN_MODEL_WINDOW_SECONDS_V1 - 1,
            IDR_OUTCOME_HUMAN_MODEL_WINDOW_SECONDS_V1,
        )
        .is_ok());
        assert!(matches!(
            require_historical_evidence_record(
                &projection,
                &outcome_ref,
                200 + IDR_OUTCOME_HUMAN_MODEL_WINDOW_SECONDS_V1,
                IDR_OUTCOME_HUMAN_MODEL_WINDOW_SECONDS_V1,
            ),
            Err(IdrRuntimeErrorV1::MissingDependency)
        ));

        projection.invalidated_records.insert(outcome_ref.clone());
        assert!(matches!(
            require_historical_evidence_record(
                &projection,
                &outcome_ref,
                201,
                IDR_OUTCOME_HUMAN_MODEL_WINDOW_SECONDS_V1,
            ),
            Err(IdrRuntimeErrorV1::MissingDependency)
        ));
    }

    #[test]
    fn invalid_action_delivery_persists_cancellation_instead_of_rolling_back() {
        let (mut projection, action_ref, _, reservation_id, permit_id) =
            execution_projection(ExecutionAggregateStateV1::PermitIssued, 200);
        let run_id = projection.run_id;
        projection.invalidated_records.insert(action_ref);
        projection.execution.as_mut().unwrap().provider_ref = reference("caller:test");

        let transition = evaluate_authoritative_transition_v1(
            &OrchestratorAuthorityV1 { _private: () },
            projection,
            &command(
                run_id,
                10,
                IdrCommandV1::DeliverPermit {
                    reservation_id,
                    permit_id,
                },
            ),
            101,
            &[],
        )
        .unwrap();
        assert_eq!(
            transition.projection().execution.as_ref().unwrap().state,
            ExecutionAggregateStateV1::Cancelled
        );
        assert_eq!(
            transition.event_type(),
            "execution_cancelled_invalid_authority"
        );
    }

    #[test]
    fn expired_authority_after_dispatch_persists_reconciliation_state() {
        let (mut projection, _, _, reservation_id, permit_id) =
            execution_projection(ExecutionAggregateStateV1::DispatchStarted, 100);
        let run_id = projection.run_id;
        projection.execution.as_mut().unwrap().provider_ref = reference("caller:test");

        let transition = evaluate_authoritative_transition_v1(
            &OrchestratorAuthorityV1 { _private: () },
            projection,
            &command(
                run_id,
                10,
                IdrCommandV1::StartDispatch {
                    reservation_id,
                    permit_id,
                },
            ),
            101,
            &[],
        )
        .unwrap();
        assert_eq!(
            transition.projection().execution.as_ref().unwrap().state,
            ExecutionAggregateStateV1::ReconciliationRequired
        );
        assert_eq!(
            transition.event_type(),
            "execution_reconciliation_required_invalid_authority"
        );
    }
}
