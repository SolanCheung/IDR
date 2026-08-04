use super::HumanCenteredProtocolError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionRunStateV1 {
    Pending,
    Running,
    WaitingInput,
    WaitingAuthorization,
    WaitingDependency,
    Succeeded,
    Rejected,
    Failed,
    Cancelled,
    TimedOut,
    Invalidated,
}

impl InteractionRunStateV1 {
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded
                | Self::Rejected
                | Self::Failed
                | Self::Cancelled
                | Self::TimedOut
                | Self::Invalidated
        )
    }

    pub const fn may_transition_to(self, next: Self) -> bool {
        use InteractionRunStateV1::*;
        matches!(
            (self, next),
            (
                Pending,
                Running | Rejected | Cancelled | TimedOut | Invalidated
            ) | (
                Running,
                WaitingInput
                    | WaitingAuthorization
                    | WaitingDependency
                    | Succeeded
                    | Rejected
                    | Failed
                    | Cancelled
                    | TimedOut
                    | Invalidated
            ) | (
                WaitingInput | WaitingAuthorization | WaitingDependency,
                Running | Rejected | Failed | Cancelled | TimedOut | Invalidated
            )
        )
    }
}

pub fn validate_interaction_run_transition(
    current: InteractionRunStateV1,
    next: InteractionRunStateV1,
) -> Result<(), HumanCenteredProtocolError> {
    if !current.may_transition_to(next) {
        return Err(HumanCenteredProtocolError::InvalidTransition);
    }
    Ok(())
}
