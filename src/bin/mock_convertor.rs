use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::signal;

#[derive(Deserialize, Debug)]
struct HealthQuery {
    environment: String,
    service: String,
    #[serde(default)]
    _from: String,
    #[serde(default)]
    _to: String,
}

/// Structure of the response which waits reporter.rs
#[derive(Serialize, Debug)]
struct ServiceHealthResponse {
    name: String,
    service_category: String,
    environment: String,
    metrics: Vec<(i64, u8)>,
}

/// State of the mock server. Mutex provides live changes.
type AppState = Arc<Mutex<HashMap<String, u8>>>;

#[tokio::main]
async fn main() {
    // Key "environment:service", value - status.
    let health_statuses: AppState = Arc::new(Mutex::new(HashMap::new()));

    // Initial state
    // 0 = OK, >0 = Problem
    health_statuses
        .lock()
        .unwrap()
        .insert("production_eu-de:test".to_string(), 2); // Imitate a problem (impact = 2)

    let app = Router::new()
        .route("/api/v1/health", get(health_handler))
        .with_state(health_statuses);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3005));
    println!("Mock convertor listening on {}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn health_handler(
    State(state): State<AppState>,
    Query(params): Query<HealthQuery>,
) -> (StatusCode, Json<ServiceHealthResponse>) {
    let key = format!("{}:{}", params.environment, params.service);
    println!("Received request for: {}", key);

    let statuses = state.lock().unwrap();
    let status_value = statuses.get(&key).cloned().unwrap_or(0);

    let response = ServiceHealthResponse {
        name: params.service.clone(),
        service_category: "mock_category".to_string(),
        environment: params.environment.clone(),
        metrics: vec![(Utc::now().timestamp(), status_value)],
    };

    println!("Responding with status: {}", status_value);
    (StatusCode::OK, Json(response))
}

async fn shutdown_signal() {
    signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C handler");
    println!("Signal received, shutting down mock server.");
}
