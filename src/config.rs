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
//! status_dashboard:
//!   url: 'https://status-dashboard.example.com'
//!   secret: 'status-dashboard-jwt-secret'
//!   jwt_preferred_username: 'operator-sd'
//!   jwt_group: 'sd-operators'
//! ```
//!
//! # Environment variables
//! Configuration can be overridden with environment variables using the `MP_` prefix
//! and `__` as separator for nested values. Examples:
//! - `MP_STATUS_DASHBOARD__SECRET` - JWT signing secret
//! - `MP_STATUS_DASHBOARD__JWT_PREFERRED_USERNAME` - JWT preferred_username claim
//! - `MP_STATUS_DASHBOARD__JWT_GROUP` - JWT group claim (will be placed into groups array)
//!

use glob::glob;

use schemars::JsonSchema;
use serde::Deserialize;
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    path::Path,
};

use config::{ConfigError, Environment, File};

use crate::types::{BinaryMetricRawDef, EnvironmentDef, FlagMetricDef, ServiceHealthDef};

/// A Configuration structure
#[derive(Clone, Debug, Deserialize, JsonSchema)]
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
    /// Health metrics query configuration
    #[serde(default)]
    pub health_query: HealthQueryConfig,
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
#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct Datasource {
    /// TSDB url
    pub url: String,
    /// query timeout
    #[serde(default = "default_timeout")]
    pub timeout: u16,
}

/// Server binding configuration
#[derive(Clone, Debug, Deserialize, JsonSchema)]
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
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum DatasourceType {
    /// Graphite
    Graphite,
}

/// Status Dashboard configuration
#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct StatusDashboardConfig {
    /// Status dashboard URL
    pub url: String,
    /// JWT token signature secret
    pub secret: Option<String>,
    /// JWT token preferred_username claim
    pub jwt_preferred_username: Option<String>,
    /// JWT token group claim (will be placed into "groups" array in JWT payload)
    pub jwt_group: Option<String>,
}

/// Health metrics query configuration
#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct HealthQueryConfig {
    /// Query start time offset for health metrics (e.g., "-5min")
    #[serde(default = "default_query_from")]
    pub query_from: String,
    /// Query end time offset for health metrics (e.g., "-2min")
    #[serde(default = "default_query_to")]
    pub query_to: String,
}

impl Default for HealthQueryConfig {
    fn default() -> Self {
        Self {
            query_from: default_query_from(),
            query_to: default_query_to(),
        }
    }
}

fn default_query_from() -> String {
    "-5min".to_string()
}

fn default_query_to() -> String {
    "-2min".to_string()
}

#[cfg(test)]
mod test {
    use crate::config;

    use serial_test::serial;
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
    #[serial]
    fn test_merge_env() {
        // Create a file inside of `std::env::temp_dir()`.
        let mut config_file = Builder::new().suffix(".yaml").tempfile().unwrap();

        config_file.write_all(CONFIG_STR1.as_bytes()).unwrap();

        env::set_var("MP_STATUS_DASHBOARD__SECRET", "val");
        let _config = config::Config::new(config_file.path().to_str().unwrap()).unwrap();
        assert_eq!(_config.status_dashboard.unwrap().secret.unwrap(), "val");

        // Clean up to avoid affecting other tests
        env::remove_var("MP_STATUS_DASHBOARD__SECRET");
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

    /// T043: Test invalid YAML syntax returns parse error
    #[test]
    #[should_panic]
    fn test_invalid_yaml_syntax() {
        let invalid_yaml = "
        datasource:
          url: 'https://graphite.example.com'
        server:
          port: 3000
          invalid syntax here [[[
        ";
        // This should panic because YAML is invalid
        let _config = config::Config::from_config_str(invalid_yaml);
    }

    /// T044: Test missing required fields validation
    #[test]
    #[should_panic]
    fn test_missing_required_fields() {
        let missing_datasource = "
        server:
          port: 3000
        environments:
          - name: prod
        flag_metrics: []
        health_metrics: {}
        ";
        // This should panic because datasource is missing
        let _config = config::Config::from_config_str(missing_datasource);
    }

    /// T045: Test default values applied correctly
    #[test]
    fn test_default_values() {
        let minimal_config = "
        datasource:
          url: 'https://graphite.example.com'
        server: {}
        environments:
          - name: prod
        flag_metrics: []
        health_metrics: {}
        ";
        let config = config::Config::from_config_str(minimal_config);

        // Verify default server address
        assert_eq!("0.0.0.0", config.server.address);

        // Verify default server port
        assert_eq!(3000, config.server.port);

        // Verify default datasource timeout
        assert_eq!(10, config.datasource.timeout);
    }

    /// T046: Test get_socket_addr produces valid address
    #[test]
    fn test_get_socket_addr() {
        let config_str = "
        datasource:
          url: 'https://graphite.example.com'
        server:
          address: '127.0.0.1'
          port: 8080
        environments:
          - name: prod
        flag_metrics: []
        health_metrics: {}
        ";
        let config = config::Config::from_config_str(config_str);

        let socket_addr = config.get_socket_addr();
        assert_eq!("127.0.0.1:8080", socket_addr.to_string());
    }

    /// T047: Test config loading from multiple sources (file, conf.d, env vars)
    /// Note: This test is effectively covered by test_merge_parts and test_merge_env
    /// but we add an explicit comprehensive test
    #[test]
    #[serial]
    fn test_config_loading_from_multiple_sources() {
        // Clear any lingering environment variables from other tests
        // This is critical for test isolation when running all tests together
        let mp_vars: Vec<String> = env::vars()
            .filter(|(key, _)| key.starts_with("MP_"))
            .map(|(key, _)| key)
            .collect();
        for key in &mp_vars {
            env::remove_var(key);
        }

        // Create temporary directory structure
        let dir = Builder::new().tempdir().unwrap();
        let main_config_path = dir.path().join("config.yaml");
        let mut main_config = File::create(&main_config_path).unwrap();

        // Create conf.d directory
        let confd_path = dir.path().join("conf.d");
        create_dir(&confd_path).expect("Cannot create conf.d");

        // Write main config with all required fields
        let main_config_content = "
        datasource:
          url: 'https://graphite.example.com'
          timeout: 10
        server:
          port: 3000
          address: '0.0.0.0'
        metric_templates:
          tmpl1:
            query: 'base_query'
            op: lt
            threshold: 10
        environments:
          - name: prod
        health_metrics: {}
        ";
        main_config
            .write_all(main_config_content.as_bytes())
            .unwrap();

        // Write conf.d part
        let flags_config_content = "
        flag_metrics:
          - name: test-metric
            service: test-service
            template:
              name: tmpl1
            environments:
              - name: prod
        ";
        let mut flags_config = File::create(confd_path.join("flags.yaml")).unwrap();
        flags_config
            .write_all(flags_config_content.as_bytes())
            .unwrap();

        // Set environment variable for server port (override main config)
        env::set_var("MP_SERVER__PORT", "8080");

        // Load config from all sources
        let config = config::Config::new(main_config_path.to_str().unwrap()).unwrap();

        // Verify main config loaded
        assert_eq!("https://graphite.example.com", config.datasource.url);
        assert_eq!(10, config.datasource.timeout);

        // Verify conf.d part merged
        assert_eq!(1, config.flag_metrics.len());
        assert_eq!("test-metric", config.flag_metrics[0].name);

        // Verify environment variable merged (overrides main config)
        assert_eq!(8080, config.server.port);

        // Clean up environment variable
        env::remove_var("MP_SERVER__PORT");

        // Cleanup
        dir.close().unwrap();
    }

    /// Generate JSON schema for configuration.
    /// Run with: cargo test generate_config_schema -- --ignored
    /// This test is ignored by default so it only runs when explicitly requested.
    #[test]
    #[ignore]
    fn generate_config_schema() {
        use schemars::schema_for;
        use std::fs;
        use std::path::Path;

        let schema = schema_for!(config::Config);
        let schema_json =
            serde_json::to_string_pretty(&schema).expect("Failed to serialize schema");

        let schemas_dir = Path::new("doc/schemas");
        if !schemas_dir.exists() {
            fs::create_dir_all(schemas_dir).expect("Failed to create doc/schemas directory");
        }

        let schema_path = schemas_dir.join("config-schema.json");
        fs::write(&schema_path, &schema_json).expect("Failed to write config-schema.json");

        println!("Generated JSON schema at: {}", schema_path.display());
    }
}
