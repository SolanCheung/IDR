//! Legacy deterministic assessment kernel used only by dev/test/shadow builds.

use idr_protocol::human_centered::{
    evaluate_decision_necessity, evaluate_intent_fast_path, select_turn_coordination_mode,
    CanonicalInputEventV1, DecisionNecessityDecisionV1, DecisionNecessityFactsV1,
    HumanCenteredProtocolError, IntentFastPathDecisionV1, IntentFastPathFactsV1,
    IntentFastPathOutcomeV1, InteractionRunStateV1, TurnCoordinationFactsV1,
    TurnCoordinationModeV1, HUMAN_CENTERED_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionAssessmentRequestV1 {
    pub input: CanonicalInputEventV1,
    pub fast_path_facts: IntentFastPathFactsV1,
    pub decision_facts: DecisionNecessityFactsV1,
    pub coordination_facts: TurnCoordinationFactsV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionPlanningPostureV1 {
    None,
    Blocked,
    Planned,
    WaitingAuthorization,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionAssessmentV1 {
    pub schema_version: u16,
    pub fast_path: IntentFastPathDecisionV1,
    pub decision_necessity: DecisionNecessityDecisionV1,
    pub coordination_mode: TurnCoordinationModeV1,
    pub action_posture: ActionPlanningPostureV1,
    pub next_run_state: InteractionRunStateV1,
}

#[derive(Debug, Default)]
pub struct InteractionRuntimeKernelV1;

impl InteractionRuntimeKernelV1 {
    pub fn assess(
        &self,
        request: &InteractionAssessmentRequestV1,
    ) -> Result<InteractionAssessmentV1, HumanCenteredProtocolError> {
        request.input.validate()?;
        validate_assessment_facts(request)?;
        let fast_path = evaluate_intent_fast_path(&request.fast_path_facts);
        let decision_necessity = evaluate_decision_necessity(&request.decision_facts);
        if fast_path.outcome == IntentFastPathOutcomeV1::AllowFastPath
            && decision_necessity.enters_decision_runtime
        {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }

        if fast_path.outcome == IntentFastPathOutcomeV1::Block {
            return Ok(InteractionAssessmentV1 {
                schema_version: HUMAN_CENTERED_SCHEMA_VERSION,
                fast_path,
                decision_necessity,
                coordination_mode: TurnCoordinationModeV1::RespondOnly,
                action_posture: ActionPlanningPostureV1::Blocked,
                next_run_state: InteractionRunStateV1::Rejected,
            });
        }

        let coordination_mode = select_turn_coordination_mode(&request.coordination_facts)?;
        let (action_posture, next_run_state) = match coordination_mode {
            TurnCoordinationModeV1::RespondOnly => (
                ActionPlanningPostureV1::None,
                InteractionRunStateV1::Running,
            ),
            TurnCoordinationModeV1::RespondThenConfirmThenAct => (
                ActionPlanningPostureV1::WaitingAuthorization,
                InteractionRunStateV1::WaitingAuthorization,
            ),
            _ => (
                ActionPlanningPostureV1::Planned,
                InteractionRunStateV1::Running,
            ),
        };
        Ok(InteractionAssessmentV1 {
            schema_version: HUMAN_CENTERED_SCHEMA_VERSION,
            fast_path,
            decision_necessity,
            coordination_mode,
            action_posture,
            next_run_state,
        })
    }
}

fn validate_assessment_facts(
    request: &InteractionAssessmentRequestV1,
) -> Result<(), HumanCenteredProtocolError> {
    let coordination = &request.coordination_facts;
    if request.fast_path_facts.impact_level != request.decision_facts.impact_level
        || !coordination.response_planned
        || coordination.authorization_required && !coordination.action_planned
        || coordination.explicit_confirmation_requested && !coordination.action_planned
        || coordination.action_planned
            && request.decision_facts.impact_level.rank()
                >= idr_protocol::human_centered::ImpactLevelV1::High.rank()
            && !coordination.authorization_required
        || coordination.safe_to_parallelize && !coordination.action_is_read_only
        || coordination.stream_progress && !coordination.action_planned
    {
        return Err(HumanCenteredProtocolError::InvalidContract);
    }
    Ok(())
}
