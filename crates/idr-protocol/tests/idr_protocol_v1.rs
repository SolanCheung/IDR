use idr_protocol::human_centered::*;
use idr_protocol::{ProductionAuthorityContextV1, ProductionReferenceV1};

fn pref(value: &str) -> ProductionReferenceV1 {
    ProductionReferenceV1::new(value).unwrap()
}

fn context() -> ProductionAuthorityContextV1 {
    ProductionAuthorityContextV1::new(
        pref("subject:user-001"),
        pref("user:user-001"),
        pref("caller:interaction-runtime"),
        pref("tenant:test"),
        pref("scope:supplier-management"),
        pref("purpose:supplier-review"),
        pref("operation:prepare-supplier-suspension"),
    )
}

fn initial_metadata(
    contract_kind: HumanCenteredContractKindV1,
    run_id: HumanCenteredRunIdV1,
    turn_id: HumanCenteredTurnIdV1,
) -> HumanCenteredContractMetadataV1 {
    HumanCenteredContractMetadataV1::initial(
        contract_kind,
        run_id,
        turn_id,
        context(),
        vec![pref("evidence:user-request")],
        Vec::new(),
        pref("policy:human-centered:v1"),
        1_000,
    )
    .unwrap()
}

fn simple_intent(run_id: HumanCenteredRunIdV1, turn_id: HumanCenteredTurnIdV1) -> IntentContractV1 {
    let hypothesis = RankedIntentHypothesisV1::new(
        IntentHypothesisV1::new(
            pref("intent:read-supplier"),
            "read supplier",
            UnitIntervalBasisPointsV1::FULL,
            vec![pref("evidence:user-request")],
            Vec::new(),
        )
        .unwrap(),
        1,
    )
    .unwrap();
    IntentContractV1::issue(
        initial_metadata(HumanCenteredContractKindV1::Intent, run_id, turn_id),
        pref("intent:user-stated"),
        vec![hypothesis.clone()],
        vec![hypothesis],
        pref("intent:read-supplier"),
        UnitIntervalBasisPointsV1::FULL,
        false,
        false,
        Vec::new(),
        IntentResolutionMethodV1::FullPath,
    )
    .unwrap()
}

#[test]
fn fast_path_requires_every_deterministic_condition() {
    let allowed = evaluate_intent_fast_path(&IntentFastPathFactsV1 {
        deterministic_command_match: true,
        required_parameters_complete: true,
        ambiguity_present: false,
        authority_context_valid: true,
        policy_allows_request: true,
        impact_level: ImpactLevelV1::Low,
        reversible: true,
        depends_on_human_model: false,
    });
    assert_eq!(allowed.outcome, IntentFastPathOutcomeV1::AllowFastPath);

    let ambiguous = evaluate_intent_fast_path(&IntentFastPathFactsV1 {
        ambiguity_present: true,
        ..IntentFastPathFactsV1 {
            deterministic_command_match: true,
            required_parameters_complete: true,
            ambiguity_present: false,
            authority_context_valid: true,
            policy_allows_request: true,
            impact_level: ImpactLevelV1::Low,
            reversible: true,
            depends_on_human_model: false,
        }
    });
    assert_eq!(ambiguous.outcome, IntentFastPathOutcomeV1::Unknown);

    let denied = evaluate_intent_fast_path(&IntentFastPathFactsV1 {
        policy_allows_request: false,
        ..IntentFastPathFactsV1 {
            deterministic_command_match: true,
            required_parameters_complete: true,
            ambiguity_present: false,
            authority_context_valid: true,
            policy_allows_request: true,
            impact_level: ImpactLevelV1::Low,
            reversible: true,
            depends_on_human_model: false,
        }
    });
    assert_eq!(denied.outcome, IntentFastPathOutcomeV1::Block);
}

#[test]
fn unknown_decision_necessity_conservatively_enters_runtime() {
    let decision = evaluate_decision_necessity(&DecisionNecessityFactsV1 {
        multiple_viable_options: false,
        material_tradeoffs: false,
        evidence_conflict: false,
        impact_level: ImpactLevelV1::Low,
        irreversible_result: false,
        affects_long_term_goal: false,
        host_user_interest_conflict: false,
        simple_lookup: false,
        unique_legal_operation: false,
        user_choice_already_explicit: false,
    });
    assert_eq!(decision.outcome, DecisionNecessityOutcomeV1::Unknown);
    assert!(decision.enters_decision_runtime);
}

#[test]
fn cognitive_service_can_only_create_a_human_model_candidate() {
    let candidate = evaluate_human_model_update_gate(&HumanModelUpdateFactsV1 {
        proposed_by_cognitive_service: true,
        explicit_user_confirmation: false,
        independent_supporting_observations: 1,
        outcome_support_present: false,
        contradiction_present: false,
        inference_policy_allows: true,
        sensitive_attribute_inference: false,
    });
    assert_eq!(
        candidate.outcome(),
        HumanModelUpdateGateOutcomeV1::StoreCandidate
    );

    let confirmed = evaluate_human_model_update_gate(&HumanModelUpdateFactsV1 {
        explicit_user_confirmation: true,
        ..HumanModelUpdateFactsV1 {
            proposed_by_cognitive_service: true,
            explicit_user_confirmation: false,
            independent_supporting_observations: 1,
            outcome_support_present: false,
            contradiction_present: false,
            inference_policy_allows: true,
            sensitive_attribute_inference: false,
        }
    });
    assert_eq!(
        confirmed.outcome(),
        HumanModelUpdateGateOutcomeV1::PromoteUserConfirmed
    );
}

#[test]
fn inactive_human_model_assertions_have_no_effective_value() {
    let gate = evaluate_human_model_update_gate(&HumanModelUpdateFactsV1 {
        proposed_by_cognitive_service: true,
        explicit_user_confirmation: true,
        independent_supporting_observations: 1,
        outcome_support_present: false,
        contradiction_present: false,
        inference_policy_allows: true,
        sensitive_attribute_inference: false,
    });
    let issue = |lifecycle_status| {
        HumanModelAssertionV1::issue(
            initial_metadata(
                HumanCenteredContractKindV1::HumanModelAssertion,
                HumanCenteredRunIdV1::mint(),
                HumanCenteredTurnIdV1::mint(),
            ),
            pref("subject:user-001"),
            HumanModelAssertionTypeV1::InteractionProfile,
            "preferred_response_density",
            HumanModelValueV1::Enum("concise".to_string()),
            "User prefers concise responses",
            HumanModelScopeV1 {
                domains: Vec::new(),
                task_types: Vec::new(),
                global: true,
            },
            HumanModelPersistenceScopeV1::LongTerm,
            &gate,
            lifecycle_status,
            UnitIntervalBasisPointsV1::FULL,
            HumanModelSourceTypeV1::ExplicitUserStatement,
            vec![pref("input:user-confirmation")],
            vec![pref("evidence:user-confirmation")],
            Vec::new(),
            HumanModelCorrectionStateV1::Confirmed,
            None,
            100,
            100,
            Some(200),
            pref("decay:none"),
            HumanModelUsagePolicyV1 {
                allowed_uses: vec!["response_style".to_string()],
                forbidden_uses: Vec::new(),
                maximum_impact_level: ImpactLevelV1::Low,
            },
        )
        .unwrap()
    };

    let active = issue(HumanModelLifecycleStatusV1::Active);
    assert!(active.effective_value(150).is_some());
    assert!(active.effective_value(200).is_none());
    for lifecycle in [
        HumanModelLifecycleStatusV1::Rejected,
        HumanModelLifecycleStatusV1::Expired,
        HumanModelLifecycleStatusV1::Superseded,
    ] {
        assert!(issue(lifecycle).effective_value(150).is_none());
    }
}

#[test]
fn authorization_is_bound_to_exact_action_revision_and_parameters() {
    let run_id = HumanCenteredRunIdV1::mint();
    let turn_id = HumanCenteredTurnIdV1::mint();
    let decision_ref = HumanCenteredContractRefV1::new(
        HumanCenteredContractKindV1::Decision,
        HumanCenteredContractIdV1::mint(),
        1,
        "d".repeat(64),
    )
    .unwrap();
    let action_v1 = ActionContractV1::issue(
        HumanCenteredContractMetadataV1::initial(
            HumanCenteredContractKindV1::Action,
            run_id,
            turn_id,
            context(),
            vec![pref("evidence:user-request")],
            vec![decision_ref.clone()],
            pref("policy:human-centered:v1"),
            1_000,
        )
        .unwrap(),
        Some(decision_ref.clone()),
        pref("operation:suspend-supplier"),
        serde_json::json!({"supplier_id": "supplier-c"}),
        vec!["supplier_status == ACTIVE".to_string()],
        vec![pref("authority:procurement-write")],
        ActionAuthorizationStateV1::Pending,
        ImpactLevelV1::High,
        true,
        "supplier-c:suspend:v1",
        vec!["supplier status is SUSPENDED".to_string()],
        vec![pref("operation:resume-supplier")],
    )
    .unwrap();
    let authorization = ExactActionAuthorizationV1::issue(
        &action_v1,
        pref("user:user-001"),
        AuthorizationDecisionV1::Approve,
        pref("scope:supplier-management"),
        100,
        200,
    )
    .unwrap();
    authorization.validate_for(&action_v1, 150).unwrap();

    let action_v2_metadata = HumanCenteredContractMetadataV1::successor(
        HumanCenteredContractKindV1::Action,
        action_v1.record_ref().unwrap(),
        run_id,
        turn_id,
        context(),
        vec![
            pref("evidence:user-request"),
            pref("evidence:new-supplier-state"),
        ],
        vec![decision_ref.clone()],
        pref("policy:human-centered:v1"),
        1_000,
    )
    .unwrap();
    let action_v2 = ActionContractV1::issue(
        action_v2_metadata,
        Some(decision_ref),
        pref("operation:suspend-supplier"),
        serde_json::json!({"supplier_id": "supplier-b"}),
        vec!["supplier_status == ACTIVE".to_string()],
        vec![pref("authority:procurement-write")],
        ActionAuthorizationStateV1::Pending,
        ImpactLevelV1::High,
        true,
        "supplier-b:suspend:v2",
        vec!["supplier status is SUSPENDED".to_string()],
        vec![pref("operation:resume-supplier")],
    )
    .unwrap();

    assert!(authorization.validate_for(&action_v2, 150).is_err());
}

#[test]
fn invalid_run_state_transitions_are_rejected() {
    assert!(validate_interaction_run_transition(
        InteractionRunStateV1::Pending,
        InteractionRunStateV1::Running
    )
    .is_ok());
    assert!(validate_interaction_run_transition(
        InteractionRunStateV1::Succeeded,
        InteractionRunStateV1::Running
    )
    .is_err());
}

#[test]
fn execution_receipt_is_exactly_bound_to_action_and_outcome() {
    let run_id = HumanCenteredRunIdV1::mint();
    let turn_id = HumanCenteredTurnIdV1::mint();
    let action = ActionContractV1::issue(
        initial_metadata(HumanCenteredContractKindV1::Action, run_id, turn_id),
        None,
        pref("operation:read-supplier"),
        serde_json::json!({"supplier_id": "supplier-c"}),
        vec!["supplier exists".to_string()],
        vec![pref("authority:procurement-read")],
        ActionAuthorizationStateV1::NotRequired,
        ImpactLevelV1::Low,
        true,
        "supplier-c:read:v1",
        vec!["supplier snapshot schema validated".to_string()],
        vec![pref("operation:restore-supplier-snapshot")],
    )
    .unwrap();
    let action_ref = action.record_ref().unwrap();
    let receipt_metadata = HumanCenteredContractMetadataV1::initial(
        HumanCenteredContractKindV1::ExecutionReceipt,
        run_id,
        turn_id,
        context(),
        vec![pref("evidence:provider-response")],
        vec![action_ref.clone()],
        pref("policy:human-centered:v1"),
        1_000,
    )
    .unwrap();
    let receipt = ExecutionReceiptV1::issue(
        receipt_metadata,
        &action,
        HumanCenteredProofIdV1::mint(),
        uuid::Uuid::new_v4(),
        pref("provider:procurement-system"),
        pref("owner:executor-1"),
        "a".repeat(64),
        Some(pref("external-execution:read-001")),
        1,
        100,
        110,
        ExecutionStateV1::Succeeded,
        Some(serde_json::json!({"status": "ACTIVE"})),
        Vec::new(),
        vec!["response schema valid".to_string()],
        Vec::new(),
    )
    .unwrap();
    let receipt_ref = receipt.record_ref().unwrap();
    let outcome_metadata = HumanCenteredContractMetadataV1::initial(
        HumanCenteredContractKindV1::Outcome,
        run_id,
        turn_id,
        context(),
        vec![pref("evidence:observed-result")],
        vec![action_ref.clone(), receipt_ref.clone()],
        pref("policy:human-centered:v1"),
        1_000,
    )
    .unwrap();
    let outcome = OutcomeRecordV1::issue(
        outcome_metadata,
        None,
        Some(action_ref),
        Some(receipt_ref),
        "observe current supplier state",
        OutcomeStateV1::Observed,
        vec!["supplier is active".to_string()],
        vec!["status was returned".to_string()],
        Vec::new(),
        OutcomeAttributionV1::ExecutionQuality,
        UnitIntervalBasisPointsV1::FULL,
        vec![pref("evidence:provider-response")],
        false,
    )
    .unwrap();

    assert_eq!(
        receipt.action_ref().kind(),
        HumanCenteredContractKindV1::Action
    );
    outcome.validate().unwrap();
}

#[test]
fn action_admission_requires_a_valid_exact_approval() {
    let run_id = HumanCenteredRunIdV1::mint();
    let turn_id = HumanCenteredTurnIdV1::mint();
    let decision_ref = HumanCenteredContractRefV1::new(
        HumanCenteredContractKindV1::Decision,
        HumanCenteredContractIdV1::mint(),
        1,
        "e".repeat(64),
    )
    .unwrap();
    let action = ActionContractV1::issue(
        HumanCenteredContractMetadataV1::initial(
            HumanCenteredContractKindV1::Action,
            run_id,
            turn_id,
            context(),
            vec![pref("evidence:user-request")],
            vec![decision_ref.clone()],
            pref("policy:human-centered:v1"),
            1_000,
        )
        .unwrap(),
        Some(decision_ref),
        pref("operation:suspend-supplier"),
        serde_json::json!({"supplier_id": "supplier-c"}),
        vec!["supplier_status == ACTIVE".to_string()],
        vec![pref("authority:procurement-write")],
        ActionAuthorizationStateV1::Pending,
        ImpactLevelV1::High,
        true,
        "supplier-c:suspend:v1",
        vec!["supplier status is SUSPENDED".to_string()],
        vec![pref("operation:resume-supplier")],
    )
    .unwrap();
    let facts = ActionAdmissionFactsV1 {
        capability_registered: true,
        capability_enabled: true,
        actor_authority_valid: true,
        agent_authority_valid: true,
        parameters_valid: true,
        preconditions_satisfied: true,
        policy_allows: true,
        verification_available: true,
        compensation_available_if_required: true,
        dependencies_current: true,
    };

    let waiting = evaluate_action_admission(&action, &facts, None, 150);
    assert_eq!(waiting.outcome, ActionAdmissionOutcomeV1::WaitAuthorization);

    let denied = ExactActionAuthorizationV1::issue(
        &action,
        pref("user:user-001"),
        AuthorizationDecisionV1::Deny,
        pref("scope:supplier-management"),
        100,
        200,
    )
    .unwrap();
    let rejected = evaluate_action_admission(&action, &facts, Some(&denied), 150);
    assert_eq!(rejected.outcome, ActionAdmissionOutcomeV1::Reject);

    let approved = ExactActionAuthorizationV1::issue(
        &action,
        pref("user:user-001"),
        AuthorizationDecisionV1::Approve,
        pref("scope:supplier-management"),
        100,
        200,
    )
    .unwrap();
    let admitted = evaluate_action_admission(&action, &facts, Some(&approved), 150);
    assert_eq!(admitted.outcome, ActionAdmissionOutcomeV1::Admit);

    let expired = evaluate_action_admission(&action, &facts, Some(&approved), 201);
    assert_eq!(expired.outcome, ActionAdmissionOutcomeV1::Reject);
}

#[test]
fn rendered_response_requires_post_render_policy_admission() {
    let run_id = HumanCenteredRunIdV1::mint();
    let turn_id = HumanCenteredTurnIdV1::mint();
    let intent = simple_intent(run_id, turn_id);
    let intent_ref = intent.record_ref().unwrap();
    let response_metadata = HumanCenteredContractMetadataV1::initial(
        HumanCenteredContractKindV1::Response,
        run_id,
        turn_id,
        context(),
        vec![pref("evidence:user-request")],
        vec![intent_ref.clone()],
        pref("policy:human-centered:v1"),
        1_000,
    )
    .unwrap();
    let response = ResponseContractV1::issue(
        response_metadata,
        vec![intent_ref],
        ResponsePhaseV1::Final,
        ResponseTypeV1::Answer,
        vec!["state the supplier status".to_string()],
        vec![pref("evidence:supplier-snapshot")],
        ExplanationDepthV1::Low,
        TerminologyLevelV1::Plain,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let rendered_content = "Supplier C is currently active.";
    let rendered = RenderedResponseEnvelopeV1::new(
        &response,
        rendered_content,
        pref("renderer:interaction-ui"),
        100,
    )
    .unwrap();
    let allowed = evaluate_rendered_response_admission(
        &response,
        &rendered,
        rendered_content,
        &ResponsePolicyFactsV1 {
            policy_evaluation_ref: pref("policy-evaluation:response-001"),
            policy_revision_ref: pref("policy:human-centered:v1"),
            evaluated_response_ref: response.record_ref().unwrap(),
            evaluated_content_digest: rendered.content_digest().to_string(),
            evaluated_at: 101,
            expires_at: 200,
            content_policy_allows: true,
            evidence_traceable: true,
            required_uncertainty_present: true,
            prohibited_content_absent: true,
            authority_scope_respected: true,
        },
        150,
    );
    assert_eq!(allowed.outcome, ResponseAdmissionOutcomeV1::Allow);

    let blocked = evaluate_rendered_response_admission(
        &response,
        &rendered,
        rendered_content,
        &ResponsePolicyFactsV1 {
            policy_evaluation_ref: pref("policy-evaluation:response-002"),
            policy_revision_ref: pref("policy:human-centered:v1"),
            evaluated_response_ref: response.record_ref().unwrap(),
            evaluated_content_digest: rendered.content_digest().to_string(),
            evaluated_at: 101,
            expires_at: 200,
            content_policy_allows: false,
            evidence_traceable: true,
            required_uncertainty_present: true,
            prohibited_content_absent: true,
            authority_scope_respected: true,
        },
        150,
    );
    assert_eq!(blocked.outcome, ResponseAdmissionOutcomeV1::Block);

    let replayed_after_expiry = evaluate_rendered_response_admission(
        &response,
        &rendered,
        rendered_content,
        &ResponsePolicyFactsV1 {
            policy_evaluation_ref: pref("policy-evaluation:response-003"),
            policy_revision_ref: pref("policy:human-centered:v1"),
            evaluated_response_ref: response.record_ref().unwrap(),
            evaluated_content_digest: rendered.content_digest().to_string(),
            evaluated_at: 101,
            expires_at: 200,
            content_policy_allows: true,
            evidence_traceable: true,
            required_uncertainty_present: true,
            prohibited_content_absent: true,
            authority_scope_respected: true,
        },
        200,
    );
    assert_eq!(
        replayed_after_expiry.outcome,
        ResponseAdmissionOutcomeV1::Block
    );
    assert_eq!(
        replayed_after_expiry.reason_codes,
        vec!["response_send_time_invalid"]
    );
}

#[test]
fn constrained_types_reject_invalid_wire_values() {
    assert!(serde_json::from_str::<ProductionReferenceV1>("\"\\n\"").is_err());
    assert!(serde_json::from_str::<HumanCenteredRunIdV1>(
        "\"00000000-0000-0000-0000-000000000000\""
    )
    .is_err());
    assert!(serde_json::from_str::<UnitIntervalBasisPointsV1>("10001").is_err());
    assert!(HumanCenteredContractRefV1::new(
        HumanCenteredContractKindV1::Intent,
        HumanCenteredContractIdV1::mint(),
        1,
        "A".repeat(64),
    )
    .is_err());
}

#[test]
fn contract_metadata_kind_cannot_be_reused_or_crossed_in_a_revision_chain() {
    let run_id = HumanCenteredRunIdV1::mint();
    let turn_id = HumanCenteredTurnIdV1::mint();
    let intent = simple_intent(run_id, turn_id);
    let intent_ref = intent.record_ref().unwrap();

    let action_with_intent_metadata = ActionContractV1::issue(
        initial_metadata(HumanCenteredContractKindV1::Intent, run_id, turn_id),
        None,
        pref("operation:read-supplier"),
        serde_json::json!({"supplier_id": "supplier-c"}),
        Vec::new(),
        Vec::new(),
        ActionAuthorizationStateV1::NotRequired,
        ImpactLevelV1::Low,
        false,
        "supplier-c:read:wrong-kind",
        vec!["result schema validated".to_string()],
        Vec::new(),
    );
    assert!(matches!(
        action_with_intent_metadata,
        Err(HumanCenteredProtocolError::ContractKindMismatch)
    ));

    let crossed_successor = HumanCenteredContractMetadataV1::successor(
        HumanCenteredContractKindV1::Action,
        intent_ref,
        run_id,
        turn_id,
        context(),
        Vec::new(),
        Vec::new(),
        pref("policy:human-centered:v1"),
        1_000,
    );
    assert!(matches!(
        crossed_successor,
        Err(HumanCenteredProtocolError::ContractKindMismatch)
    ));
}

#[test]
fn action_contract_expiry_is_enforced_independently_of_authorization_expiry() {
    let run_id = HumanCenteredRunIdV1::mint();
    let turn_id = HumanCenteredTurnIdV1::mint();
    let decision_ref = HumanCenteredContractRefV1::new(
        HumanCenteredContractKindV1::Decision,
        HumanCenteredContractIdV1::mint(),
        1,
        "f".repeat(64),
    )
    .unwrap();
    let action = ActionContractV1::issue(
        HumanCenteredContractMetadataV1::initial(
            HumanCenteredContractKindV1::Action,
            run_id,
            turn_id,
            context(),
            vec![pref("evidence:user-request")],
            vec![decision_ref.clone()],
            pref("policy:human-centered:v1"),
            1_000,
        )
        .unwrap(),
        Some(decision_ref),
        pref("operation:suspend-supplier"),
        serde_json::json!({"supplier_id": "supplier-c"}),
        Vec::new(),
        Vec::new(),
        ActionAuthorizationStateV1::Pending,
        ImpactLevelV1::High,
        true,
        "supplier-c:suspend:expiry-test",
        vec!["supplier status is suspended".to_string()],
        vec![pref("operation:resume-supplier")],
    )
    .unwrap();
    let authorization = ExactActionAuthorizationV1::issue(
        &action,
        pref("user:user-001"),
        AuthorizationDecisionV1::Approve,
        pref("scope:supplier-management"),
        900,
        1_100,
    )
    .unwrap();

    let facts = ActionAdmissionFactsV1 {
        capability_registered: true,
        capability_enabled: true,
        actor_authority_valid: true,
        agent_authority_valid: true,
        parameters_valid: true,
        preconditions_satisfied: true,
        policy_allows: true,
        verification_available: true,
        compensation_available_if_required: true,
        dependencies_current: true,
    };
    let decision = evaluate_action_admission(&action, &facts, Some(&authorization), 1_001);
    assert_eq!(decision.outcome, ActionAdmissionOutcomeV1::Reject);
    assert_eq!(decision.reason_codes, vec!["action_contract_expired"]);
}

#[test]
fn execution_permit_proof_rejects_denial_expiry_overrun_and_binding_changes() {
    use ed25519_dalek::{Signer, SigningKey};
    use std::collections::BTreeSet;
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let run_id = HumanCenteredRunIdV1::mint();
    let turn_id = HumanCenteredTurnIdV1::mint();
    let decision_ref = HumanCenteredContractRefV1::new(
        HumanCenteredContractKindV1::Decision,
        HumanCenteredContractIdV1::mint(),
        1,
        "f".repeat(64),
    )
    .unwrap();
    let action = ActionContractV1::issue(
        HumanCenteredContractMetadataV1::initial(
            HumanCenteredContractKindV1::Action,
            run_id,
            turn_id,
            context(),
            vec![pref("evidence:user-request")],
            vec![decision_ref.clone()],
            pref("policy:human-centered:v1"),
            now + 300,
        )
        .unwrap(),
        Some(decision_ref),
        pref("operation:suspend-supplier"),
        serde_json::json!({"supplier_id": "supplier-c"}),
        Vec::new(),
        Vec::new(),
        ActionAuthorizationStateV1::Pending,
        ImpactLevelV1::High,
        true,
        "supplier-c:suspend:permit-test",
        vec!["supplier status is suspended".to_string()],
        vec![pref("operation:resume-supplier")],
    )
    .unwrap();
    let denied = ExactActionAuthorizationV1::issue(
        &action,
        pref("user:user-001"),
        AuthorizationDecisionV1::Deny,
        pref("scope:supplier-management"),
        now - 10,
        now + 120,
    )
    .unwrap();
    assert!(ExecutionPermitRequestV1::new(
        &action,
        &denied,
        pref("provider:supplier-api"),
        pref("owner:executor-1"),
        1,
        now,
        now + 30,
        "a".repeat(64),
    )
    .is_err());

    let approved = ExactActionAuthorizationV1::issue(
        &action,
        pref("user:user-001"),
        AuthorizationDecisionV1::Approve,
        pref("scope:supplier-management"),
        now - 10,
        now + 60,
    )
    .unwrap();
    assert!(approved.validate_for(&action, now + 60).is_err());
    assert!(ExecutionPermitRequestV1::new(
        &action,
        &approved,
        pref("provider:supplier-api"),
        pref("owner:executor-1"),
        1,
        now,
        now + 61,
        "b".repeat(64),
    )
    .is_err());

    let request = ExecutionPermitRequestV1::new(
        &action,
        &approved,
        pref("provider:supplier-api"),
        pref("owner:executor-1"),
        1,
        now,
        now + 30,
        "c".repeat(64),
    )
    .unwrap();
    let signing_key = SigningKey::from_bytes(&[9_u8; 32]);
    let claims = ProofClaimsV1::new(
        ProofKindV1::ExecutionPermit,
        pref("issuer:execution-authority"),
        request.request_ref().clone(),
        request.request_digest(),
        pref("tenant:test"),
        pref("scope:supplier-management"),
        pref("purpose:supplier-review"),
        pref("policy:human-centered:v1"),
        now,
        now + 30,
        request.dispatch_nonce(),
    )
    .unwrap();
    let signature = signing_key.sign(&claims.canonical_signing_bytes().unwrap());
    let signature_hex: String = signature
        .to_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let envelope =
        SignedProofEnvelopeV1::new(claims, pref("key:execution-authority:v1"), signature_hex)
            .unwrap();
    let public_key_hex: String = signing_key
        .verifying_key()
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let trust_root = ProofTrustRootV1::new(vec![IssuerKeyPolicyV1::new(
        pref("issuer:execution-authority"),
        pref("key:execution-authority:v1"),
        &public_key_hex,
        BTreeSet::from([ProofKindV1::ExecutionPermit]),
        pref("tenant:test"),
        now - 20,
        now + 120,
        None,
    )
    .unwrap()])
    .unwrap();
    let expected = request
        .expected_proof_binding(pref("issuer:execution-authority"))
        .unwrap();
    let verified = trust_root.verify_now(&envelope, &expected).unwrap();
    let verified_request =
        verify_execution_permit_request(verified, request.clone(), &action, &approved).unwrap();
    assert_eq!(
        verified_request.request().provider_ref(),
        &pref("provider:supplier-api")
    );

    let changed_provider = ExecutionPermitRequestV1::new(
        &action,
        &approved,
        pref("provider:attacker"),
        pref("owner:executor-1"),
        1,
        now,
        now + 30,
        "c".repeat(64),
    )
    .unwrap();
    let changed_binding = changed_provider
        .expected_proof_binding(pref("issuer:execution-authority"))
        .unwrap();
    assert!(trust_root.verify_now(&envelope, &changed_binding).is_err());
}

#[test]
fn every_irreversible_action_requires_a_decision_reference() {
    let action = ActionContractV1::issue(
        initial_metadata(
            HumanCenteredContractKindV1::Action,
            HumanCenteredRunIdV1::mint(),
            HumanCenteredTurnIdV1::mint(),
        ),
        None,
        pref("operation:irreversible-low-change"),
        serde_json::json!({"resource_id": "resource-1"}),
        Vec::new(),
        Vec::new(),
        ActionAuthorizationStateV1::NotRequired,
        ImpactLevelV1::Low,
        false,
        "irreversible-low-change:v1",
        vec!["result schema validated".to_string()],
        Vec::new(),
    );
    assert!(matches!(
        action,
        Err(HumanCenteredProtocolError::InvalidContract)
    ));
}

#[test]
fn turn_coordination_rejects_cycles_and_misdirected_authorization_gates() {
    let run_id = HumanCenteredRunIdV1::mint();
    let turn_id = HumanCenteredTurnIdV1::mint();
    let response_ref = HumanCenteredContractRefV1::new(
        HumanCenteredContractKindV1::Response,
        HumanCenteredContractIdV1::mint(),
        1,
        "a".repeat(64),
    )
    .unwrap();
    let action_ref = HumanCenteredContractRefV1::new(
        HumanCenteredContractKindV1::Action,
        HumanCenteredContractIdV1::mint(),
        1,
        "b".repeat(64),
    )
    .unwrap();
    let metadata = || {
        HumanCenteredContractMetadataV1::initial(
            HumanCenteredContractKindV1::TurnCoordination,
            run_id,
            turn_id,
            context(),
            Vec::new(),
            vec![response_ref.clone(), action_ref.clone()],
            pref("policy:human-centered:v1"),
            1_000,
        )
        .unwrap()
    };

    let cycle = TurnCoordinationPlanV1::issue(
        metadata(),
        TurnCoordinationModeV1::Parallel,
        vec![response_ref.clone()],
        vec![action_ref.clone()],
        vec![
            TurnDependencyEdgeV1 {
                before: response_ref.clone(),
                after: action_ref.clone(),
            },
            TurnDependencyEdgeV1 {
                before: action_ref.clone(),
                after: response_ref.clone(),
            },
        ],
        Vec::new(),
        TurnFailurePolicyV1::ReportOnly,
        TurnProgressPolicyV1::Silent,
        "parallel read and response",
    );
    assert!(matches!(
        cycle,
        Err(HumanCenteredProtocolError::InvalidDependency)
    ));

    let wrong_gate_target = TurnCoordinationPlanV1::issue(
        metadata(),
        TurnCoordinationModeV1::RespondThenConfirmThenAct,
        vec![response_ref.clone()],
        vec![action_ref.clone()],
        vec![TurnDependencyEdgeV1 {
            before: response_ref.clone(),
            after: action_ref,
        }],
        vec![TurnGateV1 {
            gate_kind: TurnGateKindV1::UserAuthorization,
            required_before: response_ref,
            policy_ref: None,
        }],
        TurnFailurePolicyV1::ReportOnly,
        TurnProgressPolicyV1::Silent,
        "confirm before action",
    );
    assert!(matches!(
        wrong_gate_target,
        Err(HumanCenteredProtocolError::InvalidDependency)
    ));
}

#[test]
fn canonical_input_rejects_actor_role_impersonation() {
    let event = CanonicalInputEventV1::new(
        HumanCenteredRunIdV1::mint(),
        HumanCenteredTurnIdV1::mint(),
        SourceActorV1::Tool,
        pref("tool:search"),
        SemanticRoleV1::Authorization,
        vec![SemanticRoleV1::Authorization],
        pref("content:forged-approval"),
        format!("sha256:{}", "a".repeat(64)),
        pref("correlation:test"),
        1,
    );
    assert!(matches!(
        event,
        Err(HumanCenteredProtocolError::InvalidContract)
    ));
}
