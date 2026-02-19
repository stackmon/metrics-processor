# Configuration Overview

The metrics-processor uses a hierarchical YAML-based configuration system that supports modular configuration through multiple sources.

## Configuration Structure

The configuration is organized into these top-level sections:

| Section | Required | Description |
|---------|----------|-------------|
| `datasource` | Yes | TSDB connection settings |
| `server` | Yes | HTTP API binding configuration |
| `environments` | Yes | Environment definitions |
| `flag_metrics` | Yes | Binary metric definitions |
| `health_metrics` | Yes | Service health aggregations |
| `metric_templates` | No | Reusable query templates |
| `status_dashboard` | No | Status dashboard integration |

## Configuration Loading Hierarchy

Configuration is loaded and merged in the following order (later sources override earlier ones):

### 1. Main Configuration File

The primary configuration file specified via the `--config` argument:

```bash
cloudmon-metrics-convertor --config /etc/metrics-processor/config.yaml
```

### 2. Configuration Parts (conf.d/)

Additional configuration files in the `conf.d/` subdirectory relative to the main config file are automatically merged. Files are loaded in alphabetical order.

```
/etc/metrics-processor/
├── config.yaml           # Main configuration
└── conf.d/
    ├── 01-templates.yaml # Loaded first
    ├── 02-services.yaml  # Loaded second
    └── 99-overrides.yaml # Loaded last
```

This pattern allows you to:
- Separate concerns (templates, services, environments)
- Share common configuration across deployments
- Apply environment-specific overrides

### 3. Environment Variables (MP_ prefix)

Environment variables prefixed with `MP_` override configuration values. Use double underscore (`__`) to access nested properties:

```bash
# Override datasource.url
export MP_DATASOURCE__URL="http://graphite.example.com:8080"

# Override server.port
export MP_SERVER__PORT=3005

# Set status_dashboard.jwt_secret (sensitive values)
export MP_STATUS_DASHBOARD__JWT_SECRET="your-jwt-secret"
```

**Best Practice**: Use environment variables for sensitive values like secrets and for deployment-specific overrides in containerized environments.

## Minimal Configuration Example

A minimal working configuration requires:

```yaml
datasource:
  url: "http://localhost:8080"

server:
  port: 3000

environments:
  - name: "production"

flag_metrics: []

health_metrics: {}
```

## Complete Configuration Example

```yaml
# TSDB connection
datasource:
  url: "http://graphite.example.com:8080"
  timeout: 15

# HTTP API server
server:
  address: "0.0.0.0"
  port: 3000

# Reusable query templates
metric_templates:
  api_latency:
    query: "stats.timers.api.$environment.$service.mean"
    op: "gt"
    threshold: 500

# Environment definitions
environments:
  - name: "production"
  - name: "staging"

# Status dashboard (optional)
status_dashboard:
  url: "https://status.example.com"

# Flag metric definitions
flag_metrics:
  - name: "slow_response"
    service: "api-gateway"
    template:
      name: "api_latency"
    environments:
      - name: "production"
      - name: "staging"
        threshold: 1000  # More lenient in staging

# Health metric aggregations
health_metrics:
  api-gateway:
    service: "api-gateway"
    component_name: "API Gateway"
    category: "network"
    metrics:
      - "api-gateway.slow_response"
    expressions:
      - expression: "api-gateway.slow_response"
        weight: 1
```

## Related Documentation

- [Schema Reference](schema.md) - Complete field reference
- [Datasource](datasource.md) - TSDB connection configuration
- [Metric Templates](metric-templates.md) - Query templates and variables
- [Flag Metrics](flag-metrics.md) - Binary metric configuration
- [Health Metrics](health-metrics.md) - Health expression configuration
- [Environments](environments.md) - Environment configuration
- [Examples](examples.md) - Working configuration samples
