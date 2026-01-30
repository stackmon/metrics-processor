// Integration tests for service health calculation
//
// These tests verify end-to-end health calculation flows with mocked Graphite responses

mod fixtures;

use cloudmon_metrics::common::get_service_health;
use fixtures::{graphite_responses, helpers};

// T034: End-to-end health calculation test with mocked Graphite
#[tokio::test]
async fn test_integration_health_calculation_end_to_end() {
    let mut server = mockito::Server::new_async().await;
    let mock_url = server.url();

    let state = helpers::create_health_test_state(&mock_url);

    // Mock Graphite response with all three metrics
    // Scenario: High error rate (critical), normal CPU and memory
    let _mock = helpers::setup_graphite_render_mock_async(
        &mut server,
        graphite_responses::api_service_health_response(50.0, 60.0, 10.0, 1234567890),
    );

    let result = get_service_health(
        &state,
        "api-service",
        "production",
        "2024-01-01T00:00:00Z",
        "2024-01-01T01:00:00Z",
        100,
    )
    .await;

    assert!(
        result.is_ok(),
        "End-to-end health calculation should succeed"
    );
    let health_data = result.unwrap();
    assert_eq!(health_data.len(), 1, "Should have one datapoint");

    // Error rate is 10.0 (> 5.0), so error_rate flag is false
    // CPU (50 < 80) and memory (60 < 90) are normal, so those flags are true
    // Expression evaluation:
    // - "error_rate" alone: false → skip (weight 100)
    // - "cpu && memory": true && true = true → weight 50 ✓ highest match
    // - "cpu || memory || error": true → weight 30
    // Highest matching expression = 50
    helpers::assert_health_score(
        health_data[0].weight,
        50,
        "cpu && memory should match since both resource metrics are true",
    );
}

// T035: Complex weighted expression scenarios
#[tokio::test]
async fn test_integration_complex_weighted_expressions() {
    let mut server = mockito::Server::new_async().await;
    let mock_url = server.url();

    let state = helpers::create_health_test_state(&mock_url);

    // Mock Graphite response
    // Scenario: All metrics in good state - all flags should be true
    let _mock = helpers::setup_graphite_render_mock_async(
        &mut server,
        graphite_responses::api_service_health_response(70.0, 85.0, 2.0, 1234567890),
    );

    let result = get_service_health(
        &state,
        "api-service",
        "production",
        "2024-01-01T00:00:00Z",
        "2024-01-01T01:00:00Z",
        100,
    )
    .await;

    assert!(
        result.is_ok(),
        "Complex weighted expressions should succeed"
    );
    let health_data = result.unwrap();

    // All flags are true:
    // - error_rate: 2.0 < 5.0 = true → weight 100
    // - cpu && memory: true && true = true → weight 50
    // - cpu || memory || error: true → weight 30
    // Highest weight = 100
    helpers::assert_health_score(
        health_data[0].weight,
        100,
        "highest weight (100) when error_rate flag is true",
    );
}

// T036: Edge cases - empty datapoints and partial data
#[tokio::test]
async fn test_integration_edge_cases_empty_and_partial_data() {
    let mut server = mockito::Server::new_async().await;
    let mock_url = server.url();

    let state = helpers::create_health_test_state(&mock_url);

    // Test 1: Empty datapoints array
    let _mock1 = helpers::setup_graphite_render_mock_async(
        &mut server,
        graphite_responses::api_service_empty_response(),
    );

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
    assert!(
        result.is_ok(),
        "Empty datapoints should be handled gracefully"
    );
    let health_data = result.unwrap();
    assert_eq!(
        health_data.len(),
        0,
        "Empty datapoints should produce empty result"
    );

    // Test 2: Partial data (some metrics missing datapoints)
    let _mock2 = helpers::setup_graphite_render_mock_async(
        &mut server,
        graphite_responses::api_service_partial_response(50.0, 2.0, 1234567900),
    );

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
    assert!(
        health_data2.len() > 0,
        "Should have results for timestamps with partial data"
    );

    // With cpu=true, memory=false (missing), error=true:
    // - error_rate alone: true → 100
    // - cpu && memory: true && false = false
    // - cpu || memory || error: true → 30
    // Highest = 100
    helpers::assert_health_score(
        health_data2[0].weight,
        100,
        "Partial data should evaluate expressions correctly with missing metrics as false",
    );
}
