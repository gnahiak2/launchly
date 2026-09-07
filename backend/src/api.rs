use crate::{
    deploy::DeploymentService,
    detector,
    models::{CreatePlanRequest, DeployRequest, ErrorResponse, HealthResponse},
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};

pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

pub async fn create_plan(Json(request): Json<CreatePlanRequest>) -> impl IntoResponse {
    match detector::create_plan(&request.repository_url) {
        Ok(plan) => (StatusCode::OK, Json(plan)).into_response(),
        Err(error) => (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })).into_response(),
    }
}

pub async fn create_deployment(
    State(service): State<DeploymentService>,
    Json(request): Json<DeployRequest>,
) -> impl IntoResponse {
    match service.create(request).await {
        Ok(record) => (StatusCode::ACCEPTED, Json(record)).into_response(),
        Err(error) => (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })).into_response(),
    }
}

pub async fn get_deployment(
    State(service): State<DeploymentService>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match service.get(&id).await {
        Some(record) => (StatusCode::OK, Json(record)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("deployment {id} not found"),
            }),
        )
            .into_response(),
    }
}
