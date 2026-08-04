use super::{HumanCenteredProtocolError, ImpactLevelV1, HUMAN_CENTERED_RULE_VERSION};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentFastPathFactsV1 {
    pub deterministic_command_match: bool,
    pub required_parameters_complete: bool,
    pub ambiguity_present: bool,
    pub authority_context_valid: bool,
    pub policy_allows_request: bool,
    pub impact_level: ImpactLevelV1,
    pub reversible: bool,
    pub depends_on_human_model: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentFastPathOutcomeV1 {
    AllowFastPath,
    Unknown,
    Block,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentFastPathDecisionV1 {
    pub outcome: IntentFastPathOutcomeV1,
    pub reason_codes: Vec<String>,
    pub rule_version: u64,
}

pub fn evaluate_intent_fast_path(facts: &IntentFastPathFactsV1) -> IntentFastPathDecisionV1 {
    if !facts.authority_context_valid || !facts.policy_allows_request {
        let mut reasons = Vec::new();
        if !facts.authority_context_valid {
            reasons.push("authority_context_invalid".to_string());
        }
        if !facts.policy_allows_request {
            reasons.push("policy_denied".to_string());
        }
        return IntentFastPathDecisionV1 {
            outcome: IntentFastPathOutcomeV1::Block,
            reason_codes: reasons,
            rule_version: HUMAN_CENTERED_RULE_VERSION,
        };
    }

    let allowed = facts.deterministic_command_match
        && facts.required_parameters_complete
        && !facts.ambiguity_present
        && facts.impact_level == ImpactLevelV1::Low
        && facts.reversible
        && !facts.depends_on_human_model;
    if allowed {
        return IntentFastPathDecisionV1 {
            outcome: IntentFastPathOutcomeV1::AllowFastPath,
            reason_codes: vec!["all_fast_path_conditions_satisfied".to_string()],
            rule_version: HUMAN_CENTERED_RULE_VERSION,
        };
    }

    let mut reasons = Vec::new();
    if !facts.deterministic_command_match {
        reasons.push("no_deterministic_command_match".to_string());
    }
    if !facts.required_parameters_complete {
        reasons.push("parameters_incomplete".to_string());
    }
    if facts.ambiguity_present {
        reasons.push("ambiguity_present".to_string());
    }
    if facts.impact_level != ImpactLevelV1::Low {
        reasons.push("impact_above_low".to_string());
    }
    if !facts.reversible {
        reasons.push("operation_not_reversible".to_string());
    }
    if facts.depends_on_human_model {
        reasons.push("human_model_dependency_present".to_string());
    }
    IntentFastPathDecisionV1 {
        outcome: IntentFastPathOutcomeV1::Unknown,
        reason_codes: reasons,
        rule_version: HUMAN_CENTERED_RULE_VERSION,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionNecessityFactsV1 {
    pub multiple_viable_options: bool,
    pub material_tradeoffs: bool,
    pub evidence_conflict: bool,
    pub impact_level: ImpactLevelV1,
    pub irreversible_result: bool,
    pub affects_long_term_goal: bool,
    pub host_user_interest_conflict: bool,
    pub simple_lookup: bool,
    pub unique_legal_operation: bool,
    pub user_choice_already_explicit: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionNecessityOutcomeV1 {
    NotRequired,
    Required,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionNecessityDecisionV1 {
    pub outcome: DecisionNecessityOutcomeV1,
    pub enters_decision_runtime: bool,
    pub reason_codes: Vec<String>,
    pub rule_version: u64,
}

pub fn evaluate_decision_necessity(
    facts: &DecisionNecessityFactsV1,
) -> DecisionNecessityDecisionV1 {
    let required = facts.multiple_viable_options
        || facts.material_tradeoffs
        || facts.evidence_conflict
        || facts.impact_level.rank() >= ImpactLevelV1::High.rank()
        || facts.irreversible_result
        || facts.affects_long_term_goal
        || facts.host_user_interest_conflict;
    if required {
        let mut reasons = Vec::new();
        if facts.multiple_viable_options {
            reasons.push("multiple_viable_options".to_string());
        }
        if facts.material_tradeoffs {
            reasons.push("material_tradeoffs".to_string());
        }
        if facts.evidence_conflict {
            reasons.push("evidence_conflict".to_string());
        }
        if facts.impact_level.rank() >= ImpactLevelV1::High.rank() {
            reasons.push("high_impact".to_string());
        }
        if facts.irreversible_result {
            reasons.push("irreversible_result".to_string());
        }
        if facts.affects_long_term_goal {
            reasons.push("long_term_goal_affected".to_string());
        }
        if facts.host_user_interest_conflict {
            reasons.push("host_user_interest_conflict".to_string());
        }
        return DecisionNecessityDecisionV1 {
            outcome: DecisionNecessityOutcomeV1::Required,
            enters_decision_runtime: true,
            reason_codes: reasons,
            rule_version: HUMAN_CENTERED_RULE_VERSION,
        };
    }

    if facts.simple_lookup || facts.unique_legal_operation || facts.user_choice_already_explicit {
        return DecisionNecessityDecisionV1 {
            outcome: DecisionNecessityOutcomeV1::NotRequired,
            enters_decision_runtime: false,
            reason_codes: vec!["no_material_decision_remains".to_string()],
            rule_version: HUMAN_CENTERED_RULE_VERSION,
        };
    }

    DecisionNecessityDecisionV1 {
        outcome: DecisionNecessityOutcomeV1::Unknown,
        enters_decision_runtime: true,
        reason_codes: vec!["insufficient_facts_conservative_entry".to_string()],
        rule_version: HUMAN_CENTERED_RULE_VERSION,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TurnCoordinationFactsV1 {
    pub response_planned: bool,
    pub action_planned: bool,
    pub explicit_confirmation_requested: bool,
    pub authorization_required: bool,
    pub action_is_read_only: bool,
    pub long_running: bool,
    pub stream_progress: bool,
    pub safe_to_parallelize: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnCoordinationModeV1 {
    RespondOnly,
    ActThenRespond,
    RespondThenAct,
    RespondThenConfirmThenAct,
    Parallel,
    AcknowledgeThenRun,
    StreamProgressThenFinal,
}

pub fn select_turn_coordination_mode(
    facts: &TurnCoordinationFactsV1,
) -> Result<TurnCoordinationModeV1, HumanCenteredProtocolError> {
    // Every V1 turn plan requires at least one response contract. Rejecting the
    // unrepresentable state here prevents callers from obtaining an assessment
    // that cannot subsequently be encoded as a valid coordination plan.
    if !facts.response_planned {
        return Err(HumanCenteredProtocolError::InvalidContract);
    }
    if !facts.action_planned {
        return Ok(TurnCoordinationModeV1::RespondOnly);
    }
    if facts.explicit_confirmation_requested || facts.authorization_required {
        return Ok(TurnCoordinationModeV1::RespondThenConfirmThenAct);
    }
    if facts.stream_progress {
        return Ok(TurnCoordinationModeV1::StreamProgressThenFinal);
    }
    if facts.long_running {
        return Ok(TurnCoordinationModeV1::AcknowledgeThenRun);
    }
    if facts.action_is_read_only && facts.safe_to_parallelize {
        return Ok(TurnCoordinationModeV1::Parallel);
    }
    Ok(TurnCoordinationModeV1::RespondThenAct)
}
