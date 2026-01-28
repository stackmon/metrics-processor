// Test configuration fixtures for unit and integration tests
//
// Provides YAML configuration strings for various test scenarios
// These configs are compatible with the actual Config struct

/// Empty health metrics configuration for error testing
pub fn empty_health_config(graphite_url: &str) -> String {
    format!(r#"
datasource:
  url: '{}'
server:
  port: 3000
environments:
  - name: prod
flag_metrics: []
health_metrics:
  known-service:
    service: known
    category: compute
    metrics: []
    expressions: []
"#, graphite_url)
}

/// Configuration with known service for error testing
pub fn error_test_config(graphite_url: &str) -> String {
    format!(r#"
datasource:
  url: '{}'
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
health_metrics:
  webapp:
    service: webapp
    category: compute
    metrics:
      - webapp.metric1
    expressions:
      - expression: 'webapp.metric1'
        weight: 1
"#, graphite_url)
}
