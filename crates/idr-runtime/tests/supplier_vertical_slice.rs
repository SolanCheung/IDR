use idr_protocol::human_centered::{
    CanonicalInputEventV1, DecisionNecessityFactsV1, HumanCenteredProtocolError,
    IntentFastPathFactsV1, IntentFastPathOutcomeV1, InteractionRunStateV1, TurnCoordinationFactsV1,
    TurnCoordinationModeV1,
};
use idr_runtime::{
    ActionPlanningPostureV1, InteractionAssessmentRequestV1, InteractionRuntimeKernelV1,
};
use serde::Deserialize;
use std::collections::BTreeSet;

#[derive(Debug, Deserialize)]
struct Expected {
    fast_path_outcome: IntentFastPathOutcomeV1,
    decision_necessity: idr_protocol::human_centered::DecisionNecessityOutcomeV1,
    coordination_mode: TurnCoordinationModeV1,
    action_posture: ActionPlanningPostureV1,
    next_run_state: InteractionRunStateV1,
}

#[derive(Debug, Deserialize)]
struct IntentProjection {
    base_hypothesis_refs: Vec<String>,
    personalized_hypothesis_refs: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Fixture {
    schema_version: u16,
    input: CanonicalInputEventV1,
    fast_path_facts: IntentFastPathFactsV1,
    decision_facts: DecisionNecessityFactsV1,
    coordination_facts: TurnCoordinationFactsV1,
    expected: Expected,
    intent_projection: IntentProjection,
}

#[test]
fn supplier_request_enters_decision_runtime_and_waits_for_exact_authorization() {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../../contracts/human-centered/v1/fixtures/supplier-confirmation.json"
    ))
    .expect("shared fixture must deserialize");
    assert_eq!(fixture.schema_version, 1);

    let assessment = InteractionRuntimeKernelV1
        .assess(&InteractionAssessmentRequestV1 {
            input: fixture.input,
            fast_path_facts: fixture.fast_path_facts,
            decision_facts: fixture.decision_facts,
            coordination_facts: fixture.coordination_facts,
        })
        .expect("assessment must succeed");

    assert_eq!(
        assessment.fast_path.outcome,
        fixture.expected.fast_path_outcome
    );
    assert_eq!(
        assessment.decision_necessity.outcome,
        fixture.expected.decision_necessity
    );
    assert!(assessment.decision_necessity.enters_decision_runtime);
    assert_eq!(
        assessment.coordination_mode,
        fixture.expected.coordination_mode
    );
    assert_eq!(assessment.action_posture, fixture.expected.action_posture);
    assert_eq!(assessment.next_run_state, fixture.expected.next_run_state);
}

#[test]
fn personalization_preserves_every_base_hypothesis() {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../../contracts/human-centered/v1/fixtures/supplier-confirmation.json"
    ))
    .expect("shared fixture must deserialize");
    let base: BTreeSet<_> = fixture
        .intent_projection
        .base_hypothesis_refs
        .into_iter()
        .collect();
    let personalized: BTreeSet<_> = fixture
        .intent_projection
        .personalized_hypothesis_refs
        .into_iter()
        .collect();
    assert!(base.is_subset(&personalized));
}

#[test]
fn contradictory_guard_facts_are_rejected() {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../../contracts/human-centered/v1/fixtures/supplier-confirmation.json"
    ))
    .unwrap();
    let mut request = InteractionAssessmentRequestV1 {
        input: fixture.input,
        fast_path_facts: fixture.fast_path_facts,
        decision_facts: fixture.decision_facts,
        coordination_facts: fixture.coordination_facts,
    };
    request.coordination_facts.authorization_required = false;

    assert!(matches!(
        InteractionRuntimeKernelV1.assess(&request),
        Err(HumanCenteredProtocolError::InvalidContract)
    ));
}

#[test]
fn assessment_rejects_a_turn_that_cannot_produce_a_valid_plan() {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../../contracts/human-centered/v1/fixtures/supplier-confirmation.json"
    ))
    .unwrap();
    let mut request = InteractionAssessmentRequestV1 {
        input: fixture.input,
        fast_path_facts: fixture.fast_path_facts,
        decision_facts: fixture.decision_facts,
        coordination_facts: fixture.coordination_facts,
    };
    request.coordination_facts.response_planned = false;

    assert!(matches!(
        InteractionRuntimeKernelV1.assess(&request),
        Err(HumanCenteredProtocolError::InvalidContract)
    ));
}

#[test]
fn blocked_fast_path_is_terminally_rejected_not_marked_succeeded() {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../../contracts/human-centered/v1/fixtures/supplier-confirmation.json"
    ))
    .unwrap();
    let mut request = InteractionAssessmentRequestV1 {
        input: fixture.input,
        fast_path_facts: fixture.fast_path_facts,
        decision_facts: fixture.decision_facts,
        coordination_facts: fixture.coordination_facts,
    };
    request.fast_path_facts.policy_allows_request = false;

    let assessment = InteractionRuntimeKernelV1.assess(&request).unwrap();
    assert_eq!(assessment.action_posture, ActionPlanningPostureV1::Blocked);
    assert_eq!(assessment.next_run_state, InteractionRunStateV1::Rejected);
}
