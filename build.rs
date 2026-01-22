// Build script to generate JSON schema for configuration

use schemars::{schema_for, JsonSchema};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

// Re-define the Config struct with JsonSchema derive
// This is a simplified version matching the actual Config struct

/// Configuration structure
#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct Config {
    /// TSDB datasource connection
    pub datasource: Datasource,
    /// HTTP server binding configuration
    pub server: ServerConf,
    /// Metric query templates
    pub metric_templates: Option<HashMap<String, BinaryMetricRawDef>>,
    /// Environment definitions
    pub environments: Vec<EnvironmentDef>,
    /// Flag metric definitions
    pub flag_metrics: Vec<FlagMetricDef>,
    /// Health metric definitions per service
    pub health_metrics: HashMap<String, ServiceHealthDef>,
    /// Status dashboard connection (optional)
    pub status_dashboard: Option<StatusDashboardConfig>,
}

/// TSDB Datasource connection
#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct Datasource {
    /// TSDB URL (e.g., http://localhost:8080)
    pub url: String,
    /// Query timeout in seconds (default: 10)
    #[serde(default = "default_timeout")]
    pub timeout: u16,
}

/// Server binding configuration
#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct ServerConf {
    /// IP address to bind to (default: 0.0.0.0)
    #[serde(default = "default_address")]
    pub address: String,
    /// Port to bind to (default: 3000)
    #[serde(default = "default_port")]
    pub port: u16,
}

/// Binary metric raw definition (template)
#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct BinaryMetricRawDef {
    /// TSDB query template with variable substitution
    pub query: String,
    /// Comparison operator (lt, gt, eq)
    pub op: String,
    /// Threshold value for comparison
    pub threshold: f64,
}

/// Environment definition
#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct EnvironmentDef {
    /// Environment name (e.g., production, staging)
    pub name: String,
}

/// Flag metric definition
#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct FlagMetricDef {
    /// Metric name
    pub name: String,
    /// Service name
    pub service: String,
    /// Template reference
    pub template: TemplateDef,
    /// Environment-specific overrides
    pub environments: Vec<EnvironmentOverride>,
}

/// Template reference
#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct TemplateDef {
    /// Template name (references metric_templates key)
    pub name: String,
}

/// Environment-specific override
#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct EnvironmentOverride {
    /// Environment name
    pub name: String,
    /// Overridden threshold (optional)
    pub threshold: Option<f64>,
}

/// Service health definition
#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct ServiceHealthDef {
    /// Service name
    pub service: String,
    /// Component display name
    pub component_name: Option<String>,
    /// Category (e.g., compute, network, storage)
    pub category: String,
    /// List of flag metric names to evaluate
    pub metrics: Vec<String>,
    /// Boolean expressions with weights
    pub expressions: Vec<ExpressionDef>,
}

/// Health expression definition
#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct ExpressionDef {
    /// Boolean expression (e.g., "api_slow || api_error_rate_high")
    pub expression: String,
    /// Weight: 0=healthy, 1=degraded, 2=outage
    pub weight: u8,
}

/// Status Dashboard configuration
#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct StatusDashboardConfig {
    /// Status dashboard URL
    pub url: String,
    /// JWT token signature secret (optional)
    pub secret: Option<String>,
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

fn main() {
    println!("cargo:rerun-if-changed=src/config.rs");
    println!("cargo:rerun-if-changed=src/types.rs");

    // Generate JSON schema
    let schema = schema_for!(Config);
    let schema_json = serde_json::to_string_pretty(&schema).expect("Failed to serialize schema");

    // Create doc/schemas directory if it doesn't exist
    let schemas_dir = Path::new("doc/schemas");
    if !schemas_dir.exists() {
        fs::create_dir_all(schemas_dir).expect("Failed to create doc/schemas directory");
    }

    // Write schema to file
    let schema_path = schemas_dir.join("config-schema.json");
    fs::write(&schema_path, schema_json).expect("Failed to write config-schema.json");

    println!("Generated JSON schema at: {:?}", schema_path);
}
