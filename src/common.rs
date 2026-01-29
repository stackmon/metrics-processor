//! Common methods
//!
use crate::types::{AppState, CloudMonError, CmpType, FlagMetric, ServiceHealthData};
use chrono::DateTime;
use evalexpr::*;
use std::collections::{BTreeMap, HashMap};

use crate::graphite;

/// Get Flag value for the metric
pub fn get_metric_flag_state(value: &Option<f32>, metric: &FlagMetric) -> bool {
    // Convert raw value to flag
    match *value {
        Some(x) => match metric.op {
            CmpType::Lt => x < metric.threshold,
            CmpType::Gt => x > metric.threshold,
            CmpType::Eq => x == metric.threshold,
        },
        None => false,
    }
}
/// Get Service Health as described by config
pub async fn get_service_health(
    state: &AppState,
    service: &str,
    environment: &str,
    from: &str,
    to: &str,
    max_data_points: u16,
) -> Result<ServiceHealthData, CloudMonError> {
    if !state.health_metrics.contains_key(service) {
        return Err(CloudMonError::ServiceNotSupported);
    }
    let hm_config = state.health_metrics.get(service).unwrap();
    let metric_names: Vec<String> = hm_config.metrics.clone();

    tracing::trace!("Requesting metrics {:?}", metric_names);
    let mut graphite_targets: HashMap<String, String> = HashMap::new();
    // Construct target=>query map
    for metric_name in metric_names.iter() {
        if let Some(metric) = state.flag_metrics.get(metric_name) {
            match metric.get(environment) {
                Some(m) => {
                    graphite_targets.insert(metric_name.clone(), m.query.clone());
                }
                _ => {
                    tracing::debug!(
                        "Can not find metric {} for env {}",
                        metric_name,
                        environment
                    );
                    return Err(CloudMonError::EnvNotSupported);
                }
            };
        }
    }
    tracing::debug!("Requesting Graphite {:?}", graphite_targets);
    let raw_data: Vec<graphite::GraphiteData> = graphite::get_graphite_data(
        &state.req_client,
        state.config.datasource.url.as_str(),
        &graphite_targets,
        DateTime::parse_from_rfc3339(from).ok(),
        Some(from.to_string()),
        DateTime::parse_from_rfc3339(to).ok(),
        Some(to.to_string()),
        max_data_points,
    )
    .await?;

    tracing::trace!("Response from Graphite {:?}", raw_data);

    let mut result: ServiceHealthData = Vec::new();
    // Iterate over all data elements and reorg them for health evaluation
    let mut metrics_map: BTreeMap<u32, HashMap<String, bool>> = BTreeMap::new();
    for data_element in raw_data.iter() {
        // target + datapoints
        tracing::trace!("Processing dataframe {:?}", data_element);
        match state.flag_metrics.get(&data_element.target) {
            Some(metric_cfg) => {
                // if metric is known to us
                tracing::trace!("Processing datapoints for metric {:?}", metric_cfg);
                let metric = metric_cfg.get(environment).unwrap();
                // Iterate over all fetched series
                for (val, ts) in data_element.datapoints.iter() {
                    // Convert raw value to flag
                    if val.is_some() {
                        metrics_map.entry(*ts).or_default().insert(
                            data_element.target.clone(),
                            get_metric_flag_state(val, metric),
                        );
                    }
                }
            }
            None => {
                tracing::warn!(
                    "DB Response contains unknown target: {}",
                    data_element.target
                );
            }
        }
    }
    tracing::trace!("Metric map = {:?}", metrics_map);

    // Loop through data map and evaluate health
    let hm_config = state.health_metrics.get(service).unwrap();
    for (ts, ts_val) in metrics_map.iter() {
        let mut context = HashMapContext::new();
        // build context with all metrics
        for metric in hm_config.metrics.iter() {
            let xval = match ts_val.get(metric) {
                Some(&x) => x,
                _ => false,
            };
            context
                .set_value(metric.replace("-", "_"), Value::from(xval))
                .unwrap();
        }
        let mut expression_res: u8 = 0;
        // loop over all expressions
        for expr in hm_config.expressions.iter() {
            // if expression weight is lower than what we have already - skip
            if expr.weight as u8 <= expression_res {
                continue;
            }
            match eval_boolean_with_context(expr.expression.as_str(), &context) {
                Ok(m) => {
                    if m {
                        expression_res = expr.weight as u8;
                        tracing::debug!(
                            "Summary of evaluation expression for service: {:?}, expression: {:?}, weight: {:?}",
                            service,
                            expr.expression,
                            expr.weight
                        );
                    }
                }
                Err(e) => {
                    tracing::debug!(
                        "Error during evaluation of {:?} [context: {:?}]: {:?}",
                        expr.expression,
                        context,
                        e
                    );
                    return Err(CloudMonError::ExpressionError);
                }
            }
        }
        result.push((*ts, expression_res));
    }

    tracing::debug!("Summary data: {:?}, length={}", result, result.len());

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        CmpType, EnvironmentDef, FlagMetric, MetricExpressionDef, ServiceHealthDef,
    };

    // Helper function to create a test metric
    fn create_test_metric(op: CmpType, threshold: f32) -> FlagMetric {
        FlagMetric {
            query: "test.query".to_string(),
            op,
            threshold,
        }
    }

    // T010: Test Lt operator with value < threshold returns true
    #[test]
    fn test_lt_operator_below_threshold() {
        let metric = create_test_metric(CmpType::Lt, 10.0);
        let value = Some(5.0);
        let result = get_metric_flag_state(&value, &metric);
        assert_eq!(result, true, "Lt operator: 5.0 < 10.0 should return true");
    }

    // T011: Test Lt operator with value >= threshold returns false
    #[test]
    fn test_lt_operator_above_or_equal_threshold() {
        let metric = create_test_metric(CmpType::Lt, 10.0);

        // Test equal
        let value = Some(10.0);
        let result = get_metric_flag_state(&value, &metric);
        assert_eq!(
            result, false,
            "Lt operator: 10.0 < 10.0 should return false"
        );

        // Test above
        let value = Some(15.0);
        let result = get_metric_flag_state(&value, &metric);
        assert_eq!(
            result, false,
            "Lt operator: 15.0 < 10.0 should return false"
        );
    }

    // T012: Test Gt operator with value > threshold returns true
    #[test]
    fn test_gt_operator_above_threshold() {
        let metric = create_test_metric(CmpType::Gt, 10.0);
        let value = Some(15.0);
        let result = get_metric_flag_state(&value, &metric);
        assert_eq!(result, true, "Gt operator: 15.0 > 10.0 should return true");
    }

    // T013: Test Gt operator with value <= threshold returns false
    #[test]
    fn test_gt_operator_below_or_equal_threshold() {
        let metric = create_test_metric(CmpType::Gt, 10.0);

        // Test equal
        let value = Some(10.0);
        let result = get_metric_flag_state(&value, &metric);
        assert_eq!(
            result, false,
            "Gt operator: 10.0 > 10.0 should return false"
        );

        // Test below
        let value = Some(5.0);
        let result = get_metric_flag_state(&value, &metric);
        assert_eq!(result, false, "Gt operator: 5.0 > 10.0 should return false");
    }

    // T014: Test Eq operator with value == threshold returns true
    #[test]
    fn test_eq_operator_equal_threshold() {
        let metric = create_test_metric(CmpType::Eq, 10.0);
        let value = Some(10.0);
        let result = get_metric_flag_state(&value, &metric);
        assert_eq!(result, true, "Eq operator: 10.0 == 10.0 should return true");
    }

    // T015: Test Eq operator with value != threshold returns false
    #[test]
    fn test_eq_operator_not_equal_threshold() {
        let metric = create_test_metric(CmpType::Eq, 10.0);

        // Test below
        let value = Some(5.0);
        let result = get_metric_flag_state(&value, &metric);
        assert_eq!(
            result, false,
            "Eq operator: 5.0 == 10.0 should return false"
        );

        // Test above
        let value = Some(15.0);
        let result = get_metric_flag_state(&value, &metric);
        assert_eq!(
            result, false,
            "Eq operator: 15.0 == 10.0 should return false"
        );
    }

    // T016: Test None value always returns false for all operators
    #[test]
    fn test_none_value_returns_false() {
        let value = None;

        // Test with Lt operator
        let metric = create_test_metric(CmpType::Lt, 10.0);
        let result = get_metric_flag_state(&value, &metric);
        assert_eq!(
            result, false,
            "Lt operator with None value should return false"
        );

        // Test with Gt operator
        let metric = create_test_metric(CmpType::Gt, 10.0);
        let result = get_metric_flag_state(&value, &metric);
        assert_eq!(
            result, false,
            "Gt operator with None value should return false"
        );

        // Test with Eq operator
        let metric = create_test_metric(CmpType::Eq, 10.0);
        let result = get_metric_flag_state(&value, &metric);
        assert_eq!(
            result, false,
            "Eq operator with None value should return false"
        );
    }

    // T017: Test boundary conditions (threshold ± 0.001)
    #[test]
    fn test_boundary_conditions() {
        let threshold = 10.0;

        // Lt operator with boundaries
        let metric = create_test_metric(CmpType::Lt, threshold);
        let value_below = Some(threshold - 0.001);
        assert_eq!(
            get_metric_flag_state(&value_below, &metric),
            true,
            "Lt operator: value just below threshold should return true"
        );

        let value_above = Some(threshold + 0.001);
        assert_eq!(
            get_metric_flag_state(&value_above, &metric),
            false,
            "Lt operator: value just above threshold should return false"
        );

        // Gt operator with boundaries
        let metric = create_test_metric(CmpType::Gt, threshold);
        let value_above = Some(threshold + 0.001);
        assert_eq!(
            get_metric_flag_state(&value_above, &metric),
            true,
            "Gt operator: value just above threshold should return true"
        );

        let value_below = Some(threshold - 0.001);
        assert_eq!(
            get_metric_flag_state(&value_below, &metric),
            false,
            "Gt operator: value just below threshold should return false"
        );
    }

    // T018: Test negative values with all operators
    #[test]
    fn test_negative_values() {
        // Lt operator with negative values
        let metric = create_test_metric(CmpType::Lt, -5.0);
        assert_eq!(
            get_metric_flag_state(&Some(-10.0), &metric),
            true,
            "Lt: -10.0 < -5.0 should return true"
        );
        assert_eq!(
            get_metric_flag_state(&Some(-5.0), &metric),
            false,
            "Lt: -5.0 < -5.0 should return false"
        );
        assert_eq!(
            get_metric_flag_state(&Some(0.0), &metric),
            false,
            "Lt: 0.0 < -5.0 should return false"
        );

        // Gt operator with negative values
        let metric = create_test_metric(CmpType::Gt, -5.0);
        assert_eq!(
            get_metric_flag_state(&Some(0.0), &metric),
            true,
            "Gt: 0.0 > -5.0 should return true"
        );
        assert_eq!(
            get_metric_flag_state(&Some(-5.0), &metric),
            false,
            "Gt: -5.0 > -5.0 should return false"
        );
        assert_eq!(
            get_metric_flag_state(&Some(-10.0), &metric),
            false,
            "Gt: -10.0 > -5.0 should return false"
        );

        // Eq operator with negative values
        let metric = create_test_metric(CmpType::Eq, -5.0);
        assert_eq!(
            get_metric_flag_state(&Some(-5.0), &metric),
            true,
            "Eq: -5.0 == -5.0 should return true"
        );
        assert_eq!(
            get_metric_flag_state(&Some(-4.9), &metric),
            false,
            "Eq: -4.9 == -5.0 should return false"
        );
    }

    // T019: Test zero threshold edge case
    #[test]
    fn test_zero_threshold() {
        let threshold = 0.0;

        // Lt operator with zero threshold
        let metric = create_test_metric(CmpType::Lt, threshold);
        assert_eq!(
            get_metric_flag_state(&Some(-1.0), &metric),
            true,
            "Lt: -1.0 < 0.0 should return true"
        );
        assert_eq!(
            get_metric_flag_state(&Some(0.0), &metric),
            false,
            "Lt: 0.0 < 0.0 should return false"
        );
        assert_eq!(
            get_metric_flag_state(&Some(1.0), &metric),
            false,
            "Lt: 1.0 < 0.0 should return false"
        );

        // Gt operator with zero threshold
        let metric = create_test_metric(CmpType::Gt, threshold);
        assert_eq!(
            get_metric_flag_state(&Some(1.0), &metric),
            true,
            "Gt: 1.0 > 0.0 should return true"
        );
        assert_eq!(
            get_metric_flag_state(&Some(0.0), &metric),
            false,
            "Gt: 0.0 > 0.0 should return false"
        );
        assert_eq!(
            get_metric_flag_state(&Some(-1.0), &metric),
            false,
            "Gt: -1.0 > 0.0 should return false"
        );

        // Eq operator with zero threshold
        let metric = create_test_metric(CmpType::Eq, threshold);
        assert_eq!(
            get_metric_flag_state(&Some(0.0), &metric),
            true,
            "Eq: 0.0 == 0.0 should return true"
        );
        assert_eq!(
            get_metric_flag_state(&Some(0.1), &metric),
            false,
            "Eq: 0.1 == 0.0 should return false"
        );
    }

    // T020: Test mixed operators scenario with multiple metrics
    #[test]
    fn test_mixed_operators() {
        // Create metrics with different operators
        let lt_metric = create_test_metric(CmpType::Lt, 50.0);
        let gt_metric = create_test_metric(CmpType::Gt, 10.0);
        let eq_metric = create_test_metric(CmpType::Eq, 42.0);

        // Test value that satisfies Lt condition
        let value = Some(30.0);
        assert_eq!(
            get_metric_flag_state(&value, &lt_metric),
            true,
            "30.0 < 50.0 should be true"
        );
        assert_eq!(
            get_metric_flag_state(&value, &gt_metric),
            true,
            "30.0 > 10.0 should be true"
        );
        assert_eq!(
            get_metric_flag_state(&value, &eq_metric),
            false,
            "30.0 == 42.0 should be false"
        );

        // Test value that satisfies Eq condition
        let value = Some(42.0);
        assert_eq!(
            get_metric_flag_state(&value, &lt_metric),
            true,
            "42.0 < 50.0 should be true"
        );
        assert_eq!(
            get_metric_flag_state(&value, &gt_metric),
            true,
            "42.0 > 10.0 should be true"
        );
        assert_eq!(
            get_metric_flag_state(&value, &eq_metric),
            true,
            "42.0 == 42.0 should be true"
        );

        // Test value that fails all conditions
        let value = Some(5.0);
        assert_eq!(
            get_metric_flag_state(&value, &lt_metric),
            true,
            "5.0 < 50.0 should be true"
        );
        assert_eq!(
            get_metric_flag_state(&value, &gt_metric),
            false,
            "5.0 > 10.0 should be false"
        );
        assert_eq!(
            get_metric_flag_state(&value, &eq_metric),
            false,
            "5.0 == 42.0 should be false"
        );
    }

    // Helper to create test AppState with health metrics
    fn create_health_test_state(
        service: &str,
        environment: &str,
        metrics: Vec<(&str, CmpType, f32)>,
        expressions: Vec<(&str, i32)>,
        graphite_url: &str,
    ) -> AppState {
        use crate::config::{Config, Datasource, HealthQueryConfig, ServerConf};

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
                name: environment.to_string(),
                attributes: None,
            }],
            status_dashboard: None,
            health_query: HealthQueryConfig::default(),
        };

        let mut state = AppState::new(config);

        // Setup flag metrics and collect metric names
        let mut metric_names = Vec::new();
        for (name, op, threshold) in metrics {
            let metric_key = format!("{}.{}", service, name);
            metric_names.push(metric_key.clone());
            let mut env_map = HashMap::new();
            env_map.insert(
                environment.to_string(),
                FlagMetric {
                    query: format!("stats.{}.{}.{}", service, environment, name),
                    op: op.clone(),
                    threshold,
                },
            );
            state.flag_metrics.insert(metric_key, env_map);
        }

        // Setup health metrics
        let expression_defs: Vec<MetricExpressionDef> = expressions
            .into_iter()
            .map(|(expr, weight)| MetricExpressionDef {
                expression: expr.to_string(),
                weight,
            })
            .collect();

        state.health_metrics.insert(
            service.to_string(),
            ServiceHealthDef {
                service: service.to_string(),
                component_name: None,
                category: "test".to_string(),
                metrics: metric_names,
                expressions: expression_defs,
            },
        );

        state.services.insert(service.to_string());
        state
    }

    // T026: Test single metric OR expression evaluates correctly
    #[tokio::test]
    async fn test_single_metric_or_expression() {
        use mockito;

        let mut server = mockito::Server::new_async().await;
        let mock_url = server.url();

        // Setup: single metric "error_rate" with Lt 5.0
        let state = create_health_test_state(
            "test-service",
            "production",
            vec![("error_rate", CmpType::Lt, 5.0)],
            vec![("test_service.error_rate", 100)], // Weight 100 if error_rate flag is true
            &mock_url,
        );

        // Mock Graphite response: error_rate = 2.0 (< 5.0, so flag = true)
        let _mock = server
            .mock("GET", "/render")
            .match_query(mockito::Matcher::AllOf(vec![mockito::Matcher::UrlEncoded(
                "format".into(),
                "json".into(),
            )]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{"target":"test-service.error_rate","datapoints":[[2.0,1234567890]]}]"#)
            .create();

        let result = get_service_health(
            &state,
            "test-service",
            "production",
            "2024-01-01T00:00:00Z",
            "2024-01-01T01:00:00Z",
            100,
        )
        .await;

        if let Err(ref e) = result {
            eprintln!("Error from get_service_health: {:?}", e);
        }
        assert!(
            result.is_ok(),
            "Single metric OR expression should succeed: {:?}",
            result
        );
        let health_data = result.unwrap();
        assert_eq!(health_data.len(), 1, "Should have one datapoint");
        assert_eq!(
            health_data[0].1, 100,
            "Expression weight 100 should be returned when flag is true"
        );
    }

    // T027: Test two metrics AND expression (both true)
    #[tokio::test]
    async fn test_two_metrics_and_both_true() {
        use mockito;

        let mut server = mockito::Server::new_async().await;
        let mock_url = server.url();

        // Setup: two metrics with AND expression
        let state = create_health_test_state(
            "test-service",
            "production",
            vec![
                ("error_rate", CmpType::Lt, 5.0),
                ("response_time", CmpType::Lt, 100.0),
            ],
            vec![("test_service.error_rate && test_service.response_time", 100)],
            &mock_url,
        );

        // Mock Graphite response: both metrics satisfy thresholds
        let _mock = server
            .mock("GET", "/render")
            .match_query(mockito::Matcher::AllOf(vec![mockito::Matcher::UrlEncoded(
                "format".into(),
                "json".into(),
            )]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"[
                    {"target":"test-service.error_rate","datapoints":[[2.0,1234567890]]},
                    {"target":"test-service.response_time","datapoints":[[50.0,1234567890]]}
                ]"#,
            )
            .create();

        let result = get_service_health(
            &state,
            "test-service",
            "production",
            "2024-01-01T00:00:00Z",
            "2024-01-01T01:00:00Z",
            100,
        )
        .await;

        assert!(
            result.is_ok(),
            "Two metrics AND expression (both true) should succeed: {:?}",
            result
        );
        let health_data = result.unwrap();
        assert_eq!(health_data.len(), 1, "Should have one datapoint");
        assert_eq!(
            health_data[0].1, 100,
            "AND expression should return weight 100 when both flags are true"
        );
    }

    // T028: Test two metrics AND expression (one false) returns false
    #[tokio::test]
    async fn test_two_metrics_and_one_false() {
        use mockito;

        let mut server = mockito::Server::new_async().await;
        let mock_url = server.url();

        // Setup: two metrics with AND expression
        let state = create_health_test_state(
            "test-service",
            "production",
            vec![
                ("error_rate", CmpType::Lt, 5.0),
                ("response_time", CmpType::Lt, 100.0),
            ],
            vec![("test_service.error_rate && test_service.response_time", 100)],
            &mock_url,
        );

        // Mock Graphite response: error_rate OK but response_time too high
        let _mock = server
            .mock("GET", "/render")
            .match_query(mockito::Matcher::AllOf(vec![mockito::Matcher::UrlEncoded(
                "format".into(),
                "json".into(),
            )]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"[
                    {"target":"test-service.error_rate","datapoints":[[2.0,1234567890]]},
                    {"target":"test-service.response_time","datapoints":[[150.0,1234567890]]}
                ]"#,
            )
            .create();

        let result = get_service_health(
            &state,
            "test-service",
            "production",
            "2024-01-01T00:00:00Z",
            "2024-01-01T01:00:00Z",
            100,
        )
        .await;

        assert!(
            result.is_ok(),
            "Two metrics AND expression (one false) should succeed"
        );
        let health_data = result.unwrap();
        assert_eq!(health_data.len(), 1, "Should have one datapoint");
        assert_eq!(
            health_data[0].1, 0,
            "AND expression should return weight 0 when one flag is false"
        );
    }

    // T029: Test weighted expressions return highest matching weight
    #[tokio::test]
    async fn test_weighted_expressions_highest_weight() {
        use mockito;

        let mut server = mockito::Server::new_async().await;
        let mock_url = server.url();

        // Setup: multiple expressions with different weights
        let state = create_health_test_state(
            "test-service",
            "production",
            vec![
                ("error_rate", CmpType::Lt, 5.0),
                ("response_time", CmpType::Lt, 100.0),
            ],
            vec![
                ("test_service.error_rate", 50),    // Weight 50 if only error_rate
                ("test_service.response_time", 30), // Weight 30 if only response_time
                ("test_service.error_rate && test_service.response_time", 100), // Weight 100 if both
            ],
            &mock_url,
        );

        // Mock Graphite response: both flags are true
        let _mock = server
            .mock("GET", "/render")
            .match_query(mockito::Matcher::AllOf(vec![mockito::Matcher::UrlEncoded(
                "format".into(),
                "json".into(),
            )]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"[
                    {"target":"test-service.error_rate","datapoints":[[2.0,1234567890]]},
                    {"target":"test-service.response_time","datapoints":[[50.0,1234567890]]}
                ]"#,
            )
            .create();

        let result = get_service_health(
            &state,
            "test-service",
            "production",
            "2024-01-01T00:00:00Z",
            "2024-01-01T01:00:00Z",
            100,
        )
        .await;

        assert!(result.is_ok(), "Weighted expressions should succeed");
        let health_data = result.unwrap();
        assert_eq!(health_data.len(), 1, "Should have one datapoint");
        assert_eq!(
            health_data[0].1, 100,
            "Should return highest matching weight (100)"
        );
    }

    // T030: Test all false expressions return weight 0
    #[tokio::test]
    async fn test_all_false_expressions_return_zero() {
        use mockito;

        let mut server = mockito::Server::new_async().await;
        let mock_url = server.url();

        // Setup: expressions that require flags to be true
        let state = create_health_test_state(
            "test-service",
            "production",
            vec![
                ("error_rate", CmpType::Lt, 5.0),
                ("response_time", CmpType::Lt, 100.0),
            ],
            vec![("test_service.error_rate && test_service.response_time", 100)],
            &mock_url,
        );

        // Mock Graphite response: both flags are false
        let _mock = server
            .mock("GET", "/render")
            .match_query(mockito::Matcher::AllOf(vec![mockito::Matcher::UrlEncoded(
                "format".into(),
                "json".into(),
            )]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"[
                    {"target":"test-service.error_rate","datapoints":[[10.0,1234567890]]},
                    {"target":"test-service.response_time","datapoints":[[200.0,1234567890]]}
                ]"#,
            )
            .create();

        let result = get_service_health(
            &state,
            "test-service",
            "production",
            "2024-01-01T00:00:00Z",
            "2024-01-01T01:00:00Z",
            100,
        )
        .await;

        assert!(result.is_ok(), "All false expressions should succeed");
        let health_data = result.unwrap();
        assert_eq!(health_data.len(), 1, "Should have one datapoint");
        assert_eq!(
            health_data[0].1, 0,
            "Should return weight 0 when all expressions are false"
        );
    }

    // T031: Test unknown service returns ServiceNotSupported error
    #[tokio::test]
    async fn test_unknown_service_error() {
        use mockito;

        let server = mockito::Server::new_async().await;
        let mock_url = server.url();

        let state = create_health_test_state(
            "test-service",
            "production",
            vec![("error_rate", CmpType::Lt, 5.0)],
            vec![("test_service.error_rate", 100)],
            &mock_url,
        );

        let result = get_service_health(
            &state,
            "unknown-service", // Request a service that doesn't exist
            "production",
            "2024-01-01T00:00:00Z",
            "2024-01-01T01:00:00Z",
            100,
        )
        .await;

        assert!(result.is_err(), "Unknown service should return error");
        match result.unwrap_err() {
            CloudMonError::ServiceNotSupported => {
                // Expected error type
            }
            other => panic!("Expected ServiceNotSupported, got {:?}", other),
        }
    }

    // T032: Test unknown environment returns EnvNotSupported error
    #[tokio::test]
    async fn test_unknown_environment_error() {
        use mockito;

        let server = mockito::Server::new_async().await;
        let mock_url = server.url();

        let state = create_health_test_state(
            "test-service",
            "production",
            vec![("error_rate", CmpType::Lt, 5.0)],
            vec![("test_service.error_rate", 100)],
            &mock_url,
        );

        let result = get_service_health(
            &state,
            "test-service",
            "unknown-env", // Request an environment that doesn't exist
            "2024-01-01T00:00:00Z",
            "2024-01-01T01:00:00Z",
            100,
        )
        .await;

        assert!(result.is_err(), "Unknown environment should return error");
        match result.unwrap_err() {
            CloudMonError::EnvNotSupported => {
                // Expected error type
            }
            other => panic!("Expected EnvNotSupported, got {:?}", other),
        }
    }

    // T033: Test multiple datapoints across time series
    #[tokio::test]
    async fn test_multiple_datapoints_time_series() {
        use mockito;

        let mut server = mockito::Server::new_async().await;
        let mock_url = server.url();

        let state = create_health_test_state(
            "test-service",
            "production",
            vec![("error_rate", CmpType::Lt, 5.0)],
            vec![("test_service.error_rate", 100)],
            &mock_url,
        );

        // Mock Graphite response: multiple datapoints over time
        let _mock = server
            .mock("GET", "/render")
            .match_query(mockito::Matcher::AllOf(vec![mockito::Matcher::UrlEncoded(
                "format".into(),
                "json".into(),
            )]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"[{
                    "target":"test-service.error_rate",
                    "datapoints":[
                        [2.0,1234567890],
                        [3.0,1234567900],
                        [1.5,1234567910],
                        [4.0,1234567920]
                    ]
                }]"#,
            )
            .create();

        let result = get_service_health(
            &state,
            "test-service",
            "production",
            "2024-01-01T00:00:00Z",
            "2024-01-01T01:00:00Z",
            100,
        )
        .await;

        assert!(result.is_ok(), "Multiple datapoints should succeed");
        let health_data = result.unwrap();
        assert_eq!(
            health_data.len(),
            4,
            "Should have four datapoints (one per timestamp)"
        );

        // All values are < 5.0, so all should have weight 100
        for (i, (_, weight)) in health_data.iter().enumerate() {
            assert_eq!(*weight, 100, "Datapoint {} should have weight 100", i);
        }
    }

    /// Additional coverage test: Test expression evaluation with invalid syntax
    /// This tests the error path in health score calculation
    #[tokio::test]
    async fn test_invalid_expression_syntax() {
        use mockito::Matcher;

        let mut server = mockito::Server::new();

        // Mock Graphite to return valid data
        let _mock = server
            .mock("GET", "/render")
            .match_query(Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!([
                    {
                        "target": "svc1.metric1",
                        "datapoints": [[15.0, 1609459200]]
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
          tmpl1:
            query: 'metric.$environment.$service.count'
            op: gt
            threshold: 10
        environments:
          - name: prod
        flag_metrics:
          - name: metric1
            service: svc1
            template:
              name: tmpl1
            environments:
              - name: prod
        health_metrics:
          svc1:
            service: svc1
            category: compute
            metrics:
              - svc1.metric1
            expressions:
              - expression: 'invalid syntax &&& broken'
                weight: 1
        ",
            server.url()
        );

        let config = crate::config::Config::from_config_str(&config_str);
        let mut state = AppState::new(config);
        state.process_config();

        // Call get_service_health with invalid expression
        let result = get_service_health(&state, "svc1", "prod", "now-1h", "now", 10).await;

        // Should return ExpressionError
        assert!(
            result.is_err(),
            "Should return error for invalid expression"
        );
    }
}
