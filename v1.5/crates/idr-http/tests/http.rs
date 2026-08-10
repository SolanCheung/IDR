use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use idr_core::*;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use tower::ServiceExt;

fn scope() -> ScopeV1 {
    ScopeV1(BTreeMap::from([
        ("domain".into(), json!("software_development")),
        ("risk".into(), json!("low")),
        ("reversible".into(), json!(true)),
    ]))
}

fn resolve_request(request_id: &str, with_intent: bool) -> ResolveRequestV1 {
    ResolveRequestV1 {
        schema_version: SCHEMA_VERSION_V1.into(),
        request_id: request_id.into(),
        subject_ref: "user-http".into(),
        input: json!({"message": "deploy"}),
        intent_candidates: if with_intent {
            vec![IntentCandidateV1 {
                intent: "deploy".into(),
                confidence: 0.95,
                source: IntentSourceV1::ExplicitUser,
                constraints: Vec::new(),
                evidence_refs: Vec::new(),
            }]
        } else {
            Vec::new()
        },
        context: ContextV1 {
            scope: scope(),
            current_constraints: Vec::new(),
            data: json!({"as_of": "2026-01-01T00:00:00Z"}),
        },
        human_model_refs: Vec::new(),
        human_model_snapshot: Vec::new(),
        evidence: Vec::new(),
        host_capabilities: HostCapabilitiesV1::default(),
    }
}

async fn post<T: serde::Serialize, R: DeserializeOwned>(
    app: axum::Router,
    uri: &str,
    body: &T,
) -> (StatusCode, R) {
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(body).expect("serialize request")))
                .expect("build request"),
        )
        .await
        .expect("request should complete");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("read body")
        .to_bytes();
    (status, serde_json::from_slice(&bytes).expect("deserialize response"))
}

#[tokio::test]
async fn resolve_returns_decision_without_model() {
    let app = idr_http::router(IdrCore::new());
    let (status, result): (StatusCode, ResolveOutcomeV1) =
        post(app, "/v1/resolve", &resolve_request("http-1", true)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(matches!(result, ResolveOutcomeV1::Decision { .. }));
}

#[tokio::test]
async fn resolve_model_continue_returns_decision() {
    let app = idr_http::router(IdrCore::new());
    let original = resolve_request("http-2", false);
    let (status, first): (StatusCode, ResolveOutcomeV1) =
        post(app.clone(), "/v1/resolve", &original).await;
    assert_eq!(status, StatusCode::OK);
    let model_request = match first {
        ResolveOutcomeV1::ModelInferenceRequired { model_request } => model_request,
        ResolveOutcomeV1::Decision { .. } => panic!("expected model request"),
    };
    let continuation = ContinueResolveRequestV1 {
        original_request: original,
        model_result: HostModelResultV1 {
            request_id: model_request.request_id,
            resolved_intent: "deploy".into(),
            recommended_action: ActionV1::new("deploy"),
            alternatives: Vec::new(),
            constraints: Vec::new(),
            ambiguities: Vec::new(),
            confidence: 0.91,
        },
    };
    let (status, second): (StatusCode, ResolveOutcomeV1) =
        post(app, "/v1/resolve/continue", &continuation).await;
    assert_eq!(status, StatusCode::OK);
    let continued = match second {
        ResolveOutcomeV1::Decision { decision } => decision,
        ResolveOutcomeV1::ModelInferenceRequired { .. } => panic!("expected decision"),
    };
    assert_eq!(continued.model_usage, ModelUsageV1::HostDelegated);
}

#[tokio::test]
async fn feedback_updates_human_model_endpoint() {
    let app = idr_http::router(IdrCore::new());
    let (_, resolved): (StatusCode, ResolveOutcomeV1) =
        post(app.clone(), "/v1/resolve", &resolve_request("http-3", true)).await;
    let decision = match resolved {
        ResolveOutcomeV1::Decision { decision } => decision,
        ResolveOutcomeV1::ModelInferenceRequired { .. } => panic!("expected decision"),
    };
    let feedback = OutcomeFeedbackV1 {
        decision_id: decision.decision_id.clone(),
        recommended_action: decision.recommended_action,
        actual_action: ActionV1::new("manual_review"),
        user_response: UserResponseV1::Corrected,
        outcome: OutcomeV1 {
            status: OutcomeStatusV1::Success,
            details: Value::Null,
        },
        correction: Some(CorrectionV1 {
            corrected_intent: None,
            preferred_action: Some(ActionV1::new("manual_review")),
        }),
        scope: scope(),
        observed_at: "2026-01-02T00:00:00Z".into(),
    };
    let (status, result): (StatusCode, FeedbackResultV1) =
        post(app.clone(), "/v1/feedback", &feedback).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!result.assertions_updated.is_empty());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/human-model/user-http")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let assertions: Vec<HumanModelAssertionV1> =
        serde_json::from_slice(&bytes).expect("human model response");
    assert_eq!(assertions.len(), 1);
}
