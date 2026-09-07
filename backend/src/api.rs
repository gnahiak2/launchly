use crate::{
    detector,
    models::{CreatePlanRequest, ErrorResponse, HealthResponse},
};
use axum::{extract::Json, http::StatusCode, response::IntoResponse};

pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

pub async fn create_plan(Json(request): Json<CreatePlanRequest>) -> impl IntoResponse {
    match detector::create_plan(&request.repository_url) {
        Ok(plan) => (StatusCode::OK, Json(plan)).into_response(),
        Err(error) => (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })).into_response(),
    }
}
