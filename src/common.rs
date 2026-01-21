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
    return match *value {
        Some(x) => match metric.op {
            CmpType::Lt => x < metric.threshold,
            CmpType::Gt => x > metric.threshold,
            CmpType::Eq => x == metric.threshold,
        },
        None => false,
    };
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
    let metric_names: Vec<String> = Vec::from(hm_config.metrics.clone());

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
        &state.config.datasource.url.as_str(),
        &graphite_targets,
        DateTime::parse_from_rfc3339(from).ok(),
        Some(from.to_string()),
        DateTime::parse_from_rfc3339(to).ok(),
        Some(to.to_string()),
        max_data_points,
    )
    .await
    .unwrap();

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
                    if let Some(_) = val {
                        metrics_map.entry(*ts).or_insert(HashMap::new()).insert(
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
                .set_value(metric.replace("-", "_").into(), Value::from(xval))
                .unwrap();
        }
        let mut expression_res: u8 = 0;
        // loop over all expressions
        for expr in hm_config.expressions.iter() {
            // if expression weight is lower then what we have already - skip
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

    return Ok(result);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CmpType, FlagMetric};

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
        assert_eq!(result, false, "Lt operator: 10.0 < 10.0 should return false");
        
        // Test above
        let value = Some(15.0);
        let result = get_metric_flag_state(&value, &metric);
        assert_eq!(result, false, "Lt operator: 15.0 < 10.0 should return false");
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
        assert_eq!(result, false, "Gt operator: 10.0 > 10.0 should return false");
        
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
        assert_eq!(result, false, "Eq operator: 5.0 == 10.0 should return false");
        
        // Test above
        let value = Some(15.0);
        let result = get_metric_flag_state(&value, &metric);
        assert_eq!(result, false, "Eq operator: 15.0 == 10.0 should return false");
    }

    // T016: Test None value always returns false for all operators
    #[test]
    fn test_none_value_returns_false() {
        let value = None;
        
        // Test with Lt operator
        let metric = create_test_metric(CmpType::Lt, 10.0);
        let result = get_metric_flag_state(&value, &metric);
        assert_eq!(result, false, "Lt operator with None value should return false");
        
        // Test with Gt operator
        let metric = create_test_metric(CmpType::Gt, 10.0);
        let result = get_metric_flag_state(&value, &metric);
        assert_eq!(result, false, "Gt operator with None value should return false");
        
        // Test with Eq operator
        let metric = create_test_metric(CmpType::Eq, 10.0);
        let result = get_metric_flag_state(&value, &metric);
        assert_eq!(result, false, "Eq operator with None value should return false");
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
}

