use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use idr_core::*;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
struct HttpState {
    core: Arc<Mutex<IdrCore>>,
}

pub fn router(core: IdrCore) -> Router {
    Router::new()
        .route("/v1/resolve", post(resolve))
        .route("/v1/resolve/continue", post(continue_resolve))
        .route("/v1/feedback", post(feedback))
        .route("/v1/human-model/{subject_ref}", get(query_human_model))
        .with_state(HttpState {
            core: Arc::new(Mutex::new(core)),
        })
}

async fn resolve(
    State(state): State<HttpState>,
    Json(request): Json<ResolveRequestV1>,
) -> Result<Json<ResolveOutcomeV1>, ApiError> {
    let mut core = state.core.lock().await;
    Ok(Json(core.resolve(request)?))
}

async fn continue_resolve(
    State(state): State<HttpState>,
    Json(request): Json<ContinueResolveRequestV1>,
) -> Result<Json<ResolveOutcomeV1>, ApiError> {
    let mut core = state.core.lock().await;
    let decision = core.continue_resolve(request)?;
    Ok(Json(ResolveOutcomeV1::Decision {
        decision: Box::new(decision),
    }))
}

async fn feedback(
    State(state): State<HttpState>,
    Json(feedback): Json<OutcomeFeedbackV1>,
) -> Result<Json<FeedbackResultV1>, ApiError> {
    let mut core = state.core.lock().await;
    Ok(Json(core.feedback(feedback)?))
}

async fn query_human_model(
    State(state): State<HttpState>,
    Path(subject_ref): Path<String>,
) -> Json<Vec<HumanModelAssertionV1>> {
    let core = state.core.lock().await;
    Json(core.query_human_model(QueryHumanModelRequestV1 {
        subject_ref,
        scope: ScopeV1::default(),
        as_of: "9999-12-31T23:59:59Z".into(),
    }))
}

struct ApiError(IdrError);

impl From<IdrError> for ApiError {
    fn from(value: IdrError) -> Self {
        Self(value)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self.0 {
            IdrError::UnknownDecision(_) => StatusCode::NOT_FOUND,
            IdrError::FeedbackConflict(_) => StatusCode::CONFLICT,
            IdrError::InvalidContract(_) | IdrError::HostModel(_) => StatusCode::BAD_REQUEST,
            IdrError::CapabilityViolation(_) | IdrError::Unresolved(_) => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
        };
        (status, Json(json!({ "error": self.0.to_string() }))).into_response()
    }
}
