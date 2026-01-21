// Integration tests for service health calculation
// 
// These tests verify end-to-end health calculation flows with mocked Graphite responses

use cloudmon_metrics::{
    common::get_service_health,
    config::{Config, Datasource, ServerConf},
    types::{AppState, CmpType, FlagMetric, MetricExpressionDef, ServiceHealthDef, EnvironmentDef},
};
use std::collections::HashMap;

// Helper to create a comprehensive test state for integration testing
fn create_integration_test_state(graphite_url: &str) -> AppState {
    let config = Config {
        datasource: Datasource {
            url: graphite_url.to_string(),
            timeout: 30,
        },
        server: ServerConf {
            address: "127.0.0.1".to_string(),
            port: 3000,
        },
        metric_templates: Some(HashMap::new()),
        flag_metrics: Vec::new(),
        health_metrics: HashMap::new(),
        environments: vec![EnvironmentDef {
            name: "production".to_string(),
            attributes: None,
        }],
        status_dashboard: None,
    };

    let mut state = AppState::new(config);
    
    // Setup comprehensive flag metrics for integration testing
    let metrics = vec![
        ("cpu_usage", CmpType::Lt, 80.0),
        ("memory_usage", CmpType::Lt, 90.0),
        ("error_rate", CmpType::Lt, 5.0),
    ];
    
    for (name, op, threshold) in metrics {
        let metric_key = format!("api-service.{}", name);
        let mut env_map = HashMap::new();
        env_map.insert(
            "production".to_string(),
            FlagMetric {
                query: format!("stats.api-service.production.{}", name),
                op: op.clone(),
                threshold,
            },
        );
        state.flag_metrics.insert(metric_key, env_map);
    }
    
    // Setup health metrics with multiple weighted expressions
    let metric_names = vec![
        "api-service.cpu_usage".to_string(),
        "api-service.memory_usage".to_string(),
        "api-service.error_rate".to_string(),
    ];
    
    let expressions = vec![
        MetricExpressionDef {
            expression: "api_service.error_rate".to_string(),
            weight: 100, // Critical: High error rate
        },
        MetricExpressionDef {
            expression: "api_service.cpu_usage && api_service.memory_usage".to_string(),
            weight: 50, // Warning: High resource usage
        },
        MetricExpressionDef {
            expression: "api_service.cpu_usage || api_service.memory_usage || api_service.error_rate".to_string(),
            weight: 30, // Info: Any metric flagged
        },
    ];
    
    state.health_metrics.insert(
        "api-service".to_string(),
        ServiceHealthDef {
            service: "api-service".to_string(),
            component_name: None,
            category: "compute".to_string(),
            metrics: metric_names,
            expressions,
        },
    );
    
    state.services.insert("api-service".to_string());
    state
}

// T034: End-to-end health calculation test with mocked Graphite
#[tokio::test]
async fn test_integration_health_calculation_end_to_end() {
    let mut server = mockito::Server::new_async().await;
    let mock_url = server.url();
    
    let state = create_integration_test_state(&mock_url);
    
    // Mock Graphite response with all three metrics
    // Scenario: High error rate (critical), normal CPU and memory
    let _mock = server
        .mock("GET", "/render")
        .match_query(mockito::Matcher::AllOf(vec![
            mockito::Matcher::UrlEncoded("format".into(), "json".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[
                {"target":"api-service.cpu_usage","datapoints":[[50.0,1234567890]]},
                {"target":"api-service.memory_usage","datapoints":[[60.0,1234567890]]},
                {"target":"api-service.error_rate","datapoints":[[10.0,1234567890]]}
            ]"#,
        )
        .create();
    
    let result = get_service_health(
        &state,
        "api-service",
        "production",
        "2024-01-01T00:00:00Z",
        "2024-01-01T01:00:00Z",
        100,
    )
    .await;
    
    assert!(result.is_ok(), "End-to-end health calculation should succeed");
    let health_data = result.unwrap();
    assert_eq!(health_data.len(), 1, "Should have one datapoint");
    
    // Error rate is 10.0 (> 5.0), so error_rate flag is false
    // CPU (50 < 80) and memory (60 < 90) are normal, so those flags are true
    // Expression evaluation:
    // - "error_rate" alone: false → skip (weight 100)
    // - "cpu && memory": true && true = true → weight 50 ✓ highest match
    // - "cpu || memory || error": true → weight 30
    // Highest matching expression = 50
    assert_eq!(
        health_data[0].1, 50,
        "Should return weight 50 (cpu && memory) since both resource metrics are true"
    );
}

// T035: Complex weighted expression scenarios
#[tokio::test]
async fn test_integration_complex_weighted_expressions() {
    let mut server = mockito::Server::new_async().await;
    let mock_url = server.url();
    
    let state = create_integration_test_state(&mock_url);
    
    // Mock Graphite response
    // Scenario: All metrics in good state - all flags should be true
    let _mock = server
        .mock("GET", "/render")
        .match_query(mockito::Matcher::AllOf(vec![
            mockito::Matcher::UrlEncoded("format".into(), "json".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[
                {"target":"api-service.cpu_usage","datapoints":[[70.0,1234567890]]},
                {"target":"api-service.memory_usage","datapoints":[[85.0,1234567890]]},
                {"target":"api-service.error_rate","datapoints":[[2.0,1234567890]]}
            ]"#,
        )
        .create();
    
    let result = get_service_health(
        &state,
        "api-service",
        "production",
        "2024-01-01T00:00:00Z",
        "2024-01-01T01:00:00Z",
        100,
    )
    .await;
    
    assert!(result.is_ok(), "Complex weighted expressions should succeed");
    let health_data = result.unwrap();
    
    // All flags are true:
    // - error_rate: 2.0 < 5.0 = true → weight 100
    // - cpu && memory: true && true = true → weight 50
    // - cpu || memory || error: true → weight 30
    // Highest weight = 100
    assert_eq!(
        health_data[0].1, 100,
        "Should return highest weight (100) when error_rate flag is true"
    );
}

// T036: Edge cases - empty datapoints and partial data
#[tokio::test]
async fn test_integration_edge_cases_empty_and_partial_data() {
    let mut server = mockito::Server::new_async().await;
    let mock_url = server.url();
    
    let state = create_integration_test_state(&mock_url);
    
    // Test 1: Empty datapoints array
    let _mock1 = server
        .mock("GET", "/render")
        .match_query(mockito::Matcher::AllOf(vec![
            mockito::Matcher::UrlEncoded("format".into(), "json".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[
                {"target":"api-service.cpu_usage","datapoints":[]},
                {"target":"api-service.memory_usage","datapoints":[]},
                {"target":"api-service.error_rate","datapoints":[]}
            ]"#,
        )
        .create();
    
    let result = get_service_health(
        &state,
        "api-service",
        "production",
        "2024-01-01T00:00:00Z",
        "2024-01-01T01:00:00Z",
        100,
    )
    .await;
    
    // Empty datapoints should result in empty health data
    assert!(result.is_ok(), "Empty datapoints should be handled gracefully");
    let health_data = result.unwrap();
    assert_eq!(health_data.len(), 0, "Empty datapoints should produce empty result");
    
    // Test 2: Partial data (some metrics missing datapoints)
    let _mock2 = server
        .mock("GET", "/render")
        .match_query(mockito::Matcher::AllOf(vec![
            mockito::Matcher::UrlEncoded("format".into(), "json".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[
                {"target":"api-service.cpu_usage","datapoints":[[50.0,1234567900]]},
                {"target":"api-service.memory_usage","datapoints":[]},
                {"target":"api-service.error_rate","datapoints":[[2.0,1234567900]]}
            ]"#,
        )
        .create();
    
    let result2 = get_service_health(
        &state,
        "api-service",
        "production",
        "2024-01-01T00:10:00Z",
        "2024-01-01T01:10:00Z",
        100,
    )
    .await;
    
    // Partial data: only metrics with datapoints are evaluated
    // Missing metrics default to false in expression context
    assert!(result2.is_ok(), "Partial data should be handled gracefully");
    let health_data2 = result2.unwrap();
    assert!(health_data2.len() > 0, "Should have results for timestamps with partial data");
    
    // With cpu=true, memory=false (missing), error=true:
    // - error_rate alone: true → 100
    // - cpu && memory: true && false = false
    // - cpu || memory || error: true → 30
    // Highest = 100
    assert_eq!(
        health_data2[0].1, 100,
        "Partial data should still evaluate expressions correctly with missing metrics as false"
    );
}
