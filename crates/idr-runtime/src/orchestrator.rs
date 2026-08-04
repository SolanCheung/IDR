//! Persistent-orchestrator protocol surface.
//!
//! V1.3 does not yet ship a production repository or scheduler. These records
//! define the durable state and command boundary that implementations must use;
//! they do not authorize effects or weaken the production feature gates.

use idr_protocol::human_centered::{
    HumanCenteredContractRefV1, HumanCenteredProtocolError, HumanCenteredRunIdV1,
    HumanCenteredTurnIdV1, InteractionRunStateV1,
};
use idr_protocol::ProductionReferenceV1;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrchestrationStepKindV1 {
    InputAdmission,
    ContextSnapshot,
    IntentAssessment,
    Decision,
    TurnPlanning,
    ResponseAdmission,
    ActionAdmission,
    Execution,
    OutcomeObservation,
    HumanModelCandidate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrchestrationStepStateV1 {
    Pending,
    Running,
    WaitingInput,
    WaitingAuthorization,
    WaitingDependency,
    Succeeded,
    Failed,
    Cancelled,
    TimedOut,
    Invalidated,
}

impl OrchestrationStepStateV1 {
    pub const fn may_transition_to(self, next: Self) -> bool {
        use OrchestrationStepStateV1::*;
        matches!(
            (self, next),
            (Pending, Running | Cancelled | TimedOut | Invalidated)
                | (
                    Running,
                    WaitingInput
                        | WaitingAuthorization
                        | WaitingDependency
                        | Succeeded
                        | Failed
                        | Cancelled
                        | TimedOut
                        | Invalidated
                )
                | (
                    WaitingInput | WaitingAuthorization | WaitingDependency,
                    Running | Failed | Cancelled | TimedOut | Invalidated
                )
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "wait_kind", content = "binding")]
pub enum WaitConditionV1 {
    UserInput(ProductionReferenceV1),
    Authorization(HumanCenteredContractRefV1),
    Dependency(HumanCenteredContractRefV1),
    ProviderReconciliation(ProductionReferenceV1),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetryPolicyV1 {
    pub maximum_attempts: u32,
    pub initial_backoff_millis: u64,
    pub maximum_backoff_millis: u64,
}

impl RetryPolicyV1 {
    pub fn validate(&self) -> Result<(), HumanCenteredProtocolError> {
        if self.maximum_attempts == 0
            || self.initial_backoff_millis == 0
            || self.maximum_backoff_millis < self.initial_backoff_millis
        {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrchestrationStepRecordV1 {
    step_id: Uuid,
    run_id: HumanCenteredRunIdV1,
    turn_id: HumanCenteredTurnIdV1,
    step_kind: OrchestrationStepKindV1,
    state: OrchestrationStepStateV1,
    attempt: u32,
    dependency_step_ids: Vec<Uuid>,
    wait_condition: Option<WaitConditionV1>,
    retry_policy: RetryPolicyV1,
    causation_ref: ProductionReferenceV1,
    correlation_ref: ProductionReferenceV1,
}

impl OrchestrationStepRecordV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn pending(
        run_id: HumanCenteredRunIdV1,
        turn_id: HumanCenteredTurnIdV1,
        step_kind: OrchestrationStepKindV1,
        dependency_step_ids: Vec<Uuid>,
        retry_policy: RetryPolicyV1,
        causation_ref: ProductionReferenceV1,
        correlation_ref: ProductionReferenceV1,
    ) -> Result<Self, HumanCenteredProtocolError> {
        let value = Self {
            step_id: Uuid::new_v4(),
            run_id,
            turn_id,
            step_kind,
            state: OrchestrationStepStateV1::Pending,
            attempt: 0,
            dependency_step_ids,
            wait_condition: None,
            retry_policy,
            causation_ref,
            correlation_ref,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn transition(
        &mut self,
        next: OrchestrationStepStateV1,
        wait_condition: Option<WaitConditionV1>,
    ) -> Result<(), HumanCenteredProtocolError> {
        if !self.state.may_transition_to(next) {
            return Err(HumanCenteredProtocolError::InvalidTransition);
        }
        let requires_wait = matches!(
            next,
            OrchestrationStepStateV1::WaitingInput
                | OrchestrationStepStateV1::WaitingAuthorization
                | OrchestrationStepStateV1::WaitingDependency
        );
        if requires_wait != wait_condition.is_some() {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }
        if next == OrchestrationStepStateV1::Running {
            let next_attempt = self
                .attempt
                .checked_add(1)
                .ok_or(HumanCenteredProtocolError::RevisionOverflow)?;
            if next_attempt > self.retry_policy.maximum_attempts {
                return Err(HumanCenteredProtocolError::InvalidTransition);
            }
            self.attempt = next_attempt;
        }
        self.state = next;
        self.wait_condition = wait_condition;
        self.validate()
    }

    pub fn validate(&self) -> Result<(), HumanCenteredProtocolError> {
        self.retry_policy.validate()?;
        let mut dependencies = self.dependency_step_ids.clone();
        dependencies.sort();
        dependencies.dedup();
        let waiting = matches!(
            self.state,
            OrchestrationStepStateV1::WaitingInput
                | OrchestrationStepStateV1::WaitingAuthorization
                | OrchestrationStepStateV1::WaitingDependency
        );
        let wait_matches_state = matches!(
            (&self.state, &self.wait_condition),
            (
                OrchestrationStepStateV1::WaitingInput,
                Some(WaitConditionV1::UserInput(_))
            ) | (
                OrchestrationStepStateV1::WaitingAuthorization,
                Some(WaitConditionV1::Authorization(_))
            ) | (
                OrchestrationStepStateV1::WaitingDependency,
                Some(WaitConditionV1::Dependency(_))
                    | Some(WaitConditionV1::ProviderReconciliation(_))
            )
        );
        if self.step_id.is_nil()
            || self.run_id.is_empty()
            || self.turn_id.is_empty()
            || dependencies.len() != self.dependency_step_ids.len()
            || self.dependency_step_ids.contains(&self.step_id)
            || self.attempt > self.retry_policy.maximum_attempts
            || waiting != self.wait_condition.is_some()
            || waiting && !wait_matches_state
            || self.causation_ref.as_str().trim().is_empty()
            || self.correlation_ref.as_str().trim().is_empty()
        {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }
        Ok(())
    }

    pub fn step_id(&self) -> Uuid {
        self.step_id
    }

    pub fn state(&self) -> OrchestrationStepStateV1 {
        self.state
    }

    pub fn attempt(&self) -> u32 {
        self.attempt
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrchestrationRunRecordV1 {
    pub run_id: HumanCenteredRunIdV1,
    pub state: InteractionRunStateV1,
    pub cancellation_requested: bool,
    pub last_event_sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "command", content = "payload")]
pub enum OrchestrationCommandV1 {
    StartStep(Uuid),
    ResumeStep(Uuid),
    CancelRun(HumanCenteredRunIdV1),
    InvalidateContract(HumanCenteredContractRefV1),
    ReconcileProvider(ProductionReferenceV1),
}

/// Persistence boundary for a future production implementation. Implementors
/// must provide aggregate-version CAS and append the state event atomically.
pub trait OrchestrationRepositoryV1 {
    type Error;

    fn load_run(
        &self,
        run_id: HumanCenteredRunIdV1,
    ) -> Result<Option<OrchestrationRunRecordV1>, Self::Error>;

    fn compare_and_append(
        &self,
        expected_event_sequence: u64,
        run: &OrchestrationRunRecordV1,
        steps: &[OrchestrationStepRecordV1],
        command: &OrchestrationCommandV1,
    ) -> Result<(), Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference(value: &str) -> ProductionReferenceV1 {
        ProductionReferenceV1::new(value).unwrap()
    }

    #[test]
    fn step_waits_are_explicit_and_retry_attempts_are_bounded() {
        let mut step = OrchestrationStepRecordV1::pending(
            HumanCenteredRunIdV1::mint(),
            HumanCenteredTurnIdV1::mint(),
            OrchestrationStepKindV1::ActionAdmission,
            Vec::new(),
            RetryPolicyV1 {
                maximum_attempts: 1,
                initial_backoff_millis: 100,
                maximum_backoff_millis: 1_000,
            },
            reference("cause:turn-plan"),
            reference("correlation:run"),
        )
        .unwrap();
        step.transition(OrchestrationStepStateV1::Running, None)
            .unwrap();
        step.transition(
            OrchestrationStepStateV1::WaitingInput,
            Some(WaitConditionV1::UserInput(reference("input:authorization"))),
        )
        .unwrap();
        assert_eq!(step.attempt(), 1);
        assert!(step
            .transition(OrchestrationStepStateV1::Running, None)
            .is_err());
    }
}
