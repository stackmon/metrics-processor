//! Common methods
//!
use crate::types::{
    AppState, CloudMonError, CmpType, FlagMetric, ServiceHealthData, ServiceHealthPoint,
};
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
        // Determine which metrics were true at this timestamp
        let mut triggered: Vec<String> = Vec::new();
        for (mname, present) in ts_val.iter() {
            if *present {
                triggered.push(mname.clone());
            }
        }

        result.push(ServiceHealthPoint {
            ts: *ts,
            value: expression_res,
            triggered,
            metric_value: None,
        });
    }

    tracing::debug!("Summary data: {:?}, length={}", result, result.len());

    return Ok(result);
}
