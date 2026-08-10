use async_trait::async_trait;
use idr_core::*;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};

fn scope(domain: &str) -> ScopeV1 {
    ScopeV1(BTreeMap::from([
        ("domain".into(), json!(domain)),
        ("risk".into(), json!("low")),
        ("reversible".into(), json!(true)),
    ]))
}

fn request(id: &str, candidates: Vec<IntentCandidateV1>) -> ResolveRequestV1 {
    ResolveRequestV1 {
        schema_version: SCHEMA_VERSION_V1.into(),
        request_id: id.into(),
        subject_ref: "user-1".into(),
        input: json!({"message": "please deploy"}),
        intent_candidates: candidates,
        context: ContextV1 {
            scope: scope("software_development"),
            current_constraints: vec!["must_be_reversible".into()],
            data: json!({"as_of": "2026-01-10T00:00:00Z"}),
        },
        human_model_refs: Vec::new(),
        human_model_snapshot: Vec::new(),
        evidence: vec![EvidenceV1 {
            evidence_id: "input-1".into(),
            kind: EvidenceKindV1::ExplicitStatement,
            strength: 0.9,
            source: "host".into(),
            observed_at: "2026-01-10T00:00:00Z".into(),
            data: Value::Null,
        }],
        host_capabilities: HostCapabilitiesV1 {
            model_inference: true,
            supported_actions: vec!["deploy".into(), "manual_review".into()],
        },
    }
}

fn intent(name: &str, confidence: f64) -> IntentCandidateV1 {
    IntentCandidateV1 {
        intent: name.into(),
        confidence,
        source: IntentSourceV1::ExplicitUser,
        constraints: Vec::new(),
        evidence_refs: vec!["input-1".into()],
    }
}

fn decision(outcome: ResolveOutcomeV1) -> DecisionContractV1 {
    match outcome {
        ResolveOutcomeV1::Decision { decision } => *decision,
        ResolveOutcomeV1::ModelInferenceRequired { .. } => panic!("expected decision"),
        ResolveOutcomeV1::Unresolved { .. } => panic!("expected decision"),
    }
}

fn successful_feedback(
    decision: &DecisionContractV1,
    response: UserResponseV1,
    actual_action: ActionV1,
    correction: Option<CorrectionV1>,
) -> OutcomeFeedbackV1 {
    OutcomeFeedbackV1 {
        decision_id: decision.decision_id.clone(),
        decision_digest: decision.decision_digest.clone(),
        recommended_action: decision.recommended_action.clone(),
        actual_action,
        user_response: response,
        outcome: OutcomeV1 {
            status: OutcomeStatusV1::Success,
            details: json!({"ok": true}),
        },
        correction,
        scope: scope("software_development"),
        observed_at: "2026-01-11T00:00:00Z".into(),
    }
}

#[test]
fn unsupported_intent_fails_closed_without_action_fallback() {
    let mut core = IdrCore::new();
    let result = core
        .resolve(request("req-unsupported", vec![intent("delete", 0.95)]))
        .expect("resolution should return a structured outcome");

    assert!(matches!(
        result,
        ResolveOutcomeV1::Unresolved {
            unresolved: UnresolvedResultV1 {
                resolved_intent: Some(intent),
                reason: UnresolvedReasonV1::UnsupportedAction,
                ..
            }
        } if intent == "delete"
    ));
    assert!(core.evaluation_records().is_empty());
}

#[test]
fn valid_host_intent_does_not_request_model() {
    let mut core = IdrCore::new();
    let result = core.resolve(request("req-1", vec![intent("deploy", 0.95)]));
    let resolved = decision(result.expect("resolve should succeed"));
    assert_eq!(resolved.model_usage, ModelUsageV1::NotRequired);
    assert_eq!(resolved.recommended_action.action, "deploy");
    assert!(resolved
        .reason_codes
        .contains(&"current_constraints_prioritized".into()));
}

#[test]
fn missing_intent_requests_host_model() {
    let mut core = IdrCore::new();
    let result = core
        .resolve(request("req-2", Vec::new()))
        .expect("resolve should succeed");
    assert!(matches!(
        result,
        ResolveOutcomeV1::ModelInferenceRequired {
            model_request: HostModelRequestV1 {
                purpose: HostModelPurposeV1::IntentResolution,
                ..
            }
        }
    ));
}

#[test]
fn ambiguous_intent_requests_disambiguation() {
    let mut core = IdrCore::new();
    let result = core
        .resolve(request(
            "req-3",
            vec![intent("deploy", 0.9), intent("manual_review", 0.85)],
        ))
        .expect("resolve should succeed");
    assert!(matches!(
        result,
        ResolveOutcomeV1::ModelInferenceRequired {
            model_request: HostModelRequestV1 {
                purpose: HostModelPurposeV1::IntentDisambiguation,
                ..
            }
        }
    ));
}

#[test]
fn disabled_model_inference_returns_unresolved_without_model_request() {
    let mut input = request("req-model-disabled", Vec::new());
    input.host_capabilities.model_inference = false;
    let mut core = IdrCore::new();
    let result = core
        .resolve(input)
        .expect("resolution should return a structured outcome");

    assert!(matches!(
        result,
        ResolveOutcomeV1::Unresolved {
            unresolved: UnresolvedResultV1 {
                reason: UnresolvedReasonV1::ModelInferenceUnavailable,
                ..
            }
        }
    ));
}

struct CountingProvider {
    calls: AtomicUsize,
}

#[async_trait]
impl HostModelProvider for CountingProvider {
    async fn infer(
        &self,
        request: HostModelRequestV1,
    ) -> Result<HostModelResultV1, HostModelProviderError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(HostModelResultV1 {
            request_id: request.request_id,
            resolved_intent: "deploy".into(),
            recommended_action: ActionV1::new("deploy"),
            alternatives: vec![ActionV1::new("manual_review")],
            constraints: Vec::new(),
            ambiguities: Vec::new(),
            confidence: 0.92,
        })
    }
}

#[tokio::test]
async fn in_process_provider_is_called_once() {
    let provider = CountingProvider {
        calls: AtomicUsize::new(0),
    };
    let mut core = IdrCore::new();
    let resolved = core
        .resolve_with_provider(request("req-4", Vec::new()), &provider)
        .await
        .expect("provider flow should succeed");
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    assert_eq!(resolved.model_usage, ModelUsageV1::HostDelegated);
}

#[tokio::test]
async fn in_process_provider_is_not_called_for_resolved_intent() {
    let provider = CountingProvider {
        calls: AtomicUsize::new(0),
    };
    let mut core = IdrCore::new();
    core.resolve_with_provider(
        request("req-5", vec![intent("deploy", 0.95)]),
        &provider,
    )
    .await
    .expect("direct flow should succeed");
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn decision_contract_json_round_trip() {
    let mut core = IdrCore::new();
    let original = decision(
        core.resolve(request("req-6", vec![intent("deploy", 0.95)]))
            .expect("resolve should succeed"),
    );
    let json = serde_json::to_string(&original).expect("serialize");
    let restored: DecisionContractV1 = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn structured_host_result_is_validated() {
    let mut core = IdrCore::new();
    let error = core
        .continue_resolve(ContinueResolveRequestV1 {
            original_request: request("req-7", Vec::new()),
            model_result: HostModelResultV1 {
                request_id: "wrong".into(),
                resolved_intent: "deploy".into(),
                recommended_action: ActionV1::new("deploy"),
                alternatives: Vec::new(),
                constraints: Vec::new(),
                ambiguities: Vec::new(),
                confidence: 0.9,
            },
        })
        .expect_err("mismatched result must fail");
    assert!(matches!(error, IdrError::InvalidContract(_)));
}

#[test]
fn host_model_result_actions_must_match_host_capabilities() {
    let mut core = IdrCore::new();
    let original_request = request("req-capabilities", Vec::new());
    let unsupported_recommendation = core
        .continue_resolve(ContinueResolveRequestV1 {
            original_request: original_request.clone(),
            model_result: HostModelResultV1 {
                request_id: original_request.request_id.clone(),
                resolved_intent: "delete".into(),
                recommended_action: ActionV1::new("delete"),
                alternatives: Vec::new(),
                constraints: Vec::new(),
                ambiguities: Vec::new(),
                confidence: 0.9,
            },
        })
        .expect_err("unsupported recommendation must be rejected");
    assert!(matches!(
        unsupported_recommendation,
        IdrError::CapabilityViolation(_)
    ));

    let unsupported_alternative = core
        .continue_resolve(ContinueResolveRequestV1 {
            original_request: original_request.clone(),
            model_result: HostModelResultV1 {
                request_id: original_request.request_id,
                resolved_intent: "deploy".into(),
                recommended_action: ActionV1::new("deploy"),
                alternatives: vec![ActionV1::new("delete")],
                constraints: Vec::new(),
                ambiguities: Vec::new(),
                confidence: 0.9,
            },
        })
        .expect_err("unsupported alternative must be rejected");
    assert!(matches!(
        unsupported_alternative,
        IdrError::CapabilityViolation(_)
    ));
}

#[test]
fn explicit_preference_is_active_immediately() {
    let mut core = IdrCore::new();
    let assertion = core
        .observe(ObservationV1 {
            evidence_id: "explicit-1".into(),
            subject_ref: "user-1".into(),
            kind: AssertionKindV1::Preference,
            predicate: "preferred_action".into(),
            value: json!(ActionV1::new("manual_review")),
            scope: scope("software_development"),
            source_type: AssertionSourceTypeV1::Explicit,
            observed_at: "2026-01-01T00:00:00Z".into(),
        })
        .expect("explicit observation should be accepted");
    assert_eq!(assertion.status, AssertionStatusV1::Active);
    assert!(assertion.confidence >= 0.9);
}

#[test]
fn implicit_signal_requires_repetition() {
    let mut core = IdrCore::new();
    for number in 1..=2 {
        core.observe(ObservationV1 {
            evidence_id: format!("implicit-{number}"),
            subject_ref: "user-1".into(),
            kind: AssertionKindV1::Preference,
            predicate: "preferred_action".into(),
            value: json!(ActionV1::new("manual_review")),
            scope: scope("software_development"),
            source_type: AssertionSourceTypeV1::Inferred,
            observed_at: format!("2026-01-0{number}T00:00:00Z"),
        })
        .expect("implicit observation should be accepted");
    }
    assert!(core
        .query_human_model(QueryHumanModelRequestV1 {
            subject_ref: "user-1".into(),
            scope: scope("software_development"),
            as_of: "2026-01-10T00:00:00Z".into(),
        })
        .is_empty());

    let activated = core
        .observe(ObservationV1 {
            evidence_id: "implicit-3".into(),
            subject_ref: "user-1".into(),
            kind: AssertionKindV1::Preference,
            predicate: "preferred_action".into(),
            value: json!(ActionV1::new("manual_review")),
            scope: scope("software_development"),
            source_type: AssertionSourceTypeV1::Inferred,
            observed_at: "2026-01-03T00:00:00Z".into(),
        })
        .expect("third observation should be accepted");
    assert_eq!(activated.status, AssertionStatusV1::Active);
}

#[test]
fn scope_mismatch_does_not_affect_decision() {
    let mut core = IdrCore::new();
    core.observe(ObservationV1 {
        evidence_id: "explicit-design".into(),
        subject_ref: "user-1".into(),
        kind: AssertionKindV1::Preference,
        predicate: "preferred_action".into(),
        value: json!(ActionV1::new("manual_review")),
        scope: scope("industrial_design"),
        source_type: AssertionSourceTypeV1::Explicit,
        observed_at: "2026-01-01T00:00:00Z".into(),
    })
    .expect("observation should succeed");
    let resolved = decision(
        core.resolve(request("req-8", vec![intent("deploy", 0.95)]))
            .expect("resolve should succeed"),
    );
    assert_eq!(resolved.recommended_action.action, "deploy");
    assert!(resolved.human_model_basis.is_empty());
}

#[test]
fn contradiction_supersedes_without_erasing_history() {
    let mut core = IdrCore::new();
    let old = core
        .observe(ObservationV1 {
            evidence_id: "explicit-a".into(),
            subject_ref: "user-1".into(),
            kind: AssertionKindV1::Preference,
            predicate: "preferred_action".into(),
            value: json!(ActionV1::new("deploy")),
            scope: scope("software_development"),
            source_type: AssertionSourceTypeV1::Explicit,
            observed_at: "2026-01-01T00:00:00Z".into(),
        })
        .expect("first assertion");
    let new = core
        .observe(ObservationV1 {
            evidence_id: "explicit-b".into(),
            subject_ref: "user-1".into(),
            kind: AssertionKindV1::Preference,
            predicate: "preferred_action".into(),
            value: json!(ActionV1::new("manual_review")),
            scope: scope("software_development"),
            source_type: AssertionSourceTypeV1::Explicit,
            observed_at: "2026-01-02T00:00:00Z".into(),
        })
        .expect("replacement assertion");
    let historical = core
        .assertions()
        .iter()
        .find(|assertion| assertion.assertion_id == old.assertion_id)
        .expect("old assertion remains");
    assert_eq!(historical.status, AssertionStatusV1::Superseded);
    assert!(historical.confidence < old.confidence);
    assert_eq!(historical.valid_until.as_deref(), Some("2026-01-02T00:00:00Z"));
    assert!(new.supersedes.contains(&old.assertion_id));
    assert!(new.contradicts.contains(&old.assertion_id));
}

#[test]
fn expired_snapshot_does_not_affect_decision() {
    let mut input = request("req-9", vec![intent("deploy", 0.95)]);
    input.human_model_snapshot.push(HumanModelAssertionV1 {
        assertion_id: "expired".into(),
        subject_ref: "user-1".into(),
        kind: AssertionKindV1::Preference,
        predicate: "preferred_action".into(),
        value: json!(ActionV1::new("manual_review")),
        scope: scope("software_development"),
        confidence: 0.99,
        source_type: AssertionSourceTypeV1::Explicit,
        evidence_refs: vec!["old".into()],
        status: AssertionStatusV1::Active,
        valid_from: "2025-01-01T00:00:00Z".into(),
        valid_until: Some("2025-12-31T00:00:00Z".into()),
        recorded_at: "2025-01-01T00:00:00Z".into(),
        last_confirmed_at: None,
        supersedes: Vec::new(),
        contradicts: Vec::new(),
    });
    let mut core = IdrCore::new();
    let resolved = decision(core.resolve(input).expect("resolve should succeed"));
    assert_eq!(resolved.recommended_action.action, "deploy");
    assert!(resolved.human_model_basis.is_empty());
}

#[test]
fn current_constraint_blocks_conflicting_historical_preference() {
    let mut core = IdrCore::new();
    let preference = core
        .observe(ObservationV1 {
            evidence_id: "explicit-constraint-conflict".into(),
            subject_ref: "user-1".into(),
            kind: AssertionKindV1::Preference,
            predicate: "preferred_action".into(),
            value: json!(ActionV1::new("manual_review")),
            scope: scope("software_development"),
            source_type: AssertionSourceTypeV1::Explicit,
            observed_at: "2026-01-01T00:00:00Z".into(),
        })
        .expect("historical preference");
    let mut input = request("req-current-constraint", vec![intent("deploy", 0.95)]);
    input.context.current_constraints = vec!["must_deploy_directly".into()];

    let resolved = decision(core.resolve(input).expect("resolve"));

    assert_eq!(resolved.recommended_action.action, "deploy");
    assert_eq!(resolved.human_model_basis, vec![preference.assertion_id]);
    assert!(resolved
        .reason_codes
        .contains(&"human_model_preference_blocked_by_current_constraint".into()));
    assert!(!resolved
        .reason_codes
        .contains(&"human_model_preference_applied".into()));
}

#[test]
fn historical_preference_applies_when_no_current_constraint_conflict() {
    let mut core = IdrCore::new();
    core.observe(ObservationV1 {
        evidence_id: "explicit-no-constraint".into(),
        subject_ref: "user-1".into(),
        kind: AssertionKindV1::Preference,
        predicate: "preferred_action".into(),
        value: json!(ActionV1::new("manual_review")),
        scope: scope("software_development"),
        source_type: AssertionSourceTypeV1::Explicit,
        observed_at: "2026-01-01T00:00:00Z".into(),
    })
    .expect("historical preference");
    let mut input = request("req-no-current-constraint", vec![intent("deploy", 0.95)]);
    input.context.current_constraints.clear();

    let resolved = decision(core.resolve(input).expect("resolve"));

    assert_eq!(resolved.recommended_action.action, "manual_review");
    assert!(resolved
        .reason_codes
        .contains(&"human_model_preference_applied".into()));
    assert!(!resolved.reason_codes.contains(
        &"human_model_preference_blocked_by_current_constraint".into()
    ));
}

#[test]
fn same_feedback_replay_is_idempotent() {
    let mut core = IdrCore::new();
    let resolved = decision(
        core.resolve(request("req-replay", vec![intent("deploy", 0.95)]))
            .expect("resolve"),
    );
    let feedback = successful_feedback(
        &resolved,
        UserResponseV1::Corrected,
        ActionV1::new("manual_review"),
        Some(CorrectionV1 {
            corrected_intent: None,
            preferred_action: Some(ActionV1::new("manual_review")),
        }),
    );
    let first = core.feedback(feedback.clone()).expect("first feedback");
    assert_eq!(core.assertions()[0].source_type, AssertionSourceTypeV1::Explicit);
    assert_eq!(core.assertions()[0].evidence_refs.len(), 1);
    assert_eq!(core.assertions()[0].confidence, 0.9);
    let evidence_after_first = core.evidence().to_vec();
    let assertions_after_first = core.assertions().to_vec();
    let evaluations_after_first = core.evaluation_records().to_vec();

    let replay = core.feedback(feedback).expect("identical replay");

    assert_eq!(replay, first);
    assert_eq!(core.evidence(), evidence_after_first);
    assert_eq!(core.assertions(), assertions_after_first);
    assert_eq!(core.evaluation_records(), evaluations_after_first);
}

#[test]
fn conflicting_feedback_for_same_decision_fails_closed() {
    let mut core = IdrCore::new();
    let resolved = decision(
        core.resolve(request("req-conflict", vec![intent("deploy", 0.95)]))
            .expect("resolve"),
    );
    let feedback = successful_feedback(
        &resolved,
        UserResponseV1::Accepted,
        ActionV1::new("deploy"),
        None,
    );
    core.feedback(feedback.clone()).expect("first feedback");
    let evidence_after_first = core.evidence().to_vec();
    let assertions_after_first = core.assertions().to_vec();
    let evaluations_after_first = core.evaluation_records().to_vec();
    let mut conflicting = feedback;
    conflicting.actual_action = ActionV1::new("manual_review");

    assert!(matches!(
        core.feedback(conflicting),
        Err(IdrError::FeedbackConflict(_))
    ));
    assert_eq!(core.evidence(), evidence_after_first);
    assert_eq!(core.assertions(), assertions_after_first);
    assert_eq!(core.evaluation_records(), evaluations_after_first);
}

#[test]
fn three_replays_of_one_decision_do_not_activate_preference() {
    let mut core = IdrCore::new();
    let resolved = decision(
        core.resolve(request("req-three-replays", vec![intent("deploy", 0.95)]))
            .expect("resolve"),
    );
    let feedback = successful_feedback(
        &resolved,
        UserResponseV1::Accepted,
        ActionV1::new("deploy"),
        None,
    );
    let first = core.feedback(feedback.clone()).expect("first feedback");
    for _ in 0..2 {
        assert_eq!(
            core.feedback(feedback.clone()).expect("idempotent replay"),
            first
        );
    }

    assert_eq!(core.evidence().len(), 1);
    assert_eq!(core.assertions().len(), 1);
    assert_eq!(core.assertions()[0].status, AssertionStatusV1::Candidate);
    assert_eq!(core.assertions()[0].evidence_refs.len(), 1);
    assert_eq!(core.assertions()[0].confidence, 0.25);
    assert!(core
        .query_human_model(QueryHumanModelRequestV1 {
            subject_ref: "user-1".into(),
            scope: scope("software_development"),
            as_of: "2026-01-12T00:00:00Z".into(),
        })
        .is_empty());
}

#[test]
fn three_independent_decisions_can_activate_implicit_preference() {
    let mut core = IdrCore::new();
    for number in 1..=3 {
        let resolved = decision(
            core.resolve(request(
                &format!("req-independent-{number}"),
                vec![intent("deploy", 0.95)],
            ))
            .expect("resolve"),
        );
        core.feedback(successful_feedback(
            &resolved,
            UserResponseV1::Accepted,
            ActionV1::new("deploy"),
            None,
        ))
        .expect("independent feedback");
    }

    assert_eq!(core.evidence().len(), 3);
    assert_eq!(core.assertions().len(), 1);
    assert_eq!(core.assertions()[0].status, AssertionStatusV1::Active);
    assert_eq!(core.assertions()[0].evidence_refs.len(), 3);
    assert_eq!(core.assertions()[0].confidence, 0.65);
    assert_eq!(
        core.query_human_model(QueryHumanModelRequestV1 {
            subject_ref: "user-1".into(),
            scope: scope("software_development"),
            as_of: "2026-01-12T00:00:00Z".into(),
        })
        .len(),
        1
    );
}

#[test]
fn corrected_feedback_creates_override_and_changes_next_decision() {
    let mut core = IdrCore::new();
    let first = decision(
        core.resolve(request("req-11", vec![intent("deploy", 0.95)]))
            .expect("first resolve"),
    );
    let feedback_result = core
        .feedback(successful_feedback(
            &first,
            UserResponseV1::Corrected,
            ActionV1::new("manual_review"),
            Some(CorrectionV1 {
                corrected_intent: None,
                preferred_action: Some(ActionV1::new("manual_review")),
            }),
        ))
        .expect("corrected feedback");
    assert!(feedback_result
        .evidence
        .iter()
        .any(|evidence| evidence.kind == EvidenceKindV1::DecisionOverride));
    assert!(feedback_result.evaluation_record.decision_overridden);

    let mut second_request = request("req-12", vec![intent("deploy", 0.95)]);
    second_request.context.data = json!({"as_of": "2026-01-12T00:00:00Z"});
    second_request.context.current_constraints.clear();
    let second = decision(
        core.resolve(second_request)
            .expect("second resolve"),
    );
    assert_eq!(second.recommended_action.action, "manual_review");
    assert!(!second.human_model_basis.is_empty());
    assert!(second
        .reason_codes
        .contains(&"human_model_preference_applied".into()));
}

#[test]
fn tampered_feedback_cannot_update_human_model() {
    let mut core = IdrCore::new();
    let resolved = decision(
        core.resolve(request("req-feedback-binding", vec![intent("deploy", 0.95)]))
            .expect("resolve"),
    );
    let valid = successful_feedback(
        &resolved,
        UserResponseV1::Corrected,
        ActionV1::new("manual_review"),
        Some(CorrectionV1 {
            corrected_intent: None,
            preferred_action: Some(ActionV1::new("manual_review")),
        }),
    );
    let evidence_before = core.evidence().len();
    let assertions_before = core.assertions().len();

    let mut wrong_action = valid.clone();
    wrong_action.recommended_action = ActionV1::new("manual_review");
    assert!(matches!(
        core.feedback(wrong_action),
        Err(IdrError::InvalidContract(_))
    ));

    let mut wrong_scope = valid.clone();
    wrong_scope.scope = scope("industrial_design");
    assert!(matches!(
        core.feedback(wrong_scope),
        Err(IdrError::InvalidContract(_))
    ));

    let mut wrong_digest = valid;
    wrong_digest.decision_digest = "0".repeat(64);
    assert!(matches!(
        core.feedback(wrong_digest),
        Err(IdrError::InvalidContract(_))
    ));

    assert_eq!(core.evidence().len(), evidence_before);
    assert_eq!(core.assertions().len(), assertions_before);
    assert_eq!(
        core.evaluation_records()[0].outcome_status,
        EvaluationOutcomeStatusV1::Pending
    );
}

#[test]
fn evaluation_record_tracks_intent_correction() {
    let mut core = IdrCore::new();
    let resolved = decision(
        core.resolve(request("req-13", vec![intent("deploy", 0.95)]))
            .expect("resolve"),
    );
    let result = core
        .feedback(successful_feedback(
            &resolved,
            UserResponseV1::Corrected,
            ActionV1::new("manual_review"),
            Some(CorrectionV1 {
                corrected_intent: Some("review_before_deploy".into()),
                preferred_action: Some(ActionV1::new("manual_review")),
            }),
        ))
        .expect("feedback");
    assert!(result.evaluation_record.intent_corrected);
    assert_eq!(
        result.evaluation_record.outcome_status,
        EvaluationOutcomeStatusV1::Success
    );
}

#[test]
fn personality_inference_is_rejected() {
    let mut core = IdrCore::new();
    let error = core
        .observe(ObservationV1 {
            evidence_id: "bad".into(),
            subject_ref: "user-1".into(),
            kind: AssertionKindV1::InteractionPattern,
            predicate: "personality_profile".into(),
            value: json!("introvert"),
            scope: scope("software_development"),
            source_type: AssertionSourceTypeV1::Inferred,
            observed_at: "2026-01-01T00:00:00Z".into(),
        })
        .expect_err("profiling must be rejected");
    assert!(matches!(error, IdrError::InvalidContract(_)));
}

#[test]
fn core_has_no_database_or_provider_specific_dependencies() {
    let manifest = include_str!("../Cargo.toml").to_ascii_lowercase();
    for forbidden in [
        "sqlx",
        "postgres",
        "openai",
        "anthropic",
        "gemini",
        "qwen",
        "deepseek",
    ] {
        assert!(!manifest.contains(forbidden), "found {forbidden}");
    }
}
