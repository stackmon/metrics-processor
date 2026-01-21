// Test configuration fixtures for unit and integration tests
//
// Provides YAML configuration strings for various test scenarios

use serde_json::json;

/// Minimal valid configuration with one service
pub fn minimal_config() -> &'static str {
    r#"
listen_address: "127.0.0.1:8080"
graphite_url: "http://localhost:9090"

services:
  test-service:
    environments:
      - production
    
    metrics:
      - name: error_rate
        graphite_query: "stats.test-service.$environment.errors"
        operator: Lt
        threshold: 5.0
    
    health_metrics:
      - expression: "error_rate"
        weight: 100
"#
}

/// Configuration with multiple services and environments
pub fn multi_service_config() -> &'static str {
    r#"
listen_address: "0.0.0.0:8080"
graphite_url: "http://graphite.example.com"

services:
  api:
    environments:
      - production
      - staging
    
    metrics:
      - name: response_time
        graphite_query: "stats.api.$environment.response_time"
        operator: Lt
        threshold: 500.0
      
      - name: error_count
        graphite_query: "stats.api.$environment.errors"
        operator: Lt
        threshold: 10.0
    
    health_metrics:
      - expression: "response_time AND error_count"
        weight: 100
      - expression: "response_time"
        weight: 50
  
  database:
    environments:
      - production
    
    metrics:
      - name: connection_pool
        graphite_query: "stats.db.connections"
        operator: Gt
        threshold: 5.0
    
    health_metrics:
      - expression: "connection_pool"
        weight: 100
"#
}

/// Configuration with template variables
pub fn template_variable_config() -> &'static str {
    r#"
listen_address: "127.0.0.1:8080"
graphite_url: "http://localhost:9090"

services:
  web-service:
    environments:
      - dev
      - prod
    
    metrics:
      - name: cpu_usage
        graphite_query: "servers.$service.$environment.cpu"
        operator: Lt
        threshold: 80.0
      
      - name: memory_usage
        graphite_query: "servers.$service.$environment.memory"
        operator: Lt
        threshold: 90.0
    
    health_metrics:
      - expression: "cpu_usage AND memory_usage"
        weight: 100
"#
}

/// Configuration with per-environment threshold overrides
pub fn threshold_override_config() -> &'static str {
    r#"
listen_address: "127.0.0.1:8080"
graphite_url: "http://localhost:9090"

services:
  api:
    environments:
      - production
      - staging
    
    metrics:
      - name: latency
        graphite_query: "stats.api.$environment.latency"
        operator: Lt
        threshold: 1000.0
        thresholds:
          production: 500.0
          staging: 1000.0
    
    health_metrics:
      - expression: "latency"
        weight: 100
"#
}

/// Configuration with dash-to-underscore conversion in expressions
pub fn dash_conversion_config() -> &'static str {
    r#"
listen_address: "127.0.0.1:8080"
graphite_url: "http://localhost:9090"

services:
  my-service:
    environments:
      - production
    
    metrics:
      - name: error-rate
        graphite_query: "stats.my-service.errors"
        operator: Lt
        threshold: 5.0
      
      - name: request-count
        graphite_query: "stats.my-service.requests"
        operator: Gt
        threshold: 100.0
    
    health_metrics:
      - expression: "error-rate AND request-count"
        weight: 100
"#
}

/// Invalid YAML syntax for error testing
pub fn invalid_yaml_config() -> &'static str {
    r#"
listen_address: "127.0.0.1:8080"
graphite_url: "http://localhost:9090"
invalid syntax here: [unclosed bracket
services:
  test-service:
"#
}

/// Configuration with missing required fields
pub fn missing_fields_config() -> &'static str {
    r#"
listen_address: "127.0.0.1:8080"
# Missing graphite_url

services:
  test-service:
    # Missing environments
    metrics:
      - name: test_metric
        # Missing graphite_query
        operator: Lt
        threshold: 5.0
"#
}

/// Configuration with default values
pub fn default_values_config() -> &'static str {
    r#"
# No listen_address specified - should default to 127.0.0.1:8080
graphite_url: "http://localhost:9090"

services:
  test-service:
    environments:
      - production
    
    metrics:
      - name: metric1
        graphite_query: "stats.test.metric1"
        operator: Lt
        threshold: 10.0
    
    health_metrics:
      - expression: "metric1"
        weight: 100
"#
}

/// Configuration for testing socket address parsing
pub fn custom_port_config() -> &'static str {
    r#"
listen_address: "0.0.0.0:9999"
graphite_url: "http://localhost:9090"

services:
  test-service:
    environments:
      - production
    
    metrics:
      - name: test_metric
        graphite_query: "stats.test"
        operator: Lt
        threshold: 5.0
    
    health_metrics:
      - expression: "test_metric"
        weight: 100
"#
}

/// Configuration with multiple operator types
pub fn all_operators_config() -> &'static str {
    r#"
listen_address: "127.0.0.1:8080"
graphite_url: "http://localhost:9090"

services:
  test-service:
    environments:
      - production
    
    metrics:
      - name: errors
        graphite_query: "stats.errors"
        operator: Lt
        threshold: 5.0
      
      - name: requests
        graphite_query: "stats.requests"
        operator: Gt
        threshold: 100.0
      
      - name: exact_value
        graphite_query: "stats.exact"
        operator: Eq
        threshold: 42.0
    
    health_metrics:
      - expression: "errors AND requests AND exact_value"
        weight: 100
"#
}

/// Configuration with complex boolean expressions
pub fn complex_expressions_config() -> &'static str {
    r#"
listen_address: "127.0.0.1:8080"
graphite_url: "http://localhost:9090"

services:
  api:
    environments:
      - production
    
    metrics:
      - name: cpu
        graphite_query: "servers.cpu"
        operator: Lt
        threshold: 80.0
      
      - name: memory
        graphite_query: "servers.memory"
        operator: Lt
        threshold: 90.0
      
      - name: disk
        graphite_query: "servers.disk"
        operator: Lt
        threshold: 85.0
      
      - name: requests
        graphite_query: "api.requests"
        operator: Gt
        threshold: 10.0
    
    health_metrics:
      - expression: "cpu AND memory AND disk"
        weight: 100
      - expression: "cpu OR memory"
        weight: 80
      - expression: "requests"
        weight: 50
"#
}
