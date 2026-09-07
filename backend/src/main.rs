mod api;
mod deploy;
mod detector;
mod models;

use axum::{
    routing::{get, post},
    Router,
};
use std::{net::SocketAddr, path::PathBuf};
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
    let frontend = std::env::var_os("LAUNCHLY_FRONTEND_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../frontend/dist"));
    let state = deploy::DeploymentService::new();
    let app = Router::new()
        .route("/api/health", get(api::health))
        .route("/api/plans", post(api::create_plan))
        .route("/api/deployments", post(api::create_deployment))
        .route("/api/deployments/{id}", get(api::get_deployment))
        .fallback_service(ServeDir::new(frontend).append_index_html_on_directories(true))
        .with_state(state);
    let address: SocketAddr = ([0, 0, 0, 0], 8080).into();
    println!("Launchly listening on http://{address}");
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .expect("bind listener");
    axum::serve(listener, app).await.expect("serve application");
}
