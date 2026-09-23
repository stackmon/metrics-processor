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

use anyhow::Context;
use schemars::JsonSchema;
use serde::Deserialize;
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    path::Path,
};

use crate::oidc::{load_service_account, normalize_issuer};
use crate::types::{BinaryMetricRawDef, EnvironmentDef, FlagMetricDef, ServiceHealthDef};
use config::{ConfigError, Environment, File};
use zitadel::credentials::ServiceAccount;

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
    /// Zitadel OIDC issuer URL
    pub oidc_issuer: Option<String>,
    /// Path to the Zitadel machine user key file.
    ///
    /// The file is downloaded from the Zitadel Console for a machine user (service user) and has
    /// `type: serviceaccount`; it is read once at startup and any other key type fails startup.
    pub oidc_key_file: Option<String>,
    /// OIDC scopes of the token request, sent as one space-joined `scope` parameter.
    ///
    /// `MP_STATUS_DASHBOARD__OIDC_SCOPES` has to contain both
    /// `urn:zitadel:iam:org:project:role:sd_reporters`, so that the roles are reported in the
    /// `groups` claim, and `urn:zitadel:iam:org:project:id:<projectId>:aud`, which makes `aud` the
    /// project id the Status Dashboard verifies against `SD_OIDC_CLIENT_ID`. `<projectId>` is the
    /// Zitadel project shared by the Status Dashboard and the machine user. Without the audience
    /// scope Zitadel puts the client id into `aud`, which the backend rejects.
    ///
    /// There is no default, because the audience scope carries the project id of the deployment.
    pub oidc_scopes: Option<Vec<String>>,
}

pub const OIDC_ISSUER_ENV_KEY: &str = "MP_STATUS_DASHBOARD__OIDC_ISSUER";
pub const OIDC_KEY_FILE_ENV_KEY: &str = "MP_STATUS_DASHBOARD__OIDC_KEY_FILE";
pub const OIDC_SCOPES_ENV_KEY: &str = "MP_STATUS_DASHBOARD__OIDC_SCOPES";

const OIDC_ROLE_SCOPE_PREFIX: &str = "urn:zitadel:iam:org:project:role:";
const OIDC_AUDIENCE_SCOPE_PREFIX: &str = "urn:zitadel:iam:org:project:id:";
const OIDC_AUDIENCE_SCOPE_SUFFIX: &str = ":aud";
const OIDC_SCOPES_EXAMPLE: &str = concat!(
    "  oidc_scopes:\n",
    "    - \"urn:zitadel:iam:org:project:role:sd_reporters\"\n",
    "    - \"urn:zitadel:iam:org:project:id:<projectId>:aud\"",
);

pub struct OidcIdentity {
    pub issuer: String,
    pub service_account: ServiceAccount,
    pub scopes: Vec<String>,
}

impl std::fmt::Debug for OidcIdentity {
    /// The crate renders the loaded key material in its own `Debug` output, so it is not forwarded.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OidcIdentity")
            .field("issuer", &self.issuer)
            .field("service_account", &"<redacted>")
            .field("scopes", &self.scopes)
            .finish()
    }
}

impl StatusDashboardConfig {
    /// Reporting is fail-closed: an incomplete identity must stop startup, so every problem names
    /// the offending configuration key.
    pub fn oidc_identity(&self) -> anyhow::Result<OidcIdentity> {
        let issuer = self
            .oidc_issuer
            .as_deref()
            .filter(|issuer| !issuer.trim().is_empty());
        let key_file_path = self
            .oidc_key_file
            .as_deref()
            .filter(|key_file| !key_file.trim().is_empty());
        let scopes = self.oidc_scopes.as_deref().unwrap_or_default();

        if let (Some(issuer), Some(key_file_path)) = (issuer, key_file_path) {
            validate_oidc_scopes(scopes)?;

            let service_account = load_service_account(key_file_path).with_context(|| {
                format!(
                    "{} does not point to a usable Zitadel machine user key file",
                    OIDC_KEY_FILE_ENV_KEY
                )
            })?;

            return Ok(OidcIdentity {
                issuer: normalize_issuer(issuer),
                service_account,
                scopes: scopes.to_vec(),
            });
        }

        let mut missing = Vec::new();
        if issuer.is_none() {
            missing.push(OIDC_ISSUER_ENV_KEY);
        }
        if key_file_path.is_none() {
            missing.push(OIDC_KEY_FILE_ENV_KEY);
        }
        if scopes.is_empty() {
            missing.push(OIDC_SCOPES_ENV_KEY);
        }

        anyhow::bail!(
            "Status Dashboard OIDC service identity is incomplete, missing: {}",
            missing.join(", ")
        )
    }
}

/// The Status Dashboard takes the token audience and the reporter role from scopes that Zitadel
/// only applies when the project audience scope is requested, so a scope list with either scope
/// class missing would fail with 401 at report time instead of at startup.
fn validate_oidc_scopes(scopes: &[String]) -> anyhow::Result<()> {
    let mut missing = Vec::new();
    if !scopes
        .iter()
        .any(|scope| scope.starts_with(OIDC_ROLE_SCOPE_PREFIX))
    {
        missing.push(format!(
            "a scope starting with \"{OIDC_ROLE_SCOPE_PREFIX}\""
        ));
    }
    if !scopes.iter().any(|scope| {
        scope.starts_with(OIDC_AUDIENCE_SCOPE_PREFIX) && scope.ends_with(OIDC_AUDIENCE_SCOPE_SUFFIX)
    }) {
        missing.push(format!(
            "a scope starting with \"{OIDC_AUDIENCE_SCOPE_PREFIX}\" and ending with \"{OIDC_AUDIENCE_SCOPE_SUFFIX}\""
        ));
    }

    if missing.is_empty() {
        return Ok(());
    }

    anyhow::bail!(
        "{} (status_dashboard.oidc_scopes) must contain both a project role scope and the project \
         audience scope of the Zitadel project shared with the Status Dashboard, missing {}. \
         Zitadel only reports the project roles claim when the audience scope is requested, so a \
         token requested without it is rejected. Set for example:\n{}",
        OIDC_SCOPES_ENV_KEY,
        missing.join(" and "),
        OIDC_SCOPES_EXAMPLE
    )
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

        env::set_var("MP_STATUS_DASHBOARD__OIDC_KEY_FILE", "val");
        let _config = config::Config::new(config_file.path().to_str().unwrap()).unwrap();
        assert_eq!(
            _config.status_dashboard.unwrap().oidc_key_file.unwrap(),
            "val"
        );

        // Clean up to avoid affecting other tests
        env::remove_var("MP_STATUS_DASHBOARD__OIDC_KEY_FILE");
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

    const ROLE_SCOPE: &str = "urn:zitadel:iam:org:project:role:sd_reporters";
    const AUDIENCE_SCOPE: &str = "urn:zitadel:iam:org:project:id:392066917738875090:aud";

    fn status_dashboard_section() -> super::StatusDashboardConfig {
        serde_yaml::from_str(&format!(
            "url: https://status.example.com\noidc_scopes:\n  - \"{ROLE_SCOPE}\"\n  - \"{AUDIENCE_SCOPE}\"\n"
        ))
        .unwrap()
    }

    fn write_key_file(dir: &tempfile::TempDir) -> String {
        use crate::oidc::test_keys::service_account_key_file_json;

        let path = dir.path().join("service-account.json");
        std::fs::write(&path, service_account_key_file_json()).unwrap();
        path.to_str().unwrap().to_string()
    }

    fn scopes_config(scopes: Option<Vec<String>>, key_file: &str) -> super::StatusDashboardConfig {
        super::StatusDashboardConfig {
            oidc_issuer: Some("https://zitadel.example.com".to_string()),
            oidc_key_file: Some(key_file.to_string()),
            oidc_scopes: scopes,
            ..status_dashboard_section()
        }
    }

    #[test]
    fn test_oidc_identity_reports_missing_configuration_keys() {
        let both_missing = super::StatusDashboardConfig {
            oidc_scopes: None,
            ..status_dashboard_section()
        };
        let message = format!("{:#}", both_missing.oidc_identity().unwrap_err());
        for key in [
            super::OIDC_ISSUER_ENV_KEY,
            super::OIDC_KEY_FILE_ENV_KEY,
            super::OIDC_SCOPES_ENV_KEY,
        ] {
            assert!(message.contains(key), "{} not reported: {}", key, message);
        }

        let key_file_missing = super::StatusDashboardConfig {
            oidc_issuer: Some("https://zitadel.example.com".to_string()),
            ..status_dashboard_section()
        };
        let message = format!("{:#}", key_file_missing.oidc_identity().unwrap_err());
        assert!(
            message.contains(super::OIDC_KEY_FILE_ENV_KEY),
            "unexpected error: {}",
            message
        );
        assert!(
            !message.contains(super::OIDC_ISSUER_ENV_KEY),
            "unexpected error: {}",
            message
        );

        let issuer_missing = super::StatusDashboardConfig {
            oidc_key_file: Some("service-account.json".to_string()),
            ..status_dashboard_section()
        };
        let message = format!("{:#}", issuer_missing.oidc_identity().unwrap_err());
        assert!(
            message.contains(super::OIDC_ISSUER_ENV_KEY),
            "unexpected error: {}",
            message
        );
        assert!(
            !message.contains(super::OIDC_KEY_FILE_ENV_KEY),
            "unexpected error: {}",
            message
        );
    }

    #[test]
    fn test_oidc_identity_rejects_scopes_without_the_project_audience() {
        let dir = Builder::new().tempdir().unwrap();
        let key_file = write_key_file(&dir);
        let project_id_without_audience = "urn:zitadel:iam:org:project:id:392066917738875090";

        let cases = [
            vec![ROLE_SCOPE.to_string()],
            vec![
                ROLE_SCOPE.to_string(),
                project_id_without_audience.to_string(),
            ],
        ];

        for scopes in cases {
            let config = scopes_config(Some(scopes.clone()), &key_file);
            let message = format!("{:#}", config.oidc_identity().unwrap_err());

            assert!(
                message.contains(super::OIDC_SCOPES_ENV_KEY),
                "{:?} not reported: {}",
                scopes,
                message
            );
            assert!(
                message.contains(&format!(
                    "a scope starting with \"{}\" and ending with \"{}\"",
                    super::OIDC_AUDIENCE_SCOPE_PREFIX,
                    super::OIDC_AUDIENCE_SCOPE_SUFFIX
                )),
                "the missing audience scope is not named: {}",
                message
            );
            assert!(
                !message.contains(&format!(
                    "a scope starting with \"{}\"",
                    super::OIDC_ROLE_SCOPE_PREFIX
                )),
                "a configured role scope is reported as missing: {}",
                message
            );
            assert!(
                message.contains("urn:zitadel:iam:org:project:id:<projectId>:aud"),
                "the audience scope example is missing: {}",
                message
            );
        }
    }

    #[test]
    fn test_oidc_identity_rejects_scopes_without_a_project_role() {
        let dir = Builder::new().tempdir().unwrap();
        let key_file = write_key_file(&dir);

        let cases = [
            vec![AUDIENCE_SCOPE.to_string()],
            vec!["openid".to_string(), AUDIENCE_SCOPE.to_string()],
        ];

        for scopes in cases {
            let config = scopes_config(Some(scopes.clone()), &key_file);
            let message = format!("{:#}", config.oidc_identity().unwrap_err());

            assert!(
                message.contains(super::OIDC_SCOPES_ENV_KEY),
                "{:?} not reported: {}",
                scopes,
                message
            );
            assert!(
                message.contains(&format!(
                    "a scope starting with \"{}\"",
                    super::OIDC_ROLE_SCOPE_PREFIX
                )),
                "the missing role scope is not named: {}",
                message
            );
            assert!(
                !message.contains(&format!(
                    "a scope starting with \"{}\" and ending with \"{}\"",
                    super::OIDC_AUDIENCE_SCOPE_PREFIX,
                    super::OIDC_AUDIENCE_SCOPE_SUFFIX
                )),
                "a configured audience scope is reported as missing: {}",
                message
            );
            assert!(
                message.contains("urn:zitadel:iam:org:project:role:sd_reporters"),
                "the role scope example is missing: {}",
                message
            );
        }
    }

    #[test]
    fn test_oidc_identity_rejects_scopes_that_are_not_configured() {
        let dir = Builder::new().tempdir().unwrap();
        let key_file = write_key_file(&dir);
        let cases: [Option<Vec<String>>; 2] = [None, Some(Vec::new())];

        for scopes in cases {
            let config = scopes_config(scopes, &key_file);
            let message = format!("{:#}", config.oidc_identity().unwrap_err());

            assert!(
                message.contains(super::OIDC_SCOPES_ENV_KEY),
                "{} not reported: {}",
                super::OIDC_SCOPES_ENV_KEY,
                message
            );
            for prefix in [
                super::OIDC_ROLE_SCOPE_PREFIX,
                super::OIDC_AUDIENCE_SCOPE_PREFIX,
            ] {
                assert!(
                    message.contains(prefix),
                    "{} not reported: {}",
                    prefix,
                    message
                );
            }
        }
    }

    #[test]
    fn test_oidc_identity_accepts_configured_role_and_audience_scopes() {
        let dir = Builder::new().tempdir().unwrap();
        let key_file = write_key_file(&dir);
        let scopes = vec![
            "openid".to_string(),
            ROLE_SCOPE.to_string(),
            AUDIENCE_SCOPE.to_string(),
        ];

        let config = scopes_config(Some(scopes.clone()), &key_file);
        let identity = config.oidc_identity().unwrap();

        assert_eq!(identity.scopes, scopes);
        assert_eq!(identity.issuer, "https://zitadel.example.com");
    }

    #[test]
    fn test_oidc_identity_reports_the_failing_key_file() {
        use crate::oidc::test_keys::{
            application_key_file_json, key_file_json_with_type, KEY_ID, USER_ID,
        };

        let dir = Builder::new().tempdir().unwrap();

        let invalid_json = dir.path().join("invalid.json");
        std::fs::write(&invalid_json, "{ not json }").unwrap();

        let unknown_type = dir.path().join("unknown-type.json");
        std::fs::write(&unknown_type, key_file_json_with_type("widget")).unwrap();

        let application = dir.path().join("application.json");
        std::fs::write(&application, application_key_file_json()).unwrap();

        let empty_user = dir.path().join("empty-user.json");
        std::fs::write(
            &empty_user,
            serde_json::json!({
                "type": "serviceaccount",
                "keyId": KEY_ID,
                "key": "-----BEGIN",
                "userId": "",
            })
            .to_string(),
        )
        .unwrap();

        let empty_key = dir.path().join("empty-key.json");
        std::fs::write(
            &empty_key,
            serde_json::json!({
                "type": "serviceaccount",
                "keyId": KEY_ID,
                "key": "",
                "userId": USER_ID,
            })
            .to_string(),
        )
        .unwrap();

        let cases = [
            dir.path().join("does-not-exist.json"),
            invalid_json,
            unknown_type,
            application,
            empty_user,
            empty_key,
        ];
        let mut messages = Vec::new();

        for path in cases {
            let config = super::StatusDashboardConfig {
                oidc_issuer: Some("https://zitadel.example.com".to_string()),
                oidc_key_file: Some(path.to_str().unwrap().to_string()),
                ..status_dashboard_section()
            };

            let message = format!("{:#}", config.oidc_identity().unwrap_err());
            assert!(
                message.contains(super::OIDC_KEY_FILE_ENV_KEY),
                "{} not reported: {}",
                super::OIDC_KEY_FILE_ENV_KEY,
                message
            );
            messages.push(message);
        }

        assert!(
            messages[0].contains("cannot be read"),
            "unexpected error: {}",
            messages[0]
        );
        assert!(
            messages[1].contains("not valid JSON"),
            "unexpected error: {}",
            messages[1]
        );
        for index in [2, 3] {
            assert!(
                messages[index].contains("serviceaccount"),
                "unexpected error: {}",
                messages[index]
            );
        }
        assert!(
            messages[4].contains("empty userId"),
            "unexpected error: {}",
            messages[4]
        );
        assert!(
            messages[5].contains("empty key"),
            "unexpected error: {}",
            messages[5]
        );

        for (index, message) in messages.iter().enumerate() {
            assert!(
                !messages[..index].contains(message),
                "error messages are not distinguishable: {}",
                message
            );
        }
    }

    #[test]
    fn test_oidc_identity_loads_the_key_file_and_normalizes_the_issuer() {
        let dir = Builder::new().tempdir().unwrap();
        let key_file = write_key_file(&dir);

        let config = super::StatusDashboardConfig {
            oidc_issuer: Some("https://zitadel.example.com/".to_string()),
            oidc_key_file: Some(key_file.clone()),
            ..status_dashboard_section()
        };

        let identity = config.oidc_identity().unwrap();

        assert_eq!(identity.issuer, "https://zitadel.example.com");
        assert_eq!(
            identity.token_url(),
            "https://zitadel.example.com/oauth/v2/token"
        );
        let rendered = format!("{:?}", identity);
        assert!(
            rendered.contains("https://zitadel.example.com"),
            "unexpected debug output: {}",
            rendered
        );
        assert!(
            !rendered.contains("PRIVATE KEY") && !rendered.contains("key_id"),
            "key material leaked into debug output: {}",
            rendered
        );
        assert_eq!(
            identity.scopes,
            vec![ROLE_SCOPE.to_string(), AUDIENCE_SCOPE.to_string()]
        );
    }
}
