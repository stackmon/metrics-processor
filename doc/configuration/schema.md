# Configuration Schema Reference

The complete JSON schema for configuration validation is available at:

**[doc/schemas/config-schema.json](../schemas/config-schema.json)**

This schema can be used with YAML language servers and validators to provide autocompletion and validation in your editor.

## Using the Schema

### VS Code

Add to your `.vscode/settings.json`:

```json
{
  "yaml.schemas": {
    "./doc/schemas/config-schema.json": ["config.yaml", "conf.d/*.yaml"]
  }
}
```

### Command-Line Validation

```bash
# Using ajv-cli
npm install -g ajv-cli
ajv validate -s doc/schemas/config-schema.json -d config.yaml

# Using yajsv
go install github.com/neilpa/yajsv@latest
yajsv -s doc/schemas/config-schema.json config.yaml
```

## Top-Level Properties

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `datasource` | [Datasource](#datasource) | Yes | TSDB connection settings |
| `server` | [ServerConf](#serverconf) | Yes | HTTP server binding |
| `environments` | [EnvironmentDef[]](#environmentdef) | Yes | Environment definitions |
| `flag_metrics` | [FlagMetricDef[]](#flagmetricdef) | Yes | Flag metric definitions |
| `health_metrics` | Map<string, [ServiceHealthDef](#servicehealthdef)> | Yes | Health metrics by service |
| `metric_templates` | Map<string, [BinaryMetricRawDef](#binarymetricrawdef)> | No | Query templates |
| `status_dashboard` | [StatusDashboardConfig](#statusdashboardconfig) | No | Status dashboard settings |

## Type Definitions

### Datasource

TSDB connection configuration.

| Property | Type | Required | Default | Description |
|----------|------|----------|---------|-------------|
| `url` | string | Yes | -       | TSDB URL (e.g., `http://graphite:8080`) |
| `timeout` | integer | No | `2`     | Query timeout in seconds |

```yaml
datasource:
  url: "http://graphite.example.com:8080"
  timeout: 15
```

### ServerConf

HTTP API server binding configuration.

| Property | Type | Required | Default | Description |
|----------|------|----------|---------|-------------|
| `address` | string | No | `"0.0.0.0"` | IP address to bind |
| `port` | integer | No | `3000` | Port to bind |

```yaml
server:
  address: "127.0.0.1"
  port: 3005
```

### EnvironmentDef

Environment definition.

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `name` | string | Yes | Environment identifier |
| `attributes` | Map<string, string> | No | Additional metadata |

```yaml
environments:
  - name: "production"
    attributes:
      region: "eu-west-1"
```

### BinaryMetricRawDef

Metric template definition for reusable queries.

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `query` | string | Yes | TSDB query with variable substitution |
| `op` | string | Yes | Comparison operator: `lt`, `gt`, `eq` |
| `threshold` | number | Yes | Default threshold value |

```yaml
metric_templates:
  api_slow:
    query: "stats.timers.api.$environment.$service.mean"
    op: "gt"
    threshold: 500
```

### FlagMetricDef

Flag metric definition linking templates to services/environments.

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `name` | string | Yes | Metric identifier |
| `service` | string | Yes | Service name |
| `template` | [TemplateRef](#templateref) | Yes | Template reference |
| `environments` | [EnvironmentOverride[]](#environmentoverride) | Yes | Environment configurations |

```yaml
flag_metrics:
  - name: "slow_response"
    service: "api-gateway"
    template:
      name: "api_slow"
    environments:
      - name: "production"
      - name: "staging"
        threshold: 1000
```

### TemplateRef

Reference to a metric template.

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `name` | string | Yes | Template name (key in `metric_templates`) |
| `vars` | Map<string, string> | No | Additional template variables |

### EnvironmentOverride

Environment-specific threshold override.

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `name` | string | Yes | Environment name |
| `threshold` | number | No | Override threshold (uses template default if omitted) |

### ServiceHealthDef

Service health aggregation definition.

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `service` | string | Yes | Service identifier |
| `component_name` | string | No | Display name for UI |
| `category` | string | Yes | Category (e.g., `compute`, `network`) |
| `metrics` | string[] | Yes | List of flag metric names |
| `expressions` | [ExpressionDef[]](#expressiondef) | Yes | Health expressions |

```yaml
health_metrics:
  api-gateway:
    service: "api-gateway"
    component_name: "API Gateway"
    category: "network"
    metrics:
      - "api-gateway.slow_response"
      - "api-gateway.error_rate_high"
    expressions:
      - expression: "api-gateway.slow_response"
        weight: 1
      - expression: "api-gateway.error_rate_high"
        weight: 2
```

### ExpressionDef

Boolean expression with severity weight.

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `expression` | string | Yes | Boolean expression using flag metrics |
| `weight` | integer | Yes | Severity: `0`=healthy, `1`=degraded, `2`=outage |

### StatusDashboardConfig

Optional status dashboard integration.

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `url` | string | Yes | Status dashboard URL |
| `secret` | string | No | JWT signing secret |

```yaml
status_dashboard:
  url: "https://status.example.com"
  secret: "your-jwt-secret"  # Use MP_STATUS_DASHBOARD__SECRET env var instead
```

## Comparison Operators

The `op` field in templates accepts these values:

| Operator | Description | Example |
|----------|-------------|---------|
| `lt` | Less than | Value < threshold (e.g., success rate < 90%) |
| `gt` | Greater than | Value > threshold (e.g., latency > 500ms) |
| `eq` | Equal to | Value == threshold (e.g., error rate == 100%) |

## Weight Values

Health expression weights map to semaphore states:

| Weight | State | Color | Description |
|--------|-------|-------|-------------|
| `0` | Healthy | 🟢 Green | Service operating normally |
| `1` | Degraded | 🟡 Yellow | Service experiencing issues |
| `2` | Outage | 🔴 Red | Service unavailable |

## Related Documentation

- [Overview](overview.md) - Configuration structure introduction
- [Datasource](datasource.md) - TSDB connection details
- [Metric Templates](metric-templates.md) - Template configuration
- [Flag Metrics](flag-metrics.md) - Flag metric configuration
- [Health Metrics](health-metrics.md) - Health expression configuration
