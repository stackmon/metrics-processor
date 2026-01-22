// Test helper functions and custom assertions
//
// Provides utilities for creating test state, mocking Graphite responses,
// and custom assertions for clearer test failure messages

use cloudmon_metrics::{
    config::Config,
    types::AppState,
};

/// Creates a test AppState for API integration testing with multiple services
///
/// # Arguments
/// * `graphite_url` - URL of the mock Graphite server
pub fn create_api_test_state(graphite_url: &str) -> AppState {
    let config_str = format!(r#"
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
          - name: staging
        flag_metrics:
          - name: cpu-usage
            service: webapp
            template:
              name: cpu_tmpl
            environments:
              - name: prod
              - name: staging
        health_metrics:
          webapp:
            service: webapp
            category: compute
            metrics:
              - webapp.cpu-usage
            expressions:
              - expression: 'webapp.cpu_usage'
                weight: 2
    "#, graphite_url);

    let config = Config::from_config_str(&config_str);
    let mut state = AppState::new(config);
    state.process_config();
    state
}

/// Creates a test AppState for health integration testing with multiple metrics
///
/// # Arguments
/// * `graphite_url` - URL of the mock Graphite server
pub fn create_health_test_state(graphite_url: &str) -> AppState {
    let config_str = format!(r#"
        datasource:
          url: '{}'
        server:
          port: 3000
        metric_templates:
          cpu_tmpl:
            query: 'stats.$service.$environment.cpu_usage'
            op: lt
            threshold: 80.0
          memory_tmpl:
            query: 'stats.$service.$environment.memory_usage'
            op: lt
            threshold: 90.0
          error_tmpl:
            query: 'stats.$service.$environment.error_rate'
            op: lt
            threshold: 5.0
        environments:
          - name: production
        flag_metrics:
          - name: cpu_usage
            service: api-service
            template:
              name: cpu_tmpl
            environments:
              - name: production
          - name: memory_usage
            service: api-service
            template:
              name: memory_tmpl
            environments:
              - name: production
          - name: error_rate
            service: api-service
            template:
              name: error_tmpl
            environments:
              - name: production
        health_metrics:
          api-service:
            service: api-service
            category: compute
            metrics:
              - api-service.cpu_usage
              - api-service.memory_usage
              - api-service.error_rate
            expressions:
              - expression: 'api_service.error_rate'
                weight: 100
              - expression: 'api_service.cpu_usage && api_service.memory_usage'
                weight: 50
              - expression: 'api_service.cpu_usage || api_service.memory_usage || api_service.error_rate'
                weight: 30
    "#, graphite_url);

    let config = Config::from_config_str(&config_str);
    let mut state = AppState::new(config);
    state.process_config();
    state
}

/// Custom assertion for health score results
///
/// Provides clear error messages when health score doesn't match expected value
///
/// # Arguments
/// * `actual` - Actual health score returned
/// * `expected` - Expected health score
/// * `context` - Description of the test scenario
///
/// # Panics
/// Panics with descriptive message if actual != expected
pub fn assert_health_score(actual: u8, expected: u8, context: &str) {
    assert_eq!(
        actual, expected,
        "Health score calculation failed for {}: expected {}, got {}",
        context, expected, actual
    );
}


/// Helper to setup a mockito mock with common Graphite query parameters
///
/// # Arguments
/// * `server` - Mockito server instance
/// * `response_body` - JSON response to return
///
/// # Returns
/// Configured mockito::Mock ready to be created
pub fn setup_graphite_render_mock(
    server: &mut mockito::Server,
    response_body: serde_json::Value,
) -> mockito::Mock {
    server
        .mock("GET", "/render")
        .match_query(mockito::Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(response_body.to_string())
        .create()
}

/// Async version of setup_graphite_render_mock for async tests
///
/// # Arguments
/// * `server` - Mockito server instance
/// * `response_body` - JSON response to return
///
/// # Returns
/// Configured mockito::Mock ready to be created
pub fn setup_graphite_render_mock_async(
    server: &mut mockito::ServerGuard,
    response_body: serde_json::Value,
) -> mockito::Mock {
    server
        .mock("GET", "/render")
        .match_query(mockito::Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(response_body.to_string())
        .create()
}
