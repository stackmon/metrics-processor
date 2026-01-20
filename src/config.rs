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

use glob::glob;

use serde::Deserialize;
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    path::Path,
};

use config::{ConfigError, Environment, File};

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
    /// Returns a configuration object from a yaml config file path with merged values from
    /// environment variables prefixed with "MP". When setting values in the environment variables
    /// use "__" for sublements separator.
    pub fn new(config_file: &str) -> Result<Self, ConfigError> {
        let path = Path::new(config_file)
            .canonicalize()
            .expect("Can not resolve path to the config.yaml");
        let mut s = config::Config::builder()
            // Start off by merging in the requested configuration file
            .add_source(File::with_name(path.to_str().unwrap()));

        // Read and merge conf.d config parts
        let configs_glob = format!(
            "{}/conf.d/*.yaml",
            path.parent()
                .expect("Need parent to config.yaml")
                .to_str()
                .unwrap()
        );
        tracing::trace!("Analyzing {:?} as conf.d parts", configs_glob);
        for entry in glob(configs_glob.as_str()).unwrap() {
            tracing::debug!("Add {:?} config part file", entry);
            if let Ok(path) = entry {
                s = s.add_source(File::with_name(path.to_str().unwrap()));
            }
        }

        // merge environment variables (subelements separated by "__")
        // MP_STATUS_DASHBOARD__SECRET goes to status_dashboard.secret
        s = s.add_source(
            Environment::with_prefix("MP")
                .prefix_separator("_")
                .separator("__"),
        );

        s.build()?.try_deserialize()
    }

    /// Returns a configuration object from a string representing configuration file
    #[allow(dead_code)]
    pub fn from_config_str(data: &str) -> Self {
        let s = config::Config::builder()
            .add_source(File::from_str(data, config::FileFormat::Yaml))
            .build()
            .unwrap();
        s.try_deserialize().unwrap()
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
    /// Polling interval in seconds (default: 60)
    #[serde(default = "default_poll_interval")]
    pub poll_interval: u64,
    /// Number of retry attempts for fetching components (default: 3)
    #[serde(default = "default_retry_count")]
    pub retry_count: u32,
    /// Delay between retries in seconds (default: 60)
    #[serde(default = "default_retry_delay")]
    pub retry_delay: u64,
    /// HTTP request timeout in seconds (default: 10)
    #[serde(default = "default_sdb_timeout")]
    pub timeout: u64,
    /// Query time range start (default: "-5min")
    #[serde(default = "default_query_from")]
    pub query_from: String,
    /// Query time range end (default: "-2min")
    #[serde(default = "default_query_to")]
    pub query_to: String,
    /// Incident title template (default: "System incident from monitoring system")
    #[serde(default = "default_incident_title")]
    pub incident_title: String,
    /// Incident description template (default: "System-wide incident affecting multiple components. Created automatically.")
    #[serde(default = "default_incident_description")]
    pub incident_description: String,
}

fn default_poll_interval() -> u64 {
    60
}

fn default_retry_count() -> u32 {
    3
}

fn default_retry_delay() -> u64 {
    60
}

fn default_sdb_timeout() -> u64 {
    10
}

fn default_query_from() -> String {
    "-5min".to_string()
}

fn default_query_to() -> String {
    "-2min".to_string()
}

fn default_incident_title() -> String {
    "System incident from monitoring system".to_string()
}

fn default_incident_description() -> String {
    "System-wide incident affecting multiple components. Created automatically.".to_string()
}

#[cfg(test)]
mod test {
    use crate::config;

    use std::env;
    use std::fs::{create_dir, File};
    use std::io::Write;
    use tempfile::Builder;

    const CONFIG_STR1: &str = "
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
    status_dashboard:
      url: abc
    ";
    const CONFIG_PART_STR: &str = "
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
    status_dashboard:
      url: abc
    ";

    const CONFIG_FLAGS: &str = "
    flag_metrics:
      - name: a
        service: b
        template:
          name: tmpl1
        environments:
          - name: env1
            threshold: 2
    ";

    /// Test general config parsing
    #[test]
    fn test_config_file() {
        // Create a file inside of `std::env::temp_dir()`.
        let mut config_file = Builder::new().suffix(".yaml").tempfile().unwrap();

        config_file.write_all(CONFIG_STR1.as_bytes()).unwrap();

        let _config = config::Config::new(config_file.path().to_str().unwrap()).unwrap();
        assert_eq!(_config.flag_metrics.len(), 1);
        for flag in _config.flag_metrics.iter() {
            assert_eq!("a", &flag.name);
            assert_eq!("b", &flag.service);
        }
    }

    /// Test merging config with env vars
    #[test]
    fn test_merge_env() {
        // Create a file inside of `std::env::temp_dir()`.
        let mut config_file = Builder::new().suffix(".yaml").tempfile().unwrap();

        config_file.write_all(CONFIG_STR1.as_bytes()).unwrap();

        env::set_var("MP_STATUS_DASHBOARD__SECRET", "val");
        let _config = config::Config::new(config_file.path().to_str().unwrap()).unwrap();
        assert_eq!(_config.status_dashboard.unwrap().secret.unwrap(), "val");
    }

    /// Test merging of the config with conf.d elements
    #[test]
    fn test_merge_parts() {
        // Create a file inside of `std::env::temp_dir()`.
        let dir = Builder::new().tempdir().unwrap();
        let main_config_file_path = dir.path().join("config.yaml");
        let mut main_config_file = File::create(main_config_file_path.clone()).unwrap();
        let confd_file_path = dir.path().join("conf.d");
        create_dir(&confd_file_path).expect("Cannot create tmp/conf.d");
        let mut flags = File::create(&confd_file_path.as_path().join("flags.yaml")).unwrap();
        println!("flags are {:?}", flags);

        main_config_file
            .write_all(CONFIG_PART_STR.as_bytes())
            .unwrap();

        flags.write_all(CONFIG_FLAGS.as_bytes()).unwrap();

        let _config = config::Config::new(main_config_file_path.clone().to_str().unwrap()).unwrap();
        for flag in _config.flag_metrics.iter() {
            assert_eq!("a", &flag.name);
            assert_eq!("b", &flag.service);
        }

        dir.close().unwrap();
    }
}
