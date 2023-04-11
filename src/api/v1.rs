//! CloudMon metrics processor API v1
//!
//! API v1 of the metrics convertor
//!
use axum::{
    extract::Query,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::common::get_service_health;
use crate::types::{AppState, CloudMonError, ServiceHealthData};

/// Query parameters supported by the /health API call
#[derive(Debug, Deserialize)]
pub struct HealthQuery {
    /// Start point to query metrics
    pub from: String,
    pub to: String,
    #[serde(default = "default_max_data_points")]
    pub max_data_points: u32,
    pub service: String,
    pub environment: String,
}

fn default_max_data_points() -> u32 {
    100
}

/// Response of the /health API call
#[derive(Debug, Serialize, Deserialize)]
pub struct ServiceHealthResponse {
    pub name: String,
    pub service_category: String,
    pub environment: String,
    pub metrics: ServiceHealthData,
}

/// Construct supported api v1 routes
pub fn get_v1_routes() -> Router<AppState> {
    return Router::new()
        .route("/", get(root))
        .route("/info", get(info))
        .route("/health", get(handler_health));
}

/// Return API v1 root info
async fn root() -> impl IntoResponse {
    return (StatusCode::OK, Json(json!({"name": "v1"})));
}

/// Return v1 API infos
async fn info() -> impl IntoResponse {
    (StatusCode::OK, "V1 API of the CloudMon\n")
}

/// Handler method invoked for /health request
pub async fn handler_health(query: Query<HealthQuery>, State(state): State<AppState>) -> Response {
    tracing::debug!("Processing query {:?}", query);
    match state.health_metrics.get(&query.service) {
        Some(hm_config) => {
            // We have health metric configuration
            match get_service_health(
                &state,
                query.service.as_str(),
                query.environment.as_str(),
                query.from.as_str(),
                query.to.as_str(),
                query.max_data_points as u16,
            )
            .await
            {
                Ok(health_data) => (
                    StatusCode::OK,
                    Json(ServiceHealthResponse {
                        name: query.service.clone(),
                        service_category: hm_config.category.clone(),
                        environment: query.environment.clone(),
                        metrics: health_data,
                    }),
                )
                    .into_response(),
                Err(error) => match error {
                    CloudMonError::EnvNotSupported | CloudMonError::ServiceNotSupported => (
                        StatusCode::CONFLICT,
                        Json(json!({ "message": format!("{}", error) })),
                    )
                        .into_response(),
                    _ => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({ "message": format!("{}", error) })),
                    )
                        .into_response(),
                },
            }
        }
        _ => {
            // Requested service is not known
            (
                StatusCode::CONFLICT,
                Json(json!({"message": "Service not supported"})),
            )
                .into_response()
        }
    }
}
