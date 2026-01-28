//! T059: Full API integration test with mocked Graphite
//! T060: Error response format validation
//!
//! Integration tests for API endpoints with mocked Graphite backend

mod fixtures;

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use cloudmon_metrics::{api, config, graphite, types};
use fixtures::{configs, graphite_responses, helpers};
use serde_json::{json, Value};
use tower::ServiceExt;

/// T059: Create full API integration test with mocked Graphite
#[tokio::test]
async fn test_api_integration_with_mocked_graphite() {
    // Create mock Graphite server
    let mut server = mockito::Server::new();

    // Mock the /render endpoint to return sample metric data
    let _mock =
        helpers::setup_graphite_render_mock(&mut server, graphite_responses::webapp_cpu_response());

    // Create application state with mock URL using fixtures
    let state = helpers::create_api_test_state(&server.url());

    // Create combined router with both API routes
    let app = Router::new()
        .nest("/api/v1", api::v1::get_v1_routes())
        .merge(graphite::get_graphite_routes())
        .with_state(state);

    // Test 1: API v1 root endpoint
    let request = Request::builder()
        .uri("/api/v1")
        .body(Body::empty())
        .unwrap();
    let response = ServiceExt::<Request<Body>>::oneshot(app, request)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["name"], "v1");
}

/// Additional test for metrics endpoints
#[tokio::test]
async fn test_graphite_endpoints_integration() {
    // Create application state using fixtures
    let state = helpers::create_api_test_state("https://mock.example.com");

    let app = Router::new()
        .merge(graphite::get_graphite_routes())
        .with_state(state);

    // Test metrics/find endpoint
    let request = Request::builder()
        .uri("/metrics/find?query=*")
        .body(Body::empty())
        .unwrap();
    let response = ServiceExt::<Request<Body>>::oneshot(app, request)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert!(body.is_array());
    let arr = body.as_array().unwrap();
    assert!(arr.len() >= 2); // Should have flag and health
}

/// Test for functions and tags endpoints
#[tokio::test]
async fn test_graphite_utility_endpoints() {
    let state = helpers::create_api_test_state("https://mock.example.com");

    // Test functions endpoint
    let app1 = Router::new()
        .merge(graphite::get_graphite_routes())
        .with_state(state.clone());
    let request = Request::builder()
        .uri("/functions")
        .body(Body::empty())
        .unwrap();
    let response = ServiceExt::<Request<Body>>::oneshot(app1, request)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body, json!({}));

    // Test tags endpoint
    let app2 = Router::new()
        .merge(graphite::get_graphite_routes())
        .with_state(state);
    let request = Request::builder()
        .uri("/tags/autoComplete/tags")
        .body(Body::empty())
        .unwrap();
    let response = ServiceExt::<Request<Body>>::oneshot(app2, request)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body, json!([]));
}

/// T060: Test error response format validation
#[tokio::test]
async fn test_error_response_format() {
    let config_str = configs::empty_health_config("https://mock-graphite.example.com");
    let config = config::Config::from_config_str(&config_str);
    let state = types::AppState::new(config);

    let app = Router::new()
        .nest("/api/v1", api::v1::get_v1_routes())
        .with_state(state);

    // Test 1: Unknown service error (409 CONFLICT)
    let request = Request::builder()
        .uri("/api/v1/health?service=unknown&environment=prod&from=now-1h&to=now")
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();

    // Verify error response format has "message" field
    assert!(body.get("message").is_some());
    assert!(body["message"].is_string());
    let message = body["message"].as_str().unwrap();
    assert!(message.contains("not supported") || message.contains("Service not supported"));

    // Test 2: Missing parameters error (400 BAD_REQUEST)
    let request = Request::builder()
        .uri("/api/v1/health?service=known-service")
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Test 3: Invalid endpoint (404 NOT_FOUND)
    let request = Request::builder()
        .uri("/api/v1/nonexistent")
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// Additional test: Verify health endpoint with known service but environment not supported
#[tokio::test]
async fn test_health_endpoint_unsupported_environment() {
    let config_str = configs::error_test_config("https://mock-graphite.example.com");
    let config = config::Config::from_config_str(&config_str);
    let mut state = types::AppState::new(config);
    state.process_config();

    let app = Router::new()
        .nest("/api/v1", api::v1::get_v1_routes())
        .with_state(state);

    // Request with unsupported environment
    let request = Request::builder()
        .uri("/api/v1/health?service=webapp&environment=staging&from=now-1h&to=now")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();

    // Should return CONFLICT status
    assert_eq!(response.status(), StatusCode::CONFLICT);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();

    // Verify error message format
    assert!(body.get("message").is_some());
    let message = body["message"].as_str().unwrap();
    assert!(message.contains("not supported"));
}
