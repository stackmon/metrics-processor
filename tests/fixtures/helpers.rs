// Test helper functions and custom assertions
//
// Provides utilities for creating test state, mocking Graphite responses,
// and custom assertions for clearer test failure messages

use cloudmon_metrics::{
    types::{AppState, ComparisonOperator, Metric, HealthMetric},
    config::Config,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Creates a minimal test AppState for unit testing
///
/// # Example
/// ```
/// let state = create_test_state();
/// assert!(state.services.contains("test-service"));
/// ```
pub fn create_test_state() -> Arc<AppState> {
    let mut services = HashSet::new();
    services.insert("test-service".to_string());

    let mut environments = HashMap::new();
    let mut env_set = HashSet::new();
    env_set.insert("production".to_string());
    environments.insert("test-service".to_string(), env_set);

    let mut metrics = HashMap::new();
    let test_metric = Metric {
        name: "error_rate".to_string(),
        graphite_query: "stats.test-service.$environment.errors".to_string(),
        operator: ComparisonOperator::Lt,
        threshold: 5.0,
        thresholds: HashMap::new(),
    };
    
    let mut service_metrics = HashMap::new();
    let mut env_metrics = HashMap::new();
    env_metrics.insert("error_rate".to_string(), test_metric);
    service_metrics.insert("production".to_string(), env_metrics);
    metrics.insert("test-service".to_string(), service_metrics);

    let mut health_metrics = HashMap::new();
    let health_metric = HealthMetric {
        expression: "error_rate".to_string(),
        weight: 100,
    };
    
    let mut service_health = HashMap::new();
    let mut env_health = Vec::new();
    env_health.push(health_metric);
    service_health.insert("production".to_string(), env_health);
    health_metrics.insert("test-service".to_string(), service_health);

    Arc::new(AppState {
        services,
        environments,
        metrics,
        health_metrics,
        graphite_url: "http://localhost:9090".to_string(),
    })
}

/// Creates a test AppState with custom Graphite URL for mocking
///
/// # Arguments
/// * `graphite_url` - URL of the mock Graphite server
///
/// # Example
/// ```
/// let mut server = mockito::Server::new();
/// let state = create_test_state_with_mock_url(&server.url());
/// ```
pub fn create_test_state_with_mock_url(graphite_url: &str) -> Arc<AppState> {
    let mut state = (*create_test_state()).clone();
    state.graphite_url = graphite_url.to_string();
    Arc::new(state)
}

/// Creates a test AppState with custom service, environment, and metrics
///
/// # Arguments
/// * `service` - Service name
/// * `environment` - Environment name
/// * `metric_name` - Name of the metric
/// * `operator` - Comparison operator (Lt, Gt, Eq)
/// * `threshold` - Threshold value
/// * `graphite_url` - URL of the mock Graphite server
pub fn create_custom_test_state(
    service: &str,
    environment: &str,
    metric_name: &str,
    operator: ComparisonOperator,
    threshold: f64,
    graphite_url: &str,
) -> Arc<AppState> {
    let mut services = HashSet::new();
    services.insert(service.to_string());

    let mut environments = HashMap::new();
    let mut env_set = HashSet::new();
    env_set.insert(environment.to_string());
    environments.insert(service.to_string(), env_set);

    let mut metrics = HashMap::new();
    let metric = Metric {
        name: metric_name.to_string(),
        graphite_query: format!("stats.{}.{}.{}", service, environment, metric_name),
        operator,
        threshold,
        thresholds: HashMap::new(),
    };
    
    let mut service_metrics = HashMap::new();
    let mut env_metrics = HashMap::new();
    env_metrics.insert(metric_name.to_string(), metric);
    service_metrics.insert(environment.to_string(), env_metrics);
    metrics.insert(service.to_string(), service_metrics);

    let mut health_metrics = HashMap::new();
    let health_metric = HealthMetric {
        expression: metric_name.to_string(),
        weight: 100,
    };
    
    let mut service_health = HashMap::new();
    let mut env_health = Vec::new();
    env_health.push(health_metric);
    service_health.insert(environment.to_string(), env_health);
    health_metrics.insert(service.to_string(), service_health);

    Arc::new(AppState {
        services,
        environments,
        metrics,
        health_metrics,
        graphite_url: graphite_url.to_string(),
    })
}

/// Creates a test AppState with multiple metrics for complex expression testing
pub fn create_multi_metric_test_state(
    service: &str,
    environment: &str,
    metrics_config: Vec<(&str, ComparisonOperator, f64)>,
    health_expressions: Vec<(&str, u32)>,
    graphite_url: &str,
) -> Arc<AppState> {
    let mut services = HashSet::new();
    services.insert(service.to_string());

    let mut environments = HashMap::new();
    let mut env_set = HashSet::new();
    env_set.insert(environment.to_string());
    environments.insert(service.to_string(), env_set);

    let mut metrics = HashMap::new();
    let mut env_metrics = HashMap::new();
    
    for (name, operator, threshold) in metrics_config {
        let metric = Metric {
            name: name.to_string(),
            graphite_query: format!("stats.{}.{}.{}", service, environment, name),
            operator,
            threshold,
            thresholds: HashMap::new(),
        };
        env_metrics.insert(name.to_string(), metric);
    }
    
    let mut service_metrics = HashMap::new();
    service_metrics.insert(environment.to_string(), env_metrics);
    metrics.insert(service.to_string(), service_metrics);

    let mut health_metrics = HashMap::new();
    let mut env_health = Vec::new();
    
    for (expression, weight) in health_expressions {
        env_health.push(HealthMetric {
            expression: expression.to_string(),
            weight,
        });
    }
    
    let mut service_health = HashMap::new();
    service_health.insert(environment.to_string(), env_health);
    health_metrics.insert(service.to_string(), service_health);

    Arc::new(AppState {
        services,
        environments,
        metrics,
        health_metrics,
        graphite_url: graphite_url.to_string(),
    })
}

/// Custom assertion for metric flag evaluation results
///
/// Provides clear error messages when flag evaluation doesn't match expected result
///
/// # Arguments
/// * `actual` - Actual flag value returned
/// * `expected` - Expected flag value
/// * `context` - Description of the test scenario
///
/// # Panics
/// Panics with descriptive message if actual != expected
pub fn assert_metric_flag(actual: bool, expected: bool, context: &str) {
    assert_eq!(
        actual, expected,
        "Metric flag evaluation failed for {}: expected {}, got {}",
        context, expected, actual
    );
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
pub fn assert_health_score(actual: u32, expected: u32, context: &str) {
    assert_eq!(
        actual, expected,
        "Health score calculation failed for {}: expected {}, got {}",
        context, expected, actual
    );
}

/// Custom assertion for health score within tolerance
///
/// Useful for floating-point comparison scenarios
///
/// # Arguments
/// * `actual` - Actual health score returned
/// * `expected` - Expected health score
/// * `tolerance` - Acceptable difference
/// * `context` - Description of the test scenario
pub fn assert_health_score_within(actual: u32, expected: u32, tolerance: u32, context: &str) {
    let diff = if actual > expected {
        actual - expected
    } else {
        expected - actual
    };
    
    assert!(
        diff <= tolerance,
        "Health score calculation failed for {}: expected {} ± {}, got {} (diff: {})",
        context, expected, tolerance, actual, diff
    );
}

/// Helper to parse YAML config string into Config struct
///
/// # Arguments
/// * `yaml` - YAML configuration string
///
/// # Returns
/// Result containing parsed Config or error message
pub fn parse_test_config(yaml: &str) -> Result<Config, String> {
    serde_yaml::from_str(yaml).map_err(|e| format!("Failed to parse config: {}", e))
}

/// Helper to create a temporary config file for testing
///
/// # Arguments
/// * `content` - YAML content to write
///
/// # Returns
/// Path to the temporary file
#[cfg(test)]
pub fn create_temp_config_file(content: &str) -> tempfile::NamedTempFile {
    use std::io::Write;
    let mut file = tempfile::NamedTempFile::new().expect("Failed to create temp file");
    file.write_all(content.as_bytes()).expect("Failed to write to temp file");
    file.flush().expect("Failed to flush temp file");
    file
}

/// Helper to assert that a result is an error with specific message pattern
///
/// # Arguments
/// * `result` - Result to check
/// * `expected_pattern` - Expected error message substring
pub fn assert_error_contains<T>(result: Result<T, String>, expected_pattern: &str) 
where
    T: std::fmt::Debug,
{
    match result {
        Ok(val) => panic!("Expected error containing '{}', but got Ok({:?})", expected_pattern, val),
        Err(err) => assert!(
            err.contains(expected_pattern),
            "Error message '{}' does not contain expected pattern '{}'",
            err, expected_pattern
        ),
    }
}

/// Helper to create mock Graphite response for a specific metric value
///
/// # Arguments
/// * `metric_name` - Name of the metric
/// * `value` - Value to return in datapoint
/// * `timestamp` - Unix timestamp for datapoint
///
/// # Returns
/// JSON value suitable for mockito response body
pub fn mock_metric_response(metric_name: &str, value: Option<f64>, timestamp: i64) -> serde_json::Value {
    use serde_json::json;
    
    let datapoint = if let Some(v) = value {
        json!([v, timestamp])
    } else {
        json!([null, timestamp])
    };
    
    json!([
        {
            "target": metric_name,
            "datapoints": [datapoint]
        }
    ])
}

/// Helper to setup a mockito mock with common Graphite query parameters
///
/// # Arguments
/// * `server` - Mockito server instance
/// * `target` - Graphite target/query
/// * `response_body` - JSON response to return
///
/// # Returns
/// Configured mockito::Mock ready to be created
pub fn setup_graphite_mock(
    server: &mut mockito::Server,
    target: &str,
    response_body: serde_json::Value,
) -> mockito::Mock {
    server
        .mock("GET", "/render")
        .match_query(mockito::Matcher::AllOf(vec![
            mockito::Matcher::UrlEncoded("format".into(), "json".into()),
            mockito::Matcher::UrlEncoded("target".into(), target.into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(response_body.to_string())
        .create()
}
