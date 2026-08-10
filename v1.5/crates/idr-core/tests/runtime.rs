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
fn feedback_accepted_accumulates_evidence_and_activates_pattern() {
    let mut core = IdrCore::new();
    let resolved = decision(
        core.resolve(request("req-10", vec![intent("deploy", 0.95)]))
            .expect("resolve should succeed"),
    );
    for _ in 0..3 {
        core.feedback(successful_feedback(
            &resolved,
            UserResponseV1::Accepted,
            ActionV1::new("deploy"),
            None,
        ))
        .expect("feedback should succeed");
    }
    assert!(core.evidence().len() >= 3);
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
