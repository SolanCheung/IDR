//! Legacy durable single-node snapshot backend for logical contract history.
//!
//! The store maintains exact current pointers and propagates invalidation when
//! a contract revision supersedes a dependency. Snapshots carry a monotonic
//! digest chain and an append-only local anchor, but this backend is not the
//! immutable, externally anchored production audit ledger. It does not create
//! semantic contracts, grant authority, call models, or execute actions.

use fs2::FileExt;
use idr_protocol::human_centered::{
    ActionContractV1, DecisionContractV1, ExactActionAuthorizationV1, ExecutionPermitRequestV1,
    ExecutionReceiptV1, ExecutionStateV1, HumanCenteredContractIdV1, HumanCenteredContractKindV1,
    HumanCenteredContractMetadataV1, HumanCenteredContractRefV1, HumanCenteredProofIdV1,
    HumanCenteredProtocolError, HumanModelAssertionV1, IntentContractV1, OutcomeRecordV1,
    ProofKindV1, ResponseContractV1, SignedProofEnvelopeV1, TurnCoordinationPlanV1,
    VerifiedExecutionPermitRequestV1, VerifiedExecutionReceiptV1,
};
use idr_protocol::ProductionReferenceV1;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "contract_kind", content = "contract")]
pub enum HumanCenteredContractSnapshotV1 {
    Intent(IntentContractV1),
    Decision(DecisionContractV1),
    TurnCoordination(TurnCoordinationPlanV1),
    Response(ResponseContractV1),
    Action(ActionContractV1),
    ExecutionReceipt(ExecutionReceiptV1),
    Outcome(OutcomeRecordV1),
    HumanModelAssertion(HumanModelAssertionV1),
}

impl HumanCenteredContractSnapshotV1 {
    pub fn validate(&self) -> Result<(), HumanCenteredProtocolError> {
        match self {
            Self::Intent(value) => value.validate(),
            Self::Decision(value) => value.validate(),
            Self::TurnCoordination(value) => value.validate(),
            Self::Response(value) => value.validate(),
            Self::Action(value) => value.validate(),
            Self::ExecutionReceipt(value) => value.validate(),
            Self::Outcome(value) => value.validate(),
            Self::HumanModelAssertion(value) => value.validate(),
        }
    }

    pub fn record_ref(&self) -> Result<HumanCenteredContractRefV1, HumanCenteredProtocolError> {
        match self {
            Self::Intent(value) => value.record_ref(),
            Self::Decision(value) => value.record_ref(),
            Self::TurnCoordination(value) => value.record_ref(),
            Self::Response(value) => value.record_ref(),
            Self::Action(value) => value.record_ref(),
            Self::ExecutionReceipt(value) => value.record_ref(),
            Self::Outcome(value) => value.record_ref(),
            Self::HumanModelAssertion(value) => value.record_ref(),
        }
    }

    pub fn metadata(&self) -> &HumanCenteredContractMetadataV1 {
        match self {
            Self::Intent(value) => value.metadata(),
            Self::Decision(value) => value.metadata(),
            Self::TurnCoordination(value) => value.metadata(),
            Self::Response(value) => value.metadata(),
            Self::Action(value) => value.metadata(),
            Self::ExecutionReceipt(value) => value.metadata(),
            Self::Outcome(value) => value.metadata(),
            Self::HumanModelAssertion(value) => value.metadata(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct HumanCenteredStoreSnapshotV1 {
    #[serde(default)]
    sequence: u64,
    #[serde(default)]
    previous_snapshot_digest: Option<String>,
    #[serde(default)]
    snapshot_digest: String,
    records: Vec<HumanCenteredContractSnapshotV1>,
    current_pointers: Vec<HumanCenteredContractRefV1>,
    invalidated_refs: Vec<HumanCenteredContractRefV1>,
    #[serde(default)]
    execution_reservations: Vec<ExecutionReservationRecordV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoreAnchorRecordV1 {
    sequence: u64,
    snapshot_digest: String,
    previous_snapshot_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExecutionReservationRecordV1 {
    reservation_id: Uuid,
    action_ref: HumanCenteredContractRefV1,
    tenant_ref: ProductionReferenceV1,
    operation_ref: ProductionReferenceV1,
    idempotency_key: String,
    attempts: Vec<ExecutionAttemptRecordV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExecutionAttemptRecordV1 {
    attempt: u32,
    permit_id: HumanCenteredProofIdV1,
    proof_id: HumanCenteredProofIdV1,
    proof_nonce: String,
    proof_issuer_ref: ProductionReferenceV1,
    proof_key_id: ProductionReferenceV1,
    proof_expires_at: u64,
    subject_digest: String,
    permit_proof_envelope: SignedProofEnvelopeV1,
    authorization: ExactActionAuthorizationV1,
    request: ExecutionPermitRequestV1,
    consumed_at: u64,
    status: ExecutionReservationStatusV1,
    delivered_at: Option<u64>,
    dispatched_at: Option<u64>,
    receipt_ref: Option<HumanCenteredContractRefV1>,
    receipt_proof_envelope: Option<SignedProofEnvelopeV1>,
    receipt_verified_at: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionReservationStatusV1 {
    PermitIssued,
    Delivered,
    Dispatched,
    Cancelled,
    ReconciliationRequired,
    Succeeded,
    Failed,
    PartiallySucceeded,
    Rejected,
    Compensated,
    Expired,
}

impl ExecutionReservationStatusV1 {
    const fn permits_retry(self) -> bool {
        matches!(self, Self::Failed | Self::Rejected | Self::Expired)
    }

    const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded
                | Self::Failed
                | Self::PartiallySucceeded
                | Self::Rejected
                | Self::Compensated
                | Self::Cancelled
                | Self::Expired
        )
    }
}

/// Trusted time source owned by the Store boundary. Production callers cannot
/// supply an arbitrary `valid_at` value to Permit transitions.
pub trait TrustedClockV1: Send + Sync {
    fn now(&self) -> Result<u64, HumanCenteredStoreErrorV1>;
}

#[derive(Debug, Default)]
pub struct SystemTrustedClockV1;

impl TrustedClockV1 for SystemTrustedClockV1 {
    fn now(&self) -> Result<u64, HumanCenteredStoreErrorV1> {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .map_err(|_| HumanCenteredStoreErrorV1::TrustedClockUnavailable)
    }
}

/// Executor-facing permit issued only after the Store atomically consumes the
/// verified proof ID and nonce in its durable snapshot.
///
/// This type has neither `Deserialize` nor a public constructor, so a wire
/// caller cannot fabricate it and a verified proof cannot bypass replay
/// consumption by constructing a permit directly in the protocol crate.
#[derive(Debug, PartialEq, Eq)]
pub struct ExecutionPermitV1 {
    reservation_id: Uuid,
    permit_id: HumanCenteredProofIdV1,
    proof_key_id: ProductionReferenceV1,
    proof_verified_at: u64,
    issued_at: u64,
    request: ExecutionPermitRequestV1,
}

impl ExecutionPermitV1 {
    pub fn reservation_id(&self) -> Uuid {
        self.reservation_id
    }

    pub fn permit_id(&self) -> HumanCenteredProofIdV1 {
        self.permit_id
    }

    pub fn proof_key_id(&self) -> &ProductionReferenceV1 {
        &self.proof_key_id
    }

    pub fn proof_verified_at(&self) -> u64 {
        self.proof_verified_at
    }

    pub fn issued_at(&self) -> u64 {
        self.issued_at
    }

    pub fn request(&self) -> &ExecutionPermitRequestV1 {
        &self.request
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HumanCenteredCommitOutcomeV1 {
    Committed(HumanCenteredContractRefV1),
    AlreadyCommitted(HumanCenteredContractRefV1),
}

#[derive(Clone)]
pub struct FileHumanCenteredContractStoreV1 {
    path: PathBuf,
    snapshot: Arc<Mutex<HumanCenteredStoreSnapshotV1>>,
    clock: Arc<dyn TrustedClockV1>,
}

impl FileHumanCenteredContractStoreV1 {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, HumanCenteredStoreErrorV1> {
        Self::open_with_clock(path, Arc::new(SystemTrustedClockV1))
    }

    pub fn open_with_clock(
        path: impl Into<PathBuf>,
        clock: Arc<dyn TrustedClockV1>,
    ) -> Result<Self, HumanCenteredStoreErrorV1> {
        let path = path.into();
        let _store_lock = acquire_store_lock(&path)?;
        let snapshot = load_snapshot(&path)?;
        Ok(Self {
            path,
            snapshot: Arc::new(Mutex::new(snapshot)),
            clock,
        })
    }

    pub fn commit(
        &self,
        contract: HumanCenteredContractSnapshotV1,
    ) -> Result<HumanCenteredCommitOutcomeV1, HumanCenteredStoreErrorV1> {
        if matches!(
            &contract,
            HumanCenteredContractSnapshotV1::ExecutionReceipt(_)
        ) {
            return Err(HumanCenteredStoreErrorV1::VerifiedReceiptProofRequired);
        }
        self.commit_internal(contract, None)
    }

    pub fn commit_execution_receipt(
        &self,
        verified_receipt: VerifiedExecutionReceiptV1,
    ) -> Result<HumanCenteredCommitOutcomeV1, HumanCenteredStoreErrorV1> {
        let (receipt, proof_envelope, verified_at) = verified_receipt.into_parts();
        self.commit_internal(
            HumanCenteredContractSnapshotV1::ExecutionReceipt(receipt),
            Some((proof_envelope, verified_at)),
        )
    }

    fn commit_internal(
        &self,
        contract: HumanCenteredContractSnapshotV1,
        receipt_proof: Option<(SignedProofEnvelopeV1, u64)>,
    ) -> Result<HumanCenteredCommitOutcomeV1, HumanCenteredStoreErrorV1> {
        if matches!(
            &contract,
            HumanCenteredContractSnapshotV1::ExecutionReceipt(_)
        ) != receipt_proof.is_some()
        {
            return Err(HumanCenteredStoreErrorV1::VerifiedReceiptProofRequired);
        }
        contract.validate()?;
        let reference = contract.record_ref()?;
        let mut current = self
            .snapshot
            .lock()
            .map_err(|_| HumanCenteredStoreErrorV1::Poisoned)?;
        let _store_lock = acquire_store_lock(&self.path)?;
        let refreshed = load_snapshot(&self.path)?;
        *current = refreshed;
        if let Some((proof_envelope, verified_at)) = receipt_proof.as_ref() {
            let HumanCenteredContractSnapshotV1::ExecutionReceipt(receipt) = &contract else {
                return Err(HumanCenteredStoreErrorV1::VerifiedReceiptProofRequired);
            };
            validate_receipt_proof_at(receipt, proof_envelope, *verified_at, self.clock.now()?)?;
        }

        if current
            .records
            .iter()
            .any(|record| record.record_ref().ok().as_ref() == Some(&reference))
        {
            return Ok(HumanCenteredCommitOutcomeV1::AlreadyCommitted(reference));
        }

        let previous = current
            .current_pointers
            .iter()
            .find(|pointer| {
                pointer.kind() == reference.kind()
                    && pointer.contract_id() == reference.contract_id()
            })
            .cloned();
        match &previous {
            None if reference.revision() == 1
                && contract.metadata().predecessor_ref().is_none() => {}
            Some(pointer)
                if pointer.kind() == reference.kind()
                    && pointer.revision().checked_add(1) == Some(reference.revision())
                    && contract.metadata().predecessor_ref() == Some(pointer) => {}
            _ => return Err(HumanCenteredStoreErrorV1::StaleCurrentPointer),
        }
        if let Some(previous) = &previous {
            let previous_record = current
                .records
                .iter()
                .find(|record| record.record_ref().ok().as_ref() == Some(previous))
                .ok_or(HumanCenteredStoreErrorV1::CorruptSnapshot)?;
            if !lineage_context_is_continuous(previous_record.metadata(), contract.metadata()) {
                return Err(HumanCenteredStoreErrorV1::CrossObjectMismatch);
            }
        }

        for dependency in contract.metadata().dependency_refs() {
            let current_dependency = current.current_pointers.iter().find(|pointer| {
                pointer.contract_id() == dependency.contract_id()
                    && pointer.kind() == dependency.kind()
            });
            if current_dependency != Some(dependency)
                || current.invalidated_refs.contains(dependency)
                || !current
                    .records
                    .iter()
                    .any(|record| record.record_ref().ok().as_ref() == Some(dependency))
            {
                return Err(HumanCenteredStoreErrorV1::StaleDependency);
            }
        }
        validate_cross_object_consistency(
            &contract,
            &current.records,
            &current.execution_reservations,
        )
        .map_err(|_| HumanCenteredStoreErrorV1::CrossObjectMismatch)?;

        let mut candidate = current.clone();
        if let Some(previous) = &previous {
            invalidate_dependents(&mut candidate, previous)?;
        }
        let receipt_transition = match &contract {
            HumanCenteredContractSnapshotV1::ExecutionReceipt(receipt) => {
                Some((receipt.clone(), reference.clone()))
            }
            _ => None,
        };
        candidate.records.push(contract);
        if let Some((receipt, receipt_ref)) = receipt_transition {
            let (proof_envelope, verified_at) =
                receipt_proof.ok_or(HumanCenteredStoreErrorV1::VerifiedReceiptProofRequired)?;
            apply_execution_receipt_transition(
                &mut candidate,
                &receipt,
                receipt_ref,
                proof_envelope,
                verified_at,
            )?;
        }
        replace_pointer(&mut candidate.current_pointers, reference.clone());
        seal_next_snapshot(&mut candidate, &current)?;
        validate_snapshot(&candidate)?;
        persist_snapshot(&self.path, &candidate)?;
        *current = candidate;
        Ok(HumanCenteredCommitOutcomeV1::Committed(reference))
    }

    /// Atomically creates or advances the single durable reservation aggregate
    /// for an Action and its `(tenant, operation, idempotency_key)` scope.
    ///
    /// A second Authorization cannot mint a parallel Permit for the same
    /// Action. A later attempt is accepted only after a retryable terminal
    /// state and must use the next consecutive attempt number.
    pub fn issue_execution_permit(
        &self,
        verified_request: VerifiedExecutionPermitRequestV1,
    ) -> Result<ExecutionPermitV1, HumanCenteredStoreErrorV1> {
        let proof_id = verified_request.proof_id();
        let proof_nonce = verified_request.proof_nonce().to_string();
        let proof_issuer_ref = verified_request.proof_issuer_ref().clone();
        let proof_key_id = verified_request.proof_key_id().clone();
        let proof_expires_at = verified_request.proof_expires_at();
        let permit_proof_envelope = verified_request.proof_envelope().clone();
        let proof_verified_at = verified_request.verified_at();
        let authorization = verified_request.authorization().clone();
        let request = verified_request.request().clone();
        let action_ref = request.action_ref().clone();

        let mut current = self
            .snapshot
            .lock()
            .map_err(|_| HumanCenteredStoreErrorV1::Poisoned)?;
        let _store_lock = acquire_store_lock(&self.path)?;
        let refreshed = load_snapshot(&self.path)?;
        *current = refreshed;
        let trusted_now = self.clock.now()?;

        let is_current = current
            .current_pointers
            .iter()
            .any(|pointer| pointer == &action_ref)
            && !current.invalidated_refs.contains(&action_ref);
        if !is_current {
            return Err(HumanCenteredStoreErrorV1::StaleCurrentPointer);
        }
        let action = current
            .records
            .iter()
            .find_map(|record| match record {
                HumanCenteredContractSnapshotV1::Action(action)
                    if action.record_ref().ok().as_ref() == Some(&action_ref) =>
                {
                    Some(action)
                }
                _ => None,
            })
            .ok_or(HumanCenteredStoreErrorV1::CorruptSnapshot)?;
        verified_request.validate_at(action, trusted_now)?;

        if current.execution_reservations.iter().any(|reservation| {
            reservation
                .attempts
                .iter()
                .any(|attempt| attempt.proof_id == proof_id || attempt.proof_nonce == proof_nonce)
        }) {
            return Err(HumanCenteredStoreErrorV1::ProofReplayDetected);
        }
        if current.execution_reservations.iter().any(|reservation| {
            reservation.attempts.iter().any(|attempt| {
                attempt.authorization.authorization_id() == request.authorization_id()
            })
        }) {
            return Err(HumanCenteredStoreErrorV1::AuthorizationAlreadyConsumed);
        }

        let mut candidate = current.clone();
        let tenant_ref = action.metadata().authority_context().tenant_ref().clone();
        let operation_ref = action.operation_ref().clone();
        let idempotency_key = action.idempotency_key().to_string();
        let conflicting_position =
            candidate
                .execution_reservations
                .iter()
                .position(|reservation| {
                    reservation.action_ref == action_ref
                        || (reservation.tenant_ref == tenant_ref
                            && reservation.operation_ref == operation_ref
                            && reservation.idempotency_key == idempotency_key)
                });
        let next_attempt = ExecutionAttemptRecordV1 {
            attempt: request.attempt(),
            permit_id: proof_id,
            proof_id,
            proof_nonce,
            proof_issuer_ref,
            proof_key_id: proof_key_id.clone(),
            proof_expires_at,
            subject_digest: request.request_digest().to_string(),
            permit_proof_envelope,
            authorization,
            request: request.clone(),
            consumed_at: trusted_now,
            status: ExecutionReservationStatusV1::PermitIssued,
            delivered_at: None,
            dispatched_at: None,
            receipt_ref: None,
            receipt_proof_envelope: None,
            receipt_verified_at: None,
        };
        let reservation_id = if let Some(position) = conflicting_position {
            let reservation = &mut candidate.execution_reservations[position];
            if reservation.action_ref != action_ref
                || reservation.tenant_ref != tenant_ref
                || reservation.operation_ref != operation_ref
                || reservation.idempotency_key != idempotency_key
            {
                return Err(HumanCenteredStoreErrorV1::ExecutionAlreadyReserved);
            }
            let previous = reservation
                .attempts
                .last()
                .ok_or(HumanCenteredStoreErrorV1::CorruptSnapshot)?;
            if !previous.status.permits_retry()
                || request.attempt()
                    != previous
                        .attempt
                        .checked_add(1)
                        .ok_or(HumanCenteredStoreErrorV1::AttemptOverflow)?
            {
                return Err(HumanCenteredStoreErrorV1::ExecutionAlreadyReserved);
            }
            let reservation_id = reservation.reservation_id;
            reservation.attempts.push(next_attempt);
            reservation_id
        } else {
            if request.attempt() != 1 {
                return Err(HumanCenteredStoreErrorV1::InvalidExecutionAttempt);
            }
            let reservation_id = Uuid::new_v4();
            candidate
                .execution_reservations
                .push(ExecutionReservationRecordV1 {
                    reservation_id,
                    action_ref,
                    tenant_ref,
                    operation_ref,
                    idempotency_key,
                    attempts: vec![next_attempt],
                });
            reservation_id
        };
        seal_next_snapshot(&mut candidate, &current)?;
        validate_snapshot(&candidate)?;
        persist_snapshot(&self.path, &candidate)?;
        *current = candidate;
        Ok(ExecutionPermitV1 {
            reservation_id,
            permit_id: proof_id,
            proof_key_id,
            proof_verified_at,
            issued_at: trusted_now,
            request,
        })
    }

    /// Marks successful delivery of an opaque Permit to its bound owner. The
    /// transition is durable and idempotent for the same Permit.
    pub fn mark_execution_permit_delivered(
        &self,
        permit: &ExecutionPermitV1,
    ) -> Result<(), HumanCenteredStoreErrorV1> {
        self.transition_execution_permit(
            permit,
            ExecutionReservationStatusV1::PermitIssued,
            ExecutionReservationStatusV1::Delivered,
        )
    }

    /// Marks dispatch immediately before the provider call. A Receipt is
    /// accepted only for a Permit in this durable state.
    pub fn mark_execution_dispatched(
        &self,
        permit: &ExecutionPermitV1,
    ) -> Result<(), HumanCenteredStoreErrorV1> {
        self.transition_execution_permit(
            permit,
            ExecutionReservationStatusV1::Delivered,
            ExecutionReservationStatusV1::Dispatched,
        )
    }

    /// Recovers a Permit that was persisted but lost before provider dispatch.
    /// Dispatched attempts are deliberately not reissued because their external
    /// effect is unknown and must be reconciled instead of repeated.
    pub fn recover_execution_permit(
        &self,
        action_ref: &HumanCenteredContractRefV1,
        owner_ref: &ProductionReferenceV1,
    ) -> Result<ExecutionPermitV1, HumanCenteredStoreErrorV1> {
        let mut current = self
            .snapshot
            .lock()
            .map_err(|_| HumanCenteredStoreErrorV1::Poisoned)?;
        let _store_lock = acquire_store_lock(&self.path)?;
        let refreshed = load_snapshot(&self.path)?;
        *current = refreshed;
        let trusted_now = self.clock.now()?;
        let reservation = current
            .execution_reservations
            .iter()
            .find(|reservation| &reservation.action_ref == action_ref)
            .ok_or(HumanCenteredStoreErrorV1::ExecutionReservationNotFound)?;
        let attempt = reservation
            .attempts
            .last()
            .ok_or(HumanCenteredStoreErrorV1::CorruptSnapshot)?;
        if attempt.request.owner_ref() != owner_ref {
            return Err(HumanCenteredStoreErrorV1::ExecutionOwnerMismatch);
        }
        if !matches!(
            attempt.status,
            ExecutionReservationStatusV1::PermitIssued | ExecutionReservationStatusV1::Delivered
        ) {
            return Err(HumanCenteredStoreErrorV1::ExecutionRequiresReconciliation);
        }
        if trusted_now >= attempt.request.lease_until()
            || trusted_now >= attempt.proof_expires_at
            || trusted_now >= attempt.authorization.expires_at()
        {
            let mut candidate = current.clone();
            let candidate_attempt = candidate
                .execution_reservations
                .iter_mut()
                .find(|candidate| candidate.reservation_id == reservation.reservation_id)
                .and_then(|candidate| candidate.attempts.last_mut())
                .ok_or(HumanCenteredStoreErrorV1::CorruptSnapshot)?;
            candidate_attempt.status = ExecutionReservationStatusV1::Expired;
            seal_next_snapshot(&mut candidate, &current)?;
            validate_snapshot(&candidate)?;
            persist_snapshot(&self.path, &candidate)?;
            *current = candidate;
            return Err(HumanCenteredStoreErrorV1::ExecutionLeaseExpired);
        }
        Ok(ExecutionPermitV1 {
            reservation_id: reservation.reservation_id,
            permit_id: attempt.permit_id,
            proof_key_id: attempt.proof_key_id.clone(),
            proof_verified_at: attempt.consumed_at,
            issued_at: attempt.consumed_at,
            request: attempt.request.clone(),
        })
    }

    fn transition_execution_permit(
        &self,
        permit: &ExecutionPermitV1,
        expected: ExecutionReservationStatusV1,
        next: ExecutionReservationStatusV1,
    ) -> Result<(), HumanCenteredStoreErrorV1> {
        let mut current = self
            .snapshot
            .lock()
            .map_err(|_| HumanCenteredStoreErrorV1::Poisoned)?;
        let _store_lock = acquire_store_lock(&self.path)?;
        let refreshed = load_snapshot(&self.path)?;
        *current = refreshed;
        let trusted_now = self.clock.now()?;
        let mut candidate = current.clone();
        let reservation = candidate
            .execution_reservations
            .iter_mut()
            .find(|reservation| reservation.reservation_id == permit.reservation_id)
            .ok_or(HumanCenteredStoreErrorV1::ExecutionReservationNotFound)?;
        let reservation_id = reservation.reservation_id;
        let action_ref = reservation.action_ref.clone();
        let attempt = reservation
            .attempts
            .last_mut()
            .ok_or(HumanCenteredStoreErrorV1::CorruptSnapshot)?;
        validate_permit_matches_attempt(permit, reservation_id, &action_ref, attempt)?;
        let action_is_current = current
            .current_pointers
            .iter()
            .any(|pointer| pointer == &action_ref)
            && !current.invalidated_refs.contains(&action_ref);
        if !action_is_current {
            attempt.status = if attempt.dispatched_at.is_some() {
                ExecutionReservationStatusV1::ReconciliationRequired
            } else {
                ExecutionReservationStatusV1::Cancelled
            };
            seal_next_snapshot(&mut candidate, &current)?;
            validate_snapshot(&candidate)?;
            persist_snapshot(&self.path, &candidate)?;
            *current = candidate;
            return Err(HumanCenteredStoreErrorV1::ExecutionActionInvalidated);
        }
        if trusted_now >= attempt.request.lease_until()
            || trusted_now >= attempt.proof_expires_at
            || trusted_now >= attempt.authorization.expires_at()
        {
            attempt.status = ExecutionReservationStatusV1::Expired;
            seal_next_snapshot(&mut candidate, &current)?;
            validate_snapshot(&candidate)?;
            persist_snapshot(&self.path, &candidate)?;
            *current = candidate;
            return Err(HumanCenteredStoreErrorV1::ExecutionLeaseExpired);
        }
        if attempt.status == next {
            return Ok(());
        }
        if attempt.status != expected {
            return Err(HumanCenteredStoreErrorV1::InvalidExecutionTransition);
        }
        attempt.status = next;
        match next {
            ExecutionReservationStatusV1::Delivered => attempt.delivered_at = Some(trusted_now),
            ExecutionReservationStatusV1::Dispatched => attempt.dispatched_at = Some(trusted_now),
            _ => return Err(HumanCenteredStoreErrorV1::InvalidExecutionTransition),
        }
        seal_next_snapshot(&mut candidate, &current)?;
        validate_snapshot(&candidate)?;
        persist_snapshot(&self.path, &candidate)?;
        *current = candidate;
        Ok(())
    }

    pub fn load_current(
        &self,
        kind: HumanCenteredContractKindV1,
        contract_id: HumanCenteredContractIdV1,
    ) -> Result<Option<HumanCenteredContractSnapshotV1>, HumanCenteredStoreErrorV1> {
        let mut current = self
            .snapshot
            .lock()
            .map_err(|_| HumanCenteredStoreErrorV1::Poisoned)?;
        let _store_lock = acquire_store_lock(&self.path)?;
        let refreshed = load_snapshot(&self.path)?;
        *current = refreshed;
        let Some(reference) = current
            .current_pointers
            .iter()
            .find(|pointer| pointer.kind() == kind && pointer.contract_id() == contract_id)
        else {
            return Ok(None);
        };
        if current.invalidated_refs.contains(reference) {
            return Ok(None);
        }
        Ok(current
            .records
            .iter()
            .find(|record| record.record_ref().ok().as_ref() == Some(reference))
            .cloned())
    }

    pub fn is_invalidated(
        &self,
        reference: &HumanCenteredContractRefV1,
    ) -> Result<bool, HumanCenteredStoreErrorV1> {
        let mut current = self
            .snapshot
            .lock()
            .map_err(|_| HumanCenteredStoreErrorV1::Poisoned)?;
        let _store_lock = acquire_store_lock(&self.path)?;
        let refreshed = load_snapshot(&self.path)?;
        *current = refreshed;
        Ok(current.invalidated_refs.contains(reference))
    }

    pub fn revision_count(&self) -> Result<usize, HumanCenteredStoreErrorV1> {
        let mut current = self
            .snapshot
            .lock()
            .map_err(|_| HumanCenteredStoreErrorV1::Poisoned)?;
        let _store_lock = acquire_store_lock(&self.path)?;
        let refreshed = load_snapshot(&self.path)?;
        *current = refreshed;
        Ok(current.records.len())
    }
}

fn invalidate_dependents(
    snapshot: &mut HumanCenteredStoreSnapshotV1,
    stale_reference: &HumanCenteredContractRefV1,
) -> Result<(), HumanCenteredStoreErrorV1> {
    invalidate_execution_reservation(snapshot, stale_reference);
    let mut queue = VecDeque::from([stale_reference.clone()]);
    let mut visited = BTreeSet::new();
    while let Some(stale) = queue.pop_front() {
        if !visited.insert(stale.clone()) {
            continue;
        }
        let current_pointers = snapshot.current_pointers.clone();
        for pointer in current_pointers {
            if (pointer.kind() == stale.kind() && pointer.contract_id() == stale.contract_id())
                || snapshot.invalidated_refs.contains(&pointer)
            {
                continue;
            }
            let Some(record) = snapshot
                .records
                .iter()
                .find(|record| record.record_ref().ok().as_ref() == Some(&pointer))
            else {
                return Err(HumanCenteredStoreErrorV1::CorruptSnapshot);
            };
            if record.metadata().dependency_refs().contains(&stale) {
                snapshot.invalidated_refs.push(pointer.clone());
                queue.push_back(pointer);
            }
        }
    }
    snapshot.invalidated_refs.sort();
    snapshot.invalidated_refs.dedup();
    reconcile_invalidated_execution_reservations(snapshot);
    Ok(())
}

fn invalidate_execution_reservation(
    snapshot: &mut HumanCenteredStoreSnapshotV1,
    stale_reference: &HumanCenteredContractRefV1,
) {
    if stale_reference.kind() != HumanCenteredContractKindV1::Action {
        return;
    }
    for reservation in &mut snapshot.execution_reservations {
        if &reservation.action_ref != stale_reference {
            continue;
        }
        if let Some(attempt) = reservation.attempts.last_mut() {
            attempt.status = match attempt.status {
                ExecutionReservationStatusV1::PermitIssued
                | ExecutionReservationStatusV1::Delivered => {
                    ExecutionReservationStatusV1::Cancelled
                }
                ExecutionReservationStatusV1::Dispatched => {
                    ExecutionReservationStatusV1::ReconciliationRequired
                }
                status => status,
            };
        }
    }
}

fn reconcile_invalidated_execution_reservations(snapshot: &mut HumanCenteredStoreSnapshotV1) {
    for reservation in &mut snapshot.execution_reservations {
        let action_is_current = snapshot
            .current_pointers
            .iter()
            .any(|pointer| pointer == &reservation.action_ref)
            && !snapshot.invalidated_refs.contains(&reservation.action_ref);
        if action_is_current {
            continue;
        }
        if let Some(attempt) = reservation.attempts.last_mut() {
            attempt.status = match attempt.status {
                ExecutionReservationStatusV1::PermitIssued
                | ExecutionReservationStatusV1::Delivered => {
                    ExecutionReservationStatusV1::Cancelled
                }
                ExecutionReservationStatusV1::Dispatched => {
                    ExecutionReservationStatusV1::ReconciliationRequired
                }
                status => status,
            };
        }
    }
}

fn replace_pointer(
    pointers: &mut Vec<HumanCenteredContractRefV1>,
    next: HumanCenteredContractRefV1,
) {
    pointers.retain(|current| {
        current.kind() != next.kind() || current.contract_id() != next.contract_id()
    });
    pointers.push(next);
    pointers.sort();
}

fn validate_snapshot(
    snapshot: &HumanCenteredStoreSnapshotV1,
) -> Result<(), HumanCenteredStoreErrorV1> {
    let empty_state = snapshot.records.is_empty()
        && snapshot.current_pointers.is_empty()
        && snapshot.invalidated_refs.is_empty()
        && snapshot.execution_reservations.is_empty();
    if snapshot.sequence == 0 {
        if !empty_state
            || snapshot.previous_snapshot_digest.is_some()
            || !snapshot.snapshot_digest.is_empty()
        {
            return Err(HumanCenteredStoreErrorV1::CorruptSnapshot);
        }
    } else if !valid_store_digest(&snapshot.snapshot_digest)
        || snapshot
            .previous_snapshot_digest
            .as_ref()
            .is_some_and(|digest| !valid_store_digest(digest))
        || compute_snapshot_digest(snapshot)? != snapshot.snapshot_digest
    {
        return Err(HumanCenteredStoreErrorV1::CorruptSnapshot);
    }
    let mut record_refs = BTreeSet::new();
    let mut record_positions = BTreeMap::new();
    let mut revision_keys = BTreeSet::new();
    for (position, record) in snapshot.records.iter().enumerate() {
        record.validate()?;
        let reference = record.record_ref()?;
        if !record_refs.insert(reference.clone())
            || !revision_keys.insert((
                reference.kind(),
                reference.contract_id(),
                reference.revision(),
            ))
            || record_positions.insert(reference, position).is_some()
        {
            return Err(HumanCenteredStoreErrorV1::CorruptSnapshot);
        }
    }

    let mut latest_refs: BTreeMap<
        (HumanCenteredContractKindV1, HumanCenteredContractIdV1),
        HumanCenteredContractRefV1,
    > = BTreeMap::new();
    for (position, record) in snapshot.records.iter().enumerate() {
        let reference = record.record_ref()?;
        if let Some(predecessor) = record.metadata().predecessor_ref() {
            if predecessor.kind() != reference.kind()
                || record_positions
                    .get(predecessor)
                    .is_none_or(|predecessor_position| *predecessor_position >= position)
            {
                return Err(HumanCenteredStoreErrorV1::CorruptSnapshot);
            }
        }
        if record
            .metadata()
            .dependency_refs()
            .iter()
            .any(|dependency| {
                record_positions
                    .get(dependency)
                    .is_none_or(|dependency_position| *dependency_position >= position)
            })
        {
            return Err(HumanCenteredStoreErrorV1::CorruptSnapshot);
        }
        if validate_cross_object_consistency(
            record,
            &snapshot.records[..position],
            &snapshot.execution_reservations,
        )
        .is_err()
        {
            return Err(HumanCenteredStoreErrorV1::CorruptSnapshot);
        }
        let key = (reference.kind(), reference.contract_id());
        match latest_refs.get(&key) {
            Some(latest) if latest.revision() >= reference.revision() => {}
            _ => {
                latest_refs.insert(key, reference);
            }
        }
    }

    let mut current_ids = BTreeSet::new();
    for pointer in &snapshot.current_pointers {
        pointer.validate()?;
        if !record_refs.contains(pointer)
            || !current_ids.insert((pointer.kind(), pointer.contract_id()))
            || latest_refs.get(&(pointer.kind(), pointer.contract_id())) != Some(pointer)
        {
            return Err(HumanCenteredStoreErrorV1::CorruptSnapshot);
        }
    }
    if current_ids.len() != latest_refs.len() {
        return Err(HumanCenteredStoreErrorV1::CorruptSnapshot);
    }
    for reference in &snapshot.invalidated_refs {
        reference.validate()?;
        if !record_refs.contains(reference) {
            return Err(HumanCenteredStoreErrorV1::CorruptSnapshot);
        }
    }
    let replayed = replay_history(&snapshot.records)?;
    if replayed.current_pointers != snapshot.current_pointers
        || replayed.invalidated_refs != snapshot.invalidated_refs
    {
        return Err(HumanCenteredStoreErrorV1::CorruptSnapshot);
    }
    let mut reservation_ids = BTreeSet::new();
    let mut authorization_ids = BTreeSet::new();
    let mut idempotency_scopes = BTreeSet::new();
    let mut reserved_actions = BTreeSet::new();
    let mut proof_ids = BTreeSet::new();
    let mut proof_nonces = BTreeSet::new();
    for reservation in &snapshot.execution_reservations {
        reservation.action_ref.validate()?;
        let Some(HumanCenteredContractSnapshotV1::Action(action)) = snapshot
            .records
            .iter()
            .find(|record| record.record_ref().ok().as_ref() == Some(&reservation.action_ref))
        else {
            return Err(HumanCenteredStoreErrorV1::CorruptSnapshot);
        };
        if reservation.reservation_id.is_nil()
            || !reservation_ids.insert(reservation.reservation_id)
            || reservation.tenant_ref != *action.metadata().authority_context().tenant_ref()
            || reservation.operation_ref != *action.operation_ref()
            || reservation.idempotency_key != action.idempotency_key()
            || reservation.attempts.is_empty()
            || !reserved_actions.insert(reservation.action_ref.clone())
            || !idempotency_scopes.insert((
                reservation.tenant_ref.clone(),
                reservation.operation_ref.clone(),
                reservation.idempotency_key.clone(),
            ))
        {
            return Err(HumanCenteredStoreErrorV1::CorruptSnapshot);
        }
        for (index, attempt) in reservation.attempts.iter().enumerate() {
            let expected_attempt =
                u32::try_from(index + 1).map_err(|_| HumanCenteredStoreErrorV1::CorruptSnapshot)?;
            attempt
                .request
                .validate_for(action, &attempt.authorization)?;
            attempt.permit_proof_envelope.validate_shape()?;
            let permit_claims = attempt.permit_proof_envelope.claims();
            if attempt.attempt != expected_attempt
                || attempt.request.attempt() != expected_attempt
                || attempt.permit_id != attempt.proof_id
                || attempt.proof_id.is_empty()
                || !proof_ids.insert(attempt.proof_id)
                || !valid_store_digest(&attempt.proof_nonce)
                || !proof_nonces.insert(attempt.proof_nonce.clone())
                || attempt.proof_nonce != attempt.request.dispatch_nonce()
                || attempt.proof_issuer_ref.as_str().trim().is_empty()
                || attempt.proof_key_id.as_str().trim().is_empty()
                || attempt.proof_expires_at < attempt.request.lease_until()
                || permit_claims.proof_id() != attempt.proof_id
                || permit_claims.proof_kind() != ProofKindV1::ExecutionPermit
                || permit_claims.issuer_ref() != &attempt.proof_issuer_ref
                || attempt.permit_proof_envelope.key_id() != &attempt.proof_key_id
                || permit_claims.subject_ref() != attempt.request.request_ref()
                || permit_claims.subject_digest() != attempt.request.request_digest()
                || permit_claims.nonce() != attempt.request.dispatch_nonce()
                || permit_claims.expires_at() != attempt.proof_expires_at
                || !valid_store_digest(&attempt.subject_digest)
                || attempt.subject_digest != attempt.request.request_digest()
                || !authorization_ids.insert(attempt.authorization.authorization_id())
                || attempt.authorization.authorization_id() != attempt.request.authorization_id()
                || attempt.consumed_at < attempt.request.lease_issued_at()
                || attempt.consumed_at >= attempt.request.lease_until()
                || attempt.consumed_at >= attempt.proof_expires_at
                || attempt.consumed_at >= attempt.authorization.expires_at()
                || attempt.consumed_at >= action.metadata().valid_until()
                || attempt.request.action_ref() != &reservation.action_ref
            {
                return Err(HumanCenteredStoreErrorV1::CorruptSnapshot);
            }
            let timing_is_valid = match attempt.status {
                ExecutionReservationStatusV1::PermitIssued => {
                    attempt.delivered_at.is_none()
                        && attempt.dispatched_at.is_none()
                        && attempt.receipt_ref.is_none()
                        && attempt.receipt_proof_envelope.is_none()
                        && attempt.receipt_verified_at.is_none()
                }
                ExecutionReservationStatusV1::Delivered => {
                    attempt.delivered_at.is_some()
                        && attempt.dispatched_at.is_none()
                        && attempt.receipt_ref.is_none()
                        && attempt.receipt_proof_envelope.is_none()
                        && attempt.receipt_verified_at.is_none()
                }
                ExecutionReservationStatusV1::Dispatched => {
                    attempt.delivered_at.is_some()
                        && attempt.dispatched_at.is_some()
                        && attempt.receipt_ref.is_none()
                        && attempt.receipt_proof_envelope.is_none()
                        && attempt.receipt_verified_at.is_none()
                }
                ExecutionReservationStatusV1::Expired => {
                    attempt.receipt_ref.is_none()
                        && attempt.receipt_proof_envelope.is_none()
                        && attempt.receipt_verified_at.is_none()
                }
                ExecutionReservationStatusV1::Cancelled => {
                    attempt.dispatched_at.is_none()
                        && attempt.receipt_ref.is_none()
                        && attempt.receipt_proof_envelope.is_none()
                        && attempt.receipt_verified_at.is_none()
                }
                ExecutionReservationStatusV1::ReconciliationRequired => {
                    attempt.delivered_at.is_some()
                        && attempt.dispatched_at.is_some()
                        && attempt.receipt_ref.is_none()
                        && attempt.receipt_proof_envelope.is_none()
                        && attempt.receipt_verified_at.is_none()
                }
                status if status.is_terminal() => {
                    attempt.delivered_at.is_some()
                        && attempt.dispatched_at.is_some()
                        && attempt.receipt_ref.is_some()
                        && attempt.receipt_proof_envelope.is_some()
                        && attempt.receipt_verified_at.is_some()
                }
                _ => false,
            };
            if !timing_is_valid
                || attempt
                    .delivered_at
                    .is_some_and(|delivered| delivered < attempt.consumed_at)
                || attempt.dispatched_at.is_some_and(|dispatched| {
                    attempt
                        .delivered_at
                        .is_none_or(|delivered| dispatched < delivered)
                })
            {
                return Err(HumanCenteredStoreErrorV1::CorruptSnapshot);
            }
            if let Some(receipt_ref) = &attempt.receipt_ref {
                let Some(HumanCenteredContractSnapshotV1::ExecutionReceipt(receipt)) = snapshot
                    .records
                    .iter()
                    .find(|record| record.record_ref().ok().as_ref() == Some(receipt_ref))
                else {
                    return Err(HumanCenteredStoreErrorV1::CorruptSnapshot);
                };
                if !receipt_matches_attempt(receipt, reservation, attempt) {
                    return Err(HumanCenteredStoreErrorV1::CorruptSnapshot);
                }
                let proof_envelope = attempt
                    .receipt_proof_envelope
                    .as_ref()
                    .ok_or(HumanCenteredStoreErrorV1::CorruptSnapshot)?;
                proof_envelope.validate_shape()?;
                let claims = proof_envelope.claims();
                if claims.proof_kind() != ProofKindV1::ProviderExecutionReceipt
                    || claims.issuer_ref() != receipt.provider_ref()
                    || claims.subject_ref() != &receipt.proof_subject_ref()?
                    || claims.subject_digest() != receipt_ref.record_digest()
                    || claims.nonce() != receipt.dispatch_nonce()
                    || claims.issued_at() < receipt.completed_at()
                    || attempt.receipt_verified_at.is_none_or(|verified_at| {
                        verified_at < claims.issued_at() || verified_at >= claims.expires_at()
                    })
                {
                    return Err(HumanCenteredStoreErrorV1::CorruptSnapshot);
                }
            }
            if index + 1 < reservation.attempts.len() && !attempt.status.permits_retry() {
                return Err(HumanCenteredStoreErrorV1::CorruptSnapshot);
            }
        }
    }
    Ok(())
}

fn seal_next_snapshot(
    candidate: &mut HumanCenteredStoreSnapshotV1,
    previous: &HumanCenteredStoreSnapshotV1,
) -> Result<(), HumanCenteredStoreErrorV1> {
    candidate.sequence = previous
        .sequence
        .checked_add(1)
        .ok_or(HumanCenteredStoreErrorV1::SequenceOverflow)?;
    candidate.previous_snapshot_digest =
        (previous.sequence > 0).then(|| previous.snapshot_digest.clone());
    candidate.snapshot_digest.clear();
    candidate.snapshot_digest = compute_snapshot_digest(candidate)?;
    Ok(())
}

fn compute_snapshot_digest(
    snapshot: &HumanCenteredStoreSnapshotV1,
) -> Result<String, HumanCenteredStoreErrorV1> {
    let bytes = serde_json::to_vec(&(
        "idr-store-snapshot-v3",
        snapshot.sequence,
        &snapshot.previous_snapshot_digest,
        &snapshot.records,
        &snapshot.current_pointers,
        &snapshot.invalidated_refs,
        &snapshot.execution_reservations,
    ))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn valid_store_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn replay_history(
    records: &[HumanCenteredContractSnapshotV1],
) -> Result<HumanCenteredStoreSnapshotV1, HumanCenteredStoreErrorV1> {
    let mut replayed = HumanCenteredStoreSnapshotV1::default();
    for record in records {
        let reference = record.record_ref()?;
        let previous = replayed
            .current_pointers
            .iter()
            .find(|pointer| {
                pointer.kind() == reference.kind()
                    && pointer.contract_id() == reference.contract_id()
            })
            .cloned();
        match &previous {
            None if reference.revision() == 1 && record.metadata().predecessor_ref().is_none() => {}
            Some(pointer)
                if pointer.revision().checked_add(1) == Some(reference.revision())
                    && record.metadata().predecessor_ref() == Some(pointer) =>
            {
                let previous_record = replayed
                    .records
                    .iter()
                    .find(|candidate| candidate.record_ref().ok().as_ref() == Some(pointer))
                    .ok_or(HumanCenteredStoreErrorV1::CorruptSnapshot)?;
                if !lineage_context_is_continuous(previous_record.metadata(), record.metadata()) {
                    return Err(HumanCenteredStoreErrorV1::CorruptSnapshot);
                }
                invalidate_dependents(&mut replayed, pointer)?;
            }
            _ => return Err(HumanCenteredStoreErrorV1::CorruptSnapshot),
        }
        replayed.records.push(record.clone());
        replace_pointer(&mut replayed.current_pointers, reference);
    }
    Ok(replayed)
}

fn lineage_context_is_continuous(
    previous: &HumanCenteredContractMetadataV1,
    next: &HumanCenteredContractMetadataV1,
) -> bool {
    previous.run_id() == next.run_id()
        && previous.turn_id() == next.turn_id()
        && previous.authority_context() == next.authority_context()
}

fn validate_cross_object_consistency(
    contract: &HumanCenteredContractSnapshotV1,
    prior_records: &[HumanCenteredContractSnapshotV1],
    execution_reservations: &[ExecutionReservationRecordV1],
) -> Result<(), ()> {
    let same_context = |left: &HumanCenteredContractMetadataV1,
                        right: &HumanCenteredContractMetadataV1| {
        left.run_id() == right.run_id()
            && left.turn_id() == right.turn_id()
            && left.authority_context() == right.authority_context()
    };
    let find = |reference: &HumanCenteredContractRefV1| {
        prior_records
            .iter()
            .find(|record| record.record_ref().ok().as_ref() == Some(reference))
    };

    match contract {
        HumanCenteredContractSnapshotV1::Intent(_) => {}
        HumanCenteredContractSnapshotV1::Decision(decision) => {
            let Some(HumanCenteredContractSnapshotV1::Intent(intent)) = find(decision.intent_ref())
            else {
                return Err(());
            };
            if !same_context(decision.metadata(), intent.metadata()) {
                return Err(());
            }
        }
        HumanCenteredContractSnapshotV1::TurnCoordination(plan) => {
            for reference in plan.response_refs().iter().chain(plan.action_refs().iter()) {
                let Some(dependency) = find(reference) else {
                    return Err(());
                };
                if !same_context(plan.metadata(), dependency.metadata()) {
                    return Err(());
                }
            }
        }
        HumanCenteredContractSnapshotV1::Response(response) => {
            for reference in response.source_contract_refs() {
                let Some(source) = find(reference) else {
                    return Err(());
                };
                if !same_context(response.metadata(), source.metadata()) {
                    return Err(());
                }
            }
        }
        HumanCenteredContractSnapshotV1::Action(action) => {
            if let Some(reference) = action.decision_ref() {
                let Some(HumanCenteredContractSnapshotV1::Decision(decision)) = find(reference)
                else {
                    return Err(());
                };
                if !same_context(action.metadata(), decision.metadata())
                    || !decision
                        .authorization_boundary()
                        .required_authority_refs
                        .iter()
                        .all(|required| action.required_authority_refs().contains(required))
                {
                    return Err(());
                }
                if action.authorization_state()
                    == idr_protocol::human_centered::ActionAuthorizationStateV1::NotRequired
                    && (action.impact_level().rank()
                        > decision
                            .authorization_boundary()
                            .maximum_automatic_impact
                            .rank()
                        || decision.authorization_boundary().user_confirmation_required
                        || decision
                            .authorization_boundary()
                            .prohibited_automatic_operations
                            .contains(action.operation_ref()))
                {
                    return Err(());
                }
            }
        }
        HumanCenteredContractSnapshotV1::ExecutionReceipt(receipt) => {
            let Some(HumanCenteredContractSnapshotV1::Action(action)) = find(receipt.action_ref())
            else {
                return Err(());
            };
            let Some(reservation) = execution_reservations
                .iter()
                .find(|reservation| reservation.action_ref == *receipt.action_ref())
            else {
                return Err(());
            };
            let Some(attempt) = reservation
                .attempts
                .iter()
                .find(|attempt| attempt.permit_id == receipt.permit_id())
            else {
                return Err(());
            };
            if !same_context(receipt.metadata(), action.metadata())
                || receipt.idempotency_key() != action.idempotency_key()
                || receipt.request_parameter_digest() != action.parameter_digest()
                || !(matches!(
                    attempt.status,
                    ExecutionReservationStatusV1::Dispatched
                        | ExecutionReservationStatusV1::ReconciliationRequired
                ) || (attempt.status.is_terminal()
                    && attempt.receipt_ref.as_ref() == receipt.record_ref().ok().as_ref()))
                || !receipt_matches_attempt(receipt, reservation, attempt)
                || attempt
                    .dispatched_at
                    .is_none_or(|dispatched_at| receipt.started_at() < dispatched_at)
                || receipt.started_at() >= attempt.request.lease_until()
                || prior_records.iter().any(|record| {
                    matches!(
                        record,
                        HumanCenteredContractSnapshotV1::ExecutionReceipt(previous)
                            if previous.action_ref() == receipt.action_ref()
                                && previous.attempt() == receipt.attempt()
                    )
                })
            {
                return Err(());
            }
        }
        HumanCenteredContractSnapshotV1::Outcome(outcome) => {
            let decision = outcome
                .decision_ref()
                .map(|reference| find(reference).ok_or(()))
                .transpose()?;
            let action = outcome
                .action_ref()
                .map(|reference| find(reference).ok_or(()))
                .transpose()?;
            let receipt = outcome
                .execution_receipt_ref()
                .map(|reference| find(reference).ok_or(()))
                .transpose()?;
            for dependency in [decision, action, receipt].into_iter().flatten() {
                if !same_context(outcome.metadata(), dependency.metadata()) {
                    return Err(());
                }
            }
            if let Some(decision) = decision {
                if !matches!(decision, HumanCenteredContractSnapshotV1::Decision(_)) {
                    return Err(());
                }
            }
            if let Some(action_record) = action {
                let HumanCenteredContractSnapshotV1::Action(action_contract) = action_record else {
                    return Err(());
                };
                if let Some(decision_ref) = outcome.decision_ref() {
                    if action_contract.decision_ref() != Some(decision_ref) {
                        return Err(());
                    }
                }
            }
            if let Some(receipt_record) = receipt {
                let HumanCenteredContractSnapshotV1::ExecutionReceipt(receipt_contract) =
                    receipt_record
                else {
                    return Err(());
                };
                if receipt_contract.action_ref() != outcome.action_ref().ok_or(())? {
                    return Err(());
                }
                if matches!(
                    receipt_contract.state(),
                    idr_protocol::human_centered::ExecutionStateV1::Failed
                        | idr_protocol::human_centered::ExecutionStateV1::Rejected
                ) && !outcome.success_criteria_satisfied().is_empty()
                {
                    return Err(());
                }
            }
        }
        HumanCenteredContractSnapshotV1::HumanModelAssertion(_) => {}
    }
    Ok(())
}

fn validate_permit_matches_attempt(
    permit: &ExecutionPermitV1,
    reservation_id: Uuid,
    action_ref: &HumanCenteredContractRefV1,
    attempt: &ExecutionAttemptRecordV1,
) -> Result<(), HumanCenteredStoreErrorV1> {
    if permit.reservation_id != reservation_id
        || permit.permit_id != attempt.permit_id
        || permit.proof_key_id != attempt.proof_key_id
        || permit.request != attempt.request
        || permit.request.action_ref() != action_ref
    {
        return Err(HumanCenteredStoreErrorV1::ExecutionPermitMismatch);
    }
    Ok(())
}

fn receipt_matches_attempt(
    receipt: &ExecutionReceiptV1,
    reservation: &ExecutionReservationRecordV1,
    attempt: &ExecutionAttemptRecordV1,
) -> bool {
    receipt.action_ref() == &reservation.action_ref
        && receipt.permit_id() == attempt.permit_id
        && receipt.authorization_id() == attempt.authorization.authorization_id()
        && receipt.provider_ref() == attempt.request.provider_ref()
        && receipt.owner_ref() == attempt.request.owner_ref()
        && receipt.dispatch_nonce() == attempt.request.dispatch_nonce()
        && receipt.attempt() == attempt.attempt
        && receipt.idempotency_key() == reservation.idempotency_key
}

fn apply_execution_receipt_transition(
    snapshot: &mut HumanCenteredStoreSnapshotV1,
    receipt: &ExecutionReceiptV1,
    receipt_ref: HumanCenteredContractRefV1,
    proof_envelope: SignedProofEnvelopeV1,
    verified_at: u64,
) -> Result<(), HumanCenteredStoreErrorV1> {
    let reservation = snapshot
        .execution_reservations
        .iter_mut()
        .find(|reservation| reservation.action_ref == *receipt.action_ref())
        .ok_or(HumanCenteredStoreErrorV1::ExecutionReservationNotFound)?;
    let reservation_id = reservation.reservation_id;
    let action_ref = reservation.action_ref.clone();
    let attempt = reservation
        .attempts
        .iter_mut()
        .find(|attempt| attempt.permit_id == receipt.permit_id())
        .ok_or(HumanCenteredStoreErrorV1::ExecutionReservationNotFound)?;
    if !matches!(
        attempt.status,
        ExecutionReservationStatusV1::Dispatched
            | ExecutionReservationStatusV1::ReconciliationRequired
    ) || !receipt_matches_attempt_parts(receipt, reservation_id, &action_ref, attempt)
    {
        return Err(HumanCenteredStoreErrorV1::InvalidExecutionTransition);
    }
    attempt.status = match receipt.state() {
        ExecutionStateV1::Succeeded => ExecutionReservationStatusV1::Succeeded,
        ExecutionStateV1::Failed => ExecutionReservationStatusV1::Failed,
        ExecutionStateV1::PartiallySucceeded => ExecutionReservationStatusV1::PartiallySucceeded,
        ExecutionStateV1::Rejected => ExecutionReservationStatusV1::Rejected,
        ExecutionStateV1::Compensated => ExecutionReservationStatusV1::Compensated,
    };
    attempt.receipt_ref = Some(receipt_ref);
    attempt.receipt_proof_envelope = Some(proof_envelope);
    attempt.receipt_verified_at = Some(verified_at);
    Ok(())
}

fn receipt_matches_attempt_parts(
    receipt: &ExecutionReceiptV1,
    _reservation_id: Uuid,
    action_ref: &HumanCenteredContractRefV1,
    attempt: &ExecutionAttemptRecordV1,
) -> bool {
    receipt.action_ref() == action_ref
        && receipt.permit_id() == attempt.permit_id
        && receipt.authorization_id() == attempt.authorization.authorization_id()
        && receipt.provider_ref() == attempt.request.provider_ref()
        && receipt.owner_ref() == attempt.request.owner_ref()
        && receipt.dispatch_nonce() == attempt.request.dispatch_nonce()
        && receipt.attempt() == attempt.attempt
        && receipt.idempotency_key() == attempt.request.idempotency_key()
}

fn validate_receipt_proof_at(
    receipt: &ExecutionReceiptV1,
    proof_envelope: &SignedProofEnvelopeV1,
    verified_at: u64,
    trusted_now: u64,
) -> Result<(), HumanCenteredStoreErrorV1> {
    receipt.validate()?;
    proof_envelope.validate_shape()?;
    let expected = receipt.expected_provider_proof_binding()?;
    let claims = proof_envelope.claims();
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
        || trusted_now < receipt.completed_at()
        || trusted_now < claims.issued_at()
        || trusted_now >= claims.expires_at()
        || verified_at < claims.issued_at()
        || verified_at >= claims.expires_at()
        || verified_at > trusted_now
    {
        return Err(HumanCenteredStoreErrorV1::Protocol(
            HumanCenteredProtocolError::ProofVerificationFailed,
        ));
    }
    Ok(())
}

fn persist_snapshot(
    path: &Path,
    snapshot: &HumanCenteredStoreSnapshotV1,
) -> Result<(), HumanCenteredStoreErrorV1> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let extension = format!("tmp.{}.{}", std::process::id(), Uuid::new_v4());
    let temporary = path.with_extension(extension);
    let bytes = serde_json::to_vec_pretty(snapshot)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    if let Some(parent) = path.parent() {
        fs::File::open(parent)?.sync_all()?;
    }
    append_anchor(path, snapshot)?;
    Ok(())
}

fn load_snapshot(path: &Path) -> Result<HumanCenteredStoreSnapshotV1, HumanCenteredStoreErrorV1> {
    let snapshot = read_snapshot(path)?;
    validate_snapshot(&snapshot)?;
    validate_or_recover_anchor(path, &snapshot)?;
    Ok(snapshot)
}

fn read_snapshot(path: &Path) -> Result<HumanCenteredStoreSnapshotV1, HumanCenteredStoreErrorV1> {
    if !path.exists() {
        return Ok(HumanCenteredStoreSnapshotV1::default());
    }
    let bytes = fs::read(path)?;
    if bytes.is_empty() {
        Err(HumanCenteredStoreErrorV1::CorruptSnapshot)
    } else {
        Ok(serde_json::from_slice(&bytes)?)
    }
}

fn validate_or_recover_anchor(
    path: &Path,
    snapshot: &HumanCenteredStoreSnapshotV1,
) -> Result<(), HumanCenteredStoreErrorV1> {
    let anchors = read_anchor_records(path)?;
    validate_anchor_chain(&anchors)?;
    if snapshot.sequence == 0 {
        return if anchors.is_empty() {
            Ok(())
        } else {
            Err(HumanCenteredStoreErrorV1::RollbackDetected)
        };
    }
    let Some(latest) = anchors.last() else {
        if snapshot.sequence == 1 && snapshot.previous_snapshot_digest.is_none() {
            return append_anchor(path, snapshot);
        }
        return Err(HumanCenteredStoreErrorV1::RollbackDetected);
    };
    if latest.sequence == snapshot.sequence
        && latest.snapshot_digest == snapshot.snapshot_digest
        && latest.previous_snapshot_digest == snapshot.previous_snapshot_digest
    {
        return Ok(());
    }
    if latest.sequence.checked_add(1) == Some(snapshot.sequence)
        && snapshot.previous_snapshot_digest.as_ref() == Some(&latest.snapshot_digest)
    {
        return append_anchor(path, snapshot);
    }
    Err(HumanCenteredStoreErrorV1::RollbackDetected)
}

fn read_anchor_records(path: &Path) -> Result<Vec<StoreAnchorRecordV1>, HumanCenteredStoreErrorV1> {
    let anchor_path = anchor_path(path);
    if !anchor_path.exists() {
        return Ok(Vec::new());
    }
    let bytes = fs::read(&anchor_path)?;
    let committed_len = bytes
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |index| index + 1);
    let committed = &bytes[..committed_len];
    let mut records = Vec::new();
    if !committed.is_empty() {
        for line in committed[..committed.len() - 1].split(|byte| *byte == b'\n') {
            if line.iter().all(u8::is_ascii_whitespace) {
                return Err(HumanCenteredStoreErrorV1::CorruptAnchor);
            }
            records.push(
                serde_json::from_slice(line)
                    .map_err(|_| HumanCenteredStoreErrorV1::CorruptAnchor)?,
            );
        }
    }
    validate_anchor_chain(&records)?;

    // A newline is the local commit marker for an anchor record. Only bytes
    // after the final marker may be discarded; malformed committed records or
    // gaps in the chain remain hard corruption errors.
    if committed_len < bytes.len() {
        let file = OpenOptions::new().write(true).open(&anchor_path)?;
        file.set_len(
            u64::try_from(committed_len).map_err(|_| HumanCenteredStoreErrorV1::CorruptAnchor)?,
        )?;
        file.sync_all()?;
        if let Some(parent) = anchor_path.parent() {
            fs::File::open(parent)?.sync_all()?;
        }
    }
    if committed_len == 0 && !bytes.is_empty() && records.is_empty() {
        // The only record was incomplete. It is safe to recover it from the
        // already sealed snapshot in validate_or_recover_anchor().
        if !bytes.iter().any(|byte| !byte.is_ascii_whitespace()) {
            return Err(HumanCenteredStoreErrorV1::CorruptAnchor);
        }
    }
    Ok(records)
}

fn validate_anchor_chain(anchors: &[StoreAnchorRecordV1]) -> Result<(), HumanCenteredStoreErrorV1> {
    let mut previous: Option<&StoreAnchorRecordV1> = None;
    for anchor in anchors {
        if !valid_store_digest(&anchor.snapshot_digest)
            || anchor
                .previous_snapshot_digest
                .as_ref()
                .is_some_and(|digest| !valid_store_digest(digest))
        {
            return Err(HumanCenteredStoreErrorV1::CorruptAnchor);
        }
        match previous {
            None if anchor.sequence == 1 && anchor.previous_snapshot_digest.is_none() => {}
            Some(prior)
                if prior.sequence.checked_add(1) == Some(anchor.sequence)
                    && anchor.previous_snapshot_digest.as_ref() == Some(&prior.snapshot_digest) => {
            }
            _ => return Err(HumanCenteredStoreErrorV1::CorruptAnchor),
        }
        previous = Some(anchor);
    }
    Ok(())
}

fn append_anchor(
    path: &Path,
    snapshot: &HumanCenteredStoreSnapshotV1,
) -> Result<(), HumanCenteredStoreErrorV1> {
    let anchor = StoreAnchorRecordV1 {
        sequence: snapshot.sequence,
        snapshot_digest: snapshot.snapshot_digest.clone(),
        previous_snapshot_digest: snapshot.previous_snapshot_digest.clone(),
    };
    let anchor_path = anchor_path(path);
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(&anchor_path)?;
    serde_json::to_writer(&mut file, &anchor)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    if let Some(parent) = anchor_path.parent() {
        fs::File::open(parent)?.sync_all()?;
    }
    Ok(())
}

fn anchor_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".anchor");
    PathBuf::from(name)
}

fn acquire_store_lock(path: &Path) -> Result<fs::File, HumanCenteredStoreErrorV1> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut lock_name = path.as_os_str().to_os_string();
    lock_name.push(".lock");
    let lock_path = PathBuf::from(lock_name);
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)?;
    file.lock_exclusive()?;
    Ok(file)
}

#[derive(Debug, thiserror::Error)]
pub enum HumanCenteredStoreErrorV1 {
    #[error("human-centered protocol error: {0}")]
    Protocol(#[from] HumanCenteredProtocolError),
    #[error("human-centered store I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("human-centered store serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("human-centered store lock is poisoned")]
    Poisoned,
    #[error("human-centered current pointer is stale")]
    StaleCurrentPointer,
    #[error("human-centered dependency is stale or invalidated")]
    StaleDependency,
    #[error("human-centered cross-object contract consistency check failed")]
    CrossObjectMismatch,
    #[error("human-centered durable snapshot is corrupt")]
    CorruptSnapshot,
    #[error("human-centered snapshot anchor is corrupt")]
    CorruptAnchor,
    #[error("human-centered snapshot rollback or anchor loss was detected")]
    RollbackDetected,
    #[error("execution proof ID or nonce has already been consumed")]
    ProofReplayDetected,
    #[error("exact action authorization has already issued an execution permit")]
    AuthorizationAlreadyConsumed,
    #[error("trusted clock is unavailable")]
    TrustedClockUnavailable,
    #[error("the Action or idempotency scope already has an execution reservation")]
    ExecutionAlreadyReserved,
    #[error("execution reservation was not found")]
    ExecutionReservationNotFound,
    #[error("execution Permit does not match the durable reservation")]
    ExecutionPermitMismatch,
    #[error("execution owner does not match the durable reservation")]
    ExecutionOwnerMismatch,
    #[error("execution lease has expired")]
    ExecutionLeaseExpired,
    #[error("execution was dispatched and requires provider reconciliation")]
    ExecutionRequiresReconciliation,
    #[error(
        "the bound Action is no longer current; execution was cancelled or requires reconciliation"
    )]
    ExecutionActionInvalidated,
    #[error("execution state transition is invalid")]
    InvalidExecutionTransition,
    #[error("a verified provider Receipt proof is required")]
    VerifiedReceiptProofRequired,
    #[error("execution attempt number is invalid")]
    InvalidExecutionAttempt,
    #[error("execution attempt number overflow")]
    AttemptOverflow,
    #[error("human-centered snapshot sequence overflow")]
    SequenceOverflow,
}
