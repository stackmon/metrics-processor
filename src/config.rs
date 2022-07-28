//! CloudMon Metrics Processor configuration
//!
//! # Example configuration
//! ```yaml
//! ---
//! datasource:
//!   url: 'https:/a.b'
//! server:
//!   port: 3005
//! templates:
//!   tmpl1:
//!     query: dummy_query
//!     op: lt
//!     threshold: 1
//! environments:
//!   - name: env1
//! flag_metrics:
//!   - name: a
//!     service: b
//!     template:
//!       name: tmpl1
//!     environments:
//!       - name: env1
//!         threshold: 2
//! health_metrics:
//!   test:
//!     service: a
//!     category: compute
//!     metrics:
//!       - a
//!       - b-c
//!       - d-e
//!     expressions:
//!       - expression: 'a + b-c && d-e'
//!         weight: 1
//! ```

use serde::Deserialize;
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
};

use crate::types::{BinaryMetricRawDef, EnvironmentDef, FlagMetricDef, ServiceHealthDef};

/// A Configuration structure
#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    /// Datasource link
    pub datasource: Datasource,
    /// Server API binding
    pub server: ServerConf,
    /// Metric templates
    pub metric_templates: Option<HashMap<String, BinaryMetricRawDef>>,
    /// Environments
    pub environments: Vec<EnvironmentDef>,
    /// Flag metrics
    pub flag_metrics: Vec<FlagMetricDef>,
    /// Health metrics
    pub health_metrics: HashMap<String, ServiceHealthDef>,
    /// Status Dashboard connection
    pub status_dashboard: Option<StatusDashboardConfig>,
}

impl Config {
    /// Returns a configuration object from a yaml config file path.
    pub fn from_config_file(config_file: &str) -> Self {
        let f = std::fs::File::open(config_file).expect("Could not open file.");
        let config: Config = serde_yaml::from_reader(f).expect("Could not read values.");
        return config;
    }

    /// Returns a configuration object from a string representing configuration file
    #[allow(dead_code)]
    pub fn from_config_str(data: &str) -> Self {
        let config: Config = serde_yaml::from_str(data).expect("Could not read values.");
        return config;
    }

    /// Returns socket address to use for binding
    pub fn get_socket_addr(&self) -> SocketAddr {
        SocketAddr::from((
            self.server.address.as_str().parse::<IpAddr>().unwrap(),
            self.server.port,
        ))
    }
}

/// TSDB Datasource connection
#[derive(Clone, Debug, Deserialize)]
pub struct Datasource {
    /// TSDB url
    pub url: String,
    /// query timeout
    #[serde(default = "default_timeout")]
    pub timeout: u16,
}

/// Server binding configuration
#[derive(Clone, Debug, Deserialize)]
pub struct ServerConf {
    /// IP address to bind to
    #[serde(default = "default_address")]
    pub address: String,
    /// Port to bind to
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_address() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    3000
}

fn default_timeout() -> u16 {
    10
}

/// TSDB supported types enum
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatasourceType {
    /// Graphite
    Graphite,
}

/// Status Dashboard configuration
#[derive(Clone, Debug, Deserialize)]
pub struct StatusDashboardConfig {
    /// Status dashboard URL
    pub url: String,
    /// JWT token signature secret
    pub secret: Option<String>,
}

#[cfg(test)]
mod test {
    use crate::config;

    #[test]
    fn test_config_file() {
        let config_str1 = "
        datasource:
          url: 'https:/a.b'
        server:
          port: 3005
        templates:
          tmpl1:
            query: dummy_query
            op: lt
            threshold: 1
        environments:
          - name: env1
        flag_metrics:
          - name: a
            service: b
            template:
              name: tmpl1
            environments:
              - name: env1
                threshold: 2
        health_metrics:
          test:
            service: a
            category: compute
            metrics:
              - a
              - b-c
              - d-e
            expressions:
              - expression: 'a + b-c && d-e'
                weight: 1
        ";
        let _config = config::Config::from_config_str(config_str1);
        assert_eq!(_config.flag_metrics.len(), 1);
        for flag in _config.flag_metrics.iter() {
            assert_eq!("a", &flag.name);
            assert_eq!("b", &flag.service);
        }
    }
}
