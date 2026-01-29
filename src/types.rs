//! CloudMon metrics processor types
//!
//! Internal types definitions
use crate::config::Config;
use new_string_template::template::Template;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::time::Duration;

use reqwest::ClientBuilder;

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CmpType {
    Lt,
    Gt,
    Eq,
}

#[derive(Clone, Debug, Deserialize)]
pub struct BinaryMetricRawDef {
    pub query: String,
    pub op: CmpType,
    pub threshold: f32,
}

impl Default for BinaryMetricRawDef {
    fn default() -> Self {
        BinaryMetricRawDef {
            query: String::new(),
            op: CmpType::Lt,
            threshold: 0.0,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct BinaryMetricDef {
    pub query: Option<String>,
    pub op: Option<CmpType>,
    pub threshold: Option<f32>,
    pub template: Option<MetricTemplateRef>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MetricTemplateRef {
    pub name: String,
    pub vars: Option<HashMap<String, String>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EnvironmentDef {
    pub name: String,
    pub attributes: Option<HashMap<String, String>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FlagMetric {
    pub query: String,
    pub op: CmpType,
    pub threshold: f32,
}

impl Default for FlagMetric {
    fn default() -> Self {
        FlagMetric {
            query: String::new(),
            op: CmpType::Lt,
            threshold: 0.0,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct MetricExpressionDef {
    pub expression: String,
    pub weight: i32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FlagMetricDef {
    pub name: String,
    pub service: String,
    pub template: Option<MetricTemplateRef>,
    pub environments: Vec<MetricEnvironmentDef>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MetricEnvironmentDef {
    pub name: String,
    pub threshold: Option<f32>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ServiceHealthDef {
    pub service: String,
    pub component_name: Option<String>,
    pub category: String,
    pub metrics: Vec<String>,
    pub expressions: Vec<MetricExpressionDef>,
}

pub type MetricPoints = BTreeMap<u32, bool>;
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MetricData {
    pub target: String,
    #[serde(rename(serialize = "datapoints"))]
    pub points: MetricPoints,
}
/// List of the service health values (ts, data)
pub type ServiceHealthData = Vec<(u32, u8)>;

pub enum CloudMonError {
    ServiceNotSupported,
    EnvNotSupported,
    ExpressionError,
    GraphiteError,
}
impl std::error::Error for CloudMonError {}

impl fmt::Display for CloudMonError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CloudMonError::ServiceNotSupported => write!(f, "Requested service not supported"),
            CloudMonError::EnvNotSupported => write!(f, "Environment for service not supported"),
            CloudMonError::ExpressionError => write!(f, "Internal Expression evaluation error"),
            CloudMonError::GraphiteError => write!(f, "Graphite error"),
        }
    }
}
impl fmt::Debug for CloudMonError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CloudMonError::ServiceNotSupported => write!(f, "Requested service not supported"),
            CloudMonError::EnvNotSupported => write!(f, "Environment for service not supported"),
            CloudMonError::ExpressionError => write!(f, "Internal Expression evaluation error"),
            CloudMonError::GraphiteError => write!(f, "Graphite error"),
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub metric_templates: HashMap<String, BinaryMetricRawDef>,
    pub req_client: reqwest::Client,
    pub flag_metrics: HashMap<String, HashMap<String, FlagMetric>>,
    pub health_metrics: HashMap<String, ServiceHealthDef>,
    pub environments: Vec<EnvironmentDef>,
    pub services: HashSet<String>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let timeout = Duration::from_secs(config.datasource.timeout as u64);

        Self {
            config,
            metric_templates: HashMap::new(),
            flag_metrics: HashMap::new(),
            req_client: ClientBuilder::new().timeout(timeout).build().unwrap(),
            health_metrics: HashMap::new(),
            environments: Vec::new(),
            services: HashSet::new(),
        }
    }

    pub fn process_config(&mut self) {
        // We substitute $var syntax
        let custom_regex = Regex::new(r"(?mi)\$([^\.]+)").unwrap();
        if let Some(templates) = &self.config.metric_templates {
            self.metric_templates.clone_from(templates);
        }
        for metric_def in self.config.flag_metrics.iter() {
            if let Some(tmpl_ref) = &metric_def.template {
                let metric_name = format!("{}.{}", metric_def.service, metric_def.name);
                self.flag_metrics
                    .insert(metric_name.clone(), HashMap::new());
                let tmpl = self.metric_templates.get(&tmpl_ref.name).unwrap();
                let tmpl_query = Template::new(tmpl.query.clone()).with_regex(&custom_regex);
                for env in metric_def.environments.iter() {
                    let threshold = env.threshold.unwrap_or(tmpl.threshold);
                    let raw = FlagMetric {
                        query: String::new(), // Will be set below
                        op: tmpl.op.clone(),
                        threshold,
                    };
                    let vars: HashMap<&str, &str> = HashMap::from([
                        ("service", metric_def.service.as_str()),
                        ("environment", env.name.as_str()),
                    ]);
                    let mut raw = raw;
                    raw.query = tmpl_query.render(&vars).unwrap();
                    if let Some(x) = self.flag_metrics.get_mut(&metric_name) {
                        x.insert(env.name.clone(), raw.clone());
                    } else {
                        tracing::error!("Metric processing failed");
                    }
                }
            };
            self.services.insert(metric_def.service.clone());
        }

        for (metric_name, health_def) in self.config.health_metrics.iter() {
            tracing::debug!("{:?}", health_def);
            let mut int_metric = ServiceHealthDef {
                service: health_def.service.clone(),
                component_name: health_def.component_name.clone(),
                category: health_def.category.clone(),
                metrics: health_def.metrics.clone(),
                expressions: Vec::new(),
            };
            // If we have "-" in the metric name evalexpr will treat it as minus operation. In order to
            // avoid that replace "-" with "_" in the expression. Values will be renamed during
            // evaluation.
            let mut replacements: HashMap<String, String> = HashMap::new();
            for metric in health_def.metrics.iter() {
                if metric.contains("-") {
                    replacements.insert(metric.into(), metric.replace("-", "_"));
                }
            }
            for expr in health_def.expressions.iter() {
                let mut expression = expr.expression.clone();
                for (k, v) in replacements.iter() {
                    expression = expression.replace(k, v);
                }
                int_metric.expressions.push(MetricExpressionDef {
                    expression,
                    weight: expr.weight,
                });
            }
            self.health_metrics.insert(metric_name.into(), int_metric);
        }
        self.environments = self.config.environments.clone();
    }
}

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn test_state() {
        let f = "
        datasource:
          url: 'https:/a.b'
        server:
          port: 3005
        metric_templates:
          tmpl1:
            query: dummy1($environment.$service.count)
            op: lt
            threshold: 90
          tmpl2:
            query: dummy2($environment.$service.count)
            op: gt
            threshold: 80
        environments:
          - name: env1
        flag_metrics:
          - name: metric-1
            service: srvA
            template:
              name: tmpl1
            environments:
              - name: env1
              - name: env2
                threshold: 1
          - name: metric-2
            service: srvA
            template:
              name: tmpl2
            environments:
              - name: env1
              - name: env2
        health_metrics:
          srvA:
            service: srvA
            category: compute
            metrics:
              - srvA.metric-1
              - srvA.metric-2
            expressions:
              - expression: 'srvA.metric-1 || srvA.metric-2'
                weight: 1
";
        let config = config::Config::from_config_str(f);
        let mut state = types::AppState::new(config);
        state.process_config();

        // Validate flag_metrics conversion
        let m1 = state
            .flag_metrics
            .get("srvA.metric-1")
            .unwrap()
            .get("env1")
            .unwrap();
        assert_eq!("dummy1(env1.srvA.count)", m1.query);
        assert_eq!(types::CmpType::Lt, m1.op);
        assert_eq!(90.0, m1.threshold);
        let m2 = state
            .flag_metrics
            .get("srvA.metric-1")
            .unwrap()
            .get("env2")
            .unwrap();
        assert_eq!("dummy1(env2.srvA.count)", m2.query);
        assert_eq!(types::CmpType::Lt, m2.op);
        assert_eq!(1.0, m2.threshold);
        tracing::debug!("{:?}", state.health_metrics);
        let s1 = state.health_metrics.get("srvA").unwrap();
        tracing::debug!("{:?}", s1);
        // Verify we got "-" replaced with "_"
        assert_eq!(
            s1.expressions[0].expression,
            "srvA.metric_1 || srvA.metric_2"
        );
    }

    /// T037: Test template variable substitution ($environment, $service)
    #[test]
    fn test_template_variable_substitution() {
        let config_str = "
        datasource:
          url: 'https://graphite.example.com'
        server:
          port: 3000
        metric_templates:
          cpu_usage:
            query: 'metrics.$environment.$service.cpu.usage'
            op: gt
            threshold: 80
        environments:
          - name: production
        flag_metrics:
          - name: cpu-alert
            service: webapp
            template:
              name: cpu_usage
            environments:
              - name: production
        health_metrics: {}
        ";
        let config = config::Config::from_config_str(config_str);
        let mut state = types::AppState::new(config);
        state.process_config();

        // Verify $environment and $service variables are substituted correctly
        let metric = state
            .flag_metrics
            .get("webapp.cpu-alert")
            .unwrap()
            .get("production")
            .unwrap();
        assert_eq!("metrics.production.webapp.cpu.usage", metric.query);
        assert_eq!(types::CmpType::Gt, metric.op);
        assert_eq!(80.0, metric.threshold);
    }

    /// T038: Test multiple environments expansion creates correct mappings
    #[test]
    fn test_multiple_environments_expansion() {
        let config_str = "
        datasource:
          url: 'https://graphite.example.com'
        server:
          port: 3000
        metric_templates:
          error_rate:
            query: '$environment.$service.errors'
            op: gt
            threshold: 5
        environments:
          - name: dev
          - name: staging
          - name: production
        flag_metrics:
          - name: error-count
            service: api
            template:
              name: error_rate
            environments:
              - name: dev
              - name: staging
              - name: production
        health_metrics: {}
        ";
        let config = config::Config::from_config_str(config_str);
        let mut state = types::AppState::new(config);
        state.process_config();

        // Verify metric exists for all three environments
        let metric_key = "api.error-count";
        assert!(state.flag_metrics.contains_key(metric_key));

        let dev_metric = state
            .flag_metrics
            .get(metric_key)
            .unwrap()
            .get("dev")
            .unwrap();
        assert_eq!("dev.api.errors", dev_metric.query);

        let staging_metric = state
            .flag_metrics
            .get(metric_key)
            .unwrap()
            .get("staging")
            .unwrap();
        assert_eq!("staging.api.errors", staging_metric.query);

        let prod_metric = state
            .flag_metrics
            .get(metric_key)
            .unwrap()
            .get("production")
            .unwrap();
        assert_eq!("production.api.errors", prod_metric.query);
    }

    /// T039: Test per-environment threshold override
    #[test]
    fn test_per_environment_threshold_override() {
        let config_str = "
        datasource:
          url: 'https://graphite.example.com'
        server:
          port: 3000
        metric_templates:
          latency:
            query: '$environment.$service.latency.p95'
            op: gt
            threshold: 1000
        environments:
          - name: dev
          - name: production
        flag_metrics:
          - name: latency-alert
            service: api
            template:
              name: latency
            environments:
              - name: dev
                threshold: 5000
              - name: production
                threshold: 500
        health_metrics: {}
        ";
        let config = config::Config::from_config_str(config_str);
        let mut state = types::AppState::new(config);
        state.process_config();

        // Verify dev has override threshold of 5000
        let dev_metric = state
            .flag_metrics
            .get("api.latency-alert")
            .unwrap()
            .get("dev")
            .unwrap();
        assert_eq!(5000.0, dev_metric.threshold);

        // Verify production has override threshold of 500
        let prod_metric = state
            .flag_metrics
            .get("api.latency-alert")
            .unwrap()
            .get("production")
            .unwrap();
        assert_eq!(500.0, prod_metric.threshold);
    }

    /// T040: Test dash-to-underscore conversion in expressions
    #[test]
    fn test_dash_to_underscore_conversion() {
        let config_str = "
        datasource:
          url: 'https://graphite.example.com'
        server:
          port: 3000
        metric_templates: {}
        environments:
          - name: production
        flag_metrics: []
        health_metrics:
          api-service:
            service: api-service
            category: compute
            metrics:
              - api-service.cpu-usage
              - api-service.memory-usage
              - api-service.disk-io
            expressions:
              - expression: 'api-service.cpu-usage || api-service.memory-usage && api-service.disk-io'
                weight: 2
        ";
        let config = config::Config::from_config_str(config_str);
        let mut state = types::AppState::new(config);
        state.process_config();

        // Verify dashes in metric names are replaced with underscores
        let health_metric = state.health_metrics.get("api-service").unwrap();
        assert_eq!(
            "api_service.cpu_usage || api_service.memory_usage && api_service.disk_io",
            health_metric.expressions[0].expression
        );
    }

    /// T041: Test service set population from config
    #[test]
    fn test_service_set_population() {
        let config_str = "
        datasource:
          url: 'https://graphite.example.com'
        server:
          port: 3000
        metric_templates:
          tmpl:
            query: 'metric'
            op: lt
            threshold: 1
        environments:
          - name: prod
        flag_metrics:
          - name: metric1
            service: webapp
            template:
              name: tmpl
            environments:
              - name: prod
          - name: metric2
            service: database
            template:
              name: tmpl
            environments:
              - name: prod
          - name: metric3
            service: cache
            template:
              name: tmpl
            environments:
              - name: prod
        health_metrics: {}
        ";
        let config = config::Config::from_config_str(config_str);
        let mut state = types::AppState::new(config);
        state.process_config();

        // Verify all services are collected in the services set
        assert_eq!(3, state.services.len());
        assert!(state.services.contains("webapp"));
        assert!(state.services.contains("database"));
        assert!(state.services.contains("cache"));
    }

    /// T042: Test health metrics expression copying
    #[test]
    fn test_health_metrics_expression_copying() {
        let config_str = "
        datasource:
          url: 'https://graphite.example.com'
        server:
          port: 3000
        metric_templates: {}
        environments:
          - name: prod
        flag_metrics: []
        health_metrics:
          compute-service:
            service: compute
            category: infrastructure
            metrics:
              - cpu
              - memory
            expressions:
              - expression: 'cpu || memory'
                weight: 1
              - expression: 'cpu && memory'
                weight: 3
        ";
        let config = config::Config::from_config_str(config_str);
        let mut state = types::AppState::new(config);
        state.process_config();

        // Verify expressions are copied correctly
        let health_metric = state.health_metrics.get("compute-service").unwrap();
        assert_eq!(2, health_metric.expressions.len());
        assert_eq!("cpu || memory", health_metric.expressions[0].expression);
        assert_eq!(1, health_metric.expressions[0].weight);
        assert_eq!("cpu && memory", health_metric.expressions[1].expression);
        assert_eq!(3, health_metric.expressions[1].weight);
    }
}

/// Additional coverage test: Test CloudMonError Display implementation
#[test]
fn test_error_display() {
    assert_eq!(
        format!("{}", CloudMonError::ServiceNotSupported),
        "Requested service not supported"
    );
    assert_eq!(
        format!("{}", CloudMonError::EnvNotSupported),
        "Environment for service not supported"
    );
    assert_eq!(
        format!("{}", CloudMonError::ExpressionError),
        "Internal Expression evaluation error"
    );
    assert_eq!(
        format!("{}", CloudMonError::GraphiteError),
        "Graphite error"
    );
}

/// Additional coverage test: Test CloudMonError Debug implementation
#[test]
fn test_error_debug() {
    assert_eq!(
        format!("{:?}", CloudMonError::ServiceNotSupported),
        "Requested service not supported"
    );
    assert_eq!(
        format!("{:?}", CloudMonError::EnvNotSupported),
        "Environment for service not supported"
    );
    assert_eq!(
        format!("{:?}", CloudMonError::ExpressionError),
        "Internal Expression evaluation error"
    );
    assert_eq!(
        format!("{:?}", CloudMonError::GraphiteError),
        "Graphite error"
    );
}

/// Additional coverage test: Test BinaryMetricRawDef Default
#[test]
fn test_binary_metric_raw_def_default() {
    let default = BinaryMetricRawDef::default();
    assert_eq!(default.query, String::new());
    assert_eq!(default.op, CmpType::Lt);
    assert_eq!(default.threshold, 0.0);
}

/// Additional coverage test: Test FlagMetric Default implementation
#[test]
fn test_flag_metric_default() {
    let default = FlagMetric::default();
    assert_eq!(default.query, String::new());
    assert!(matches!(default.op, CmpType::Lt));
    assert_eq!(default.threshold, 0.0);
}

