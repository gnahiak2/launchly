use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct CreatePlanRequest {
    pub repository_url: String,
}

#[derive(Debug, Serialize)]
pub struct DeploymentPlan {
    pub repository_url: String,
    pub framework: String,
    pub language: String,
    pub package_manager: String,
    pub build_command: String,
    pub start_command: String,
    pub port: u16,
    pub confidence: String,
    pub rationale: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}
