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

#[cfg(test)]
mod test {
    use super::*;
    use crate::config;
    use crate::types;
    use axum::{body::Body, http::Request};
    use serde_json::Value;
    use tower::Service; // For call()
    use tower::ServiceExt; // For ready() and call()

    /// T048: Test /api/v1/ root endpoint returns name
    #[tokio::test]
    async fn test_v1_root_endpoint() {
        let config_str = "
        datasource:
          url: 'https://graphite.example.com'
        server:
          port: 3000
        environments:
          - name: prod
        flag_metrics: []
        health_metrics: {}
        ";
        let config = config::Config::from_config_str(config_str);
        let state = types::AppState::new(config);
        let mut app = get_v1_routes().with_state(state);

        let request = Request::builder().uri("/").body(Body::empty()).unwrap();

        let response = app.ready().await.unwrap().call(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body, serde_json::json!({"name": "v1"}));
    }

    /// T049: Test /api/v1/info returns API info
    #[tokio::test]
    async fn test_v1_info_endpoint() {
        let config_str = "
        datasource:
          url: 'https://graphite.example.com'
        server:
          port: 3000
        environments:
          - name: prod
        flag_metrics: []
        health_metrics: {}
        ";
        let config = config::Config::from_config_str(config_str);
        let state = types::AppState::new(config);
        let mut app = get_v1_routes().with_state(state);

        let request = Request::builder().uri("/info").body(Body::empty()).unwrap();

        let response = app.ready().await.unwrap().call(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("V1 API of the CloudMon"));
    }

    /// T050: Test /api/v1/health with valid service returns 200 + JSON
    #[tokio::test]
    async fn test_v1_health_valid_service() {
        // Create a mock Graphite server
        let mut server = mockito::Server::new();

        // Mock the /render endpoint to return sample metric data
        let _mock = server
            .mock("GET", "/render")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!([
                    {
                        "target": "test-metric",
                        "datapoints": [
                            [85.0, 1609459200],
                            [90.0, 1609459260]
                        ]
                    }
                ])
                .to_string(),
            )
            .create();

        let config_str = format!(
            "
        datasource:
          url: '{}'
        server:
          port: 3000
        metric_templates:
          cpu_tmpl:
            query: 'system.$environment.$service.cpu'
            op: gt
            threshold: 80
        environments:
          - name: prod
        flag_metrics:
          - name: cpu-usage
            service: webapp
            template:
              name: cpu_tmpl
            environments:
              - name: prod
        health_metrics:
          webapp:
            service: webapp
            category: compute
            metrics:
              - webapp.cpu-usage
            expressions:
              - expression: 'webapp.cpu_usage'
                weight: 2
        ",
            server.url()
        );

        let config = config::Config::from_config_str(&config_str);
        let mut state = types::AppState::new(config);
        state.process_config();
        let mut app = get_v1_routes().with_state(state);

        let request = Request::builder()
            .uri("/health?service=webapp&environment=prod&from=now-1h&to=now")
            .body(Body::empty())
            .unwrap();

        let response = app.ready().await.unwrap().call(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let body: Value = serde_json::from_slice(&body).unwrap();

        // Verify response structure
        assert!(body.get("name").is_some());
        assert_eq!(body["name"], "webapp");
        assert!(body.get("service_category").is_some());
        assert_eq!(body["service_category"], "compute");
        assert!(body.get("environment").is_some());
        assert_eq!(body["environment"], "prod");
        assert!(body.get("metrics").is_some());
    }

    /// T051: Test /api/v1/health with unknown service returns 409
    #[tokio::test]
    async fn test_v1_health_unknown_service() {
        let config_str = "
        datasource:
          url: 'https://graphite.example.com'
        server:
          port: 3000
        environments:
          - name: prod
        flag_metrics: []
        health_metrics:
          known-service:
            service: known
            category: compute
            metrics: []
            expressions: []
        ";
        let config = config::Config::from_config_str(config_str);
        let state = types::AppState::new(config);
        let mut app = get_v1_routes().with_state(state);

        let request = Request::builder()
            .uri("/health?service=unknown-service&environment=prod&from=now-1h&to=now")
            .body(Body::empty())
            .unwrap();

        let response = app.ready().await.unwrap().call(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert!(body["message"].as_str().unwrap().contains("not supported"));
    }

    /// T052: Test /api/v1/health with missing params returns 400
    #[tokio::test]
    async fn test_v1_health_missing_params() {
        let config_str = "
        datasource:
          url: 'https://graphite.example.com'
        server:
          port: 3000
        environments:
          - name: prod
        flag_metrics: []
        health_metrics: {}
        ";
        let config = config::Config::from_config_str(config_str);
        let state = types::AppState::new(config);
        let mut app = get_v1_routes().with_state(state);

        // Missing required query parameters
        let request = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        let response = app.ready().await.unwrap().call(request).await.unwrap();
        // Axum returns 400 BAD_REQUEST for missing required query params
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
