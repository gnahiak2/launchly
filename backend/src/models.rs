use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct CreatePlanRequest {
    pub repository_url: String,
}

#[derive(Debug, Deserialize)]
pub struct DeployRequest {
    pub repository_url: String,
    pub domain: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Serialize)]
pub struct DeploymentRecord {
    pub id: String,
    pub repository_url: String,
    pub domain: Option<String>,
    pub state: String,
    pub phase: String,
    pub message: String,
    pub plan: Option<DeploymentPlan>,
    pub container: Option<String>,
    pub host_port: Option<u16>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}
