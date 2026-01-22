# Configuration Examples

This page provides complete, working configuration examples for common use cases.

## Basic Single-Service Setup

A minimal configuration for monitoring a single service in one environment.

```yaml
---
# Basic single-service configuration
# Monitors API latency for one service in production

datasource:
  url: "http://graphite.example.com:8080"
  timeout: 10

server:
  address: "0.0.0.0"
  port: 3000

metric_templates:
  api_latency:
    query: "stats.timers.api.$environment.$service.mean"
    op: "gt"
    threshold: 500

environments:
  - name: "production"

flag_metrics:
  - name: "api_slow"
    service: "user-service"
    template:
      name: "api_latency"
    environments:
      - name: "production"

health_metrics:
  user-service:
    service: "user-service"
    component_name: "User Service"
    category: "compute"
    metrics:
      - "user-service.api_slow"
    expressions:
      - expression: "user-service.api_slow"
        weight: 1
```

## Multi-Service Monitoring

Configuration for monitoring multiple services with shared templates.

```yaml
---
# Multi-service monitoring configuration
# Monitors API latency and availability for compute, network, and storage services

datasource:
  url: "http://graphite.example.com:8080"
  timeout: 15

server:
  address: "0.0.0.0"
  port: 3000

metric_templates:
  api_latency:
    query: "stats.timers.api.$environment.$service.mean"
    op: "gt"
    threshold: 500

  api_success_rate:
    query: "asPercent(sumSeries(stats.counters.api.$environment.$service.2xx), sumSeries(stats.counters.api.$environment.$service.total))"
    op: "lt"
    threshold: 95

  api_down:
    query: "asPercent(sumSeries(stats.counters.api.$environment.$service.5xx), sumSeries(stats.counters.api.$environment.$service.total))"
    op: "eq"
    threshold: 100

environments:
  - name: "production"

# Compute Services
flag_metrics:
  - name: "api_slow"
    service: "nova"
    template:
      name: "api_latency"
    environments:
      - name: "production"

  - name: "api_success_rate_low"
    service: "nova"
    template:
      name: "api_success_rate"
    environments:
      - name: "production"

  - name: "api_down"
    service: "nova"
    template:
      name: "api_down"
    environments:
      - name: "production"

  # Network Services
  - name: "api_slow"
    service: "neutron"
    template:
      name: "api_latency"
    environments:
      - name: "production"

  - name: "api_success_rate_low"
    service: "neutron"
    template:
      name: "api_success_rate"
    environments:
      - name: "production"

  - name: "api_down"
    service: "neutron"
    template:
      name: "api_down"
    environments:
      - name: "production"

  # Storage Services
  - name: "api_slow"
    service: "cinder"
    template:
      name: "api_latency"
    environments:
      - name: "production"

  - name: "api_success_rate_low"
    service: "cinder"
    template:
      name: "api_success_rate"
    environments:
      - name: "production"

  - name: "api_down"
    service: "cinder"
    template:
      name: "api_down"
    environments:
      - name: "production"

health_metrics:
  nova:
    service: "nova"
    component_name: "Compute Service"
    category: "compute"
    metrics:
      - "nova.api_slow"
      - "nova.api_success_rate_low"
      - "nova.api_down"
    expressions:
      - expression: "nova.api_slow || nova.api_success_rate_low"
        weight: 1
      - expression: "nova.api_down"
        weight: 2

  neutron:
    service: "neutron"
    component_name: "Network Service"
    category: "network"
    metrics:
      - "neutron.api_slow"
      - "neutron.api_success_rate_low"
      - "neutron.api_down"
    expressions:
      - expression: "neutron.api_slow || neutron.api_success_rate_low"
        weight: 1
      - expression: "neutron.api_down"
        weight: 2

  cinder:
    service: "cinder"
    component_name: "Block Storage"
    category: "storage"
    metrics:
      - "cinder.api_slow"
      - "cinder.api_success_rate_low"
      - "cinder.api_down"
    expressions:
      - expression: "cinder.api_slow || cinder.api_success_rate_low"
        weight: 1
      - expression: "cinder.api_down"
        weight: 2
```

## Environment-Specific Thresholds

Configuration demonstrating different thresholds per environment.

```yaml
---
# Environment-specific thresholds configuration
# Uses stricter thresholds in production, more lenient in staging/development

datasource:
  url: "http://graphite.example.com:8080"
  timeout: 15

server:
  address: "0.0.0.0"
  port: 3000

metric_templates:
  api_latency:
    query: "stats.timers.api.$environment.$service.mean"
    op: "gt"
    threshold: 300  # Strict default for production

  api_success_rate:
    query: "asPercent(sumSeries(stats.counters.api.$environment.$service.2xx), sumSeries(stats.counters.api.$environment.$service.total))"
    op: "lt"
    threshold: 99.5  # Strict SLA for production

  api_error_rate:
    query: "asPercent(sumSeries(stats.counters.api.$environment.$service.5xx), sumSeries(stats.counters.api.$environment.$service.total))"
    op: "gt"
    threshold: 0.5  # Very low tolerance in production

environments:
  - name: "production"
    attributes:
      tier: "critical"
      sla: "99.9%"
  - name: "staging"
    attributes:
      tier: "standard"
  - name: "development"
    attributes:
      tier: "best-effort"

flag_metrics:
  # Latency with environment overrides
  - name: "api_slow"
    service: "payment-service"
    template:
      name: "api_latency"
    environments:
      - name: "production"
        # Uses default 300ms - critical service
      - name: "staging"
        threshold: 500   # 500ms in staging
      - name: "development"
        threshold: 1000  # 1s in development

  # Success rate with environment overrides
  - name: "api_success_rate_low"
    service: "payment-service"
    template:
      name: "api_success_rate"
    environments:
      - name: "production"
        threshold: 99.9  # Even stricter for payments
      - name: "staging"
        threshold: 95.0  # Relaxed for testing
      - name: "development"
        threshold: 90.0  # Very relaxed

  # Error rate with environment overrides
  - name: "api_error_rate_high"
    service: "payment-service"
    template:
      name: "api_error_rate"
    environments:
      - name: "production"
        # Uses default 0.5%
      - name: "staging"
        threshold: 5.0   # Allow more errors in staging
      - name: "development"
        threshold: 10.0  # Very tolerant

health_metrics:
  payment-service:
    service: "payment-service"
    component_name: "Payment Service"
    category: "compute"
    metrics:
      - "payment-service.api_slow"
      - "payment-service.api_success_rate_low"
      - "payment-service.api_error_rate_high"
    expressions:
      # Any single issue = degraded
      - expression: "payment-service.api_slow"
        weight: 1
      # Multiple issues = outage
      - expression: "payment-service.api_success_rate_low && payment-service.api_error_rate_high"
        weight: 2
      # Very low success rate = outage
      - expression: "payment-service.api_success_rate_low"
        weight: 2
```

## Complex Health Expressions

Configuration demonstrating advanced boolean expression patterns.

```yaml
---
# Complex health expressions configuration
# Shows various boolean operator combinations

datasource:
  url: "http://graphite.example.com:8080"
  timeout: 15

server:
  address: "0.0.0.0"
  port: 3000

metric_templates:
  api_latency_p50:
    query: "stats.timers.api.$environment.$service.p50"
    op: "gt"
    threshold: 200

  api_latency_p99:
    query: "stats.timers.api.$environment.$service.p99"
    op: "gt"
    threshold: 1000

  error_rate:
    query: "asPercent(sumSeries(stats.counters.api.$environment.$service.5xx), sumSeries(stats.counters.api.$environment.$service.total))"
    op: "gt"
    threshold: 1

  connection_pool_usage:
    query: "stats.gauges.db.$environment.$service.pool.usage"
    op: "gt"
    threshold: 80

  disk_usage:
    query: "stats.gauges.system.$environment.$service.disk.usage"
    op: "gt"
    threshold: 85

  memory_usage:
    query: "stats.gauges.system.$environment.$service.memory.usage"
    op: "gt"
    threshold: 90

environments:
  - name: "production"

flag_metrics:
  - name: "latency_p50_high"
    service: "database"
    template:
      name: "api_latency_p50"
    environments:
      - name: "production"

  - name: "latency_p99_high"
    service: "database"
    template:
      name: "api_latency_p99"
    environments:
      - name: "production"

  - name: "error_rate_high"
    service: "database"
    template:
      name: "error_rate"
    environments:
      - name: "production"

  - name: "connection_pool_high"
    service: "database"
    template:
      name: "connection_pool_usage"
    environments:
      - name: "production"

  - name: "disk_usage_high"
    service: "database"
    template:
      name: "disk_usage"
    environments:
      - name: "production"

  - name: "memory_usage_high"
    service: "database"
    template:
      name: "memory_usage"
    environments:
      - name: "production"

health_metrics:
  database:
    service: "database"
    component_name: "Database Cluster"
    category: "database"
    metrics:
      - "database.latency_p50_high"
      - "database.latency_p99_high"
      - "database.error_rate_high"
      - "database.connection_pool_high"
      - "database.disk_usage_high"
      - "database.memory_usage_high"
    expressions:
      # Minor degradation: elevated p50 latency (most users affected slightly)
      - expression: "database.latency_p50_high"
        weight: 1

      # Moderate degradation: high tail latency OR resource pressure
      - expression: "database.latency_p99_high || database.connection_pool_high"
        weight: 1

      # Moderate degradation: disk OR memory pressure
      - expression: "database.disk_usage_high || database.memory_usage_high"
        weight: 1

      # Severe: errors occurring
      - expression: "database.error_rate_high"
        weight: 2

      # Severe: multiple resource issues (compound problem)
      - expression: "database.connection_pool_high && database.memory_usage_high"
        weight: 2

      # Critical: latency high AND errors (system struggling)
      - expression: "database.latency_p99_high && database.error_rate_high"
        weight: 2

      # Critical: disk full AND any latency (imminent failure)
      - expression: "database.disk_usage_high && (database.latency_p50_high || database.latency_p99_high)"
        weight: 2
```

## Modular Configuration with conf.d

Split configuration across multiple files for maintainability.

### Main Configuration

```yaml
# config.yaml - Main configuration file
---
datasource:
  url: "http://graphite.example.com:8080"
  timeout: 15

server:
  address: "0.0.0.0"
  port: 3000

# Templates and environments defined here
# Services defined in conf.d/
metric_templates:
  api_latency:
    query: "stats.timers.api.$environment.$service.mean"
    op: "gt"
    threshold: 500

  api_success_rate:
    query: "asPercent(sumSeries(stats.counters.api.$environment.$service.2xx), sumSeries(stats.counters.api.$environment.$service.total))"
    op: "lt"
    threshold: 95

  api_down:
    query: "asPercent(sumSeries(stats.counters.api.$environment.$service.5xx), sumSeries(stats.counters.api.$environment.$service.total))"
    op: "eq"
    threshold: 100

environments:
  - name: "production"
  - name: "staging"

# Empty arrays - will be merged from conf.d/
flag_metrics: []
health_metrics: {}
```

### Compute Services File

> **Note:** This is a configuration fragment to be merged with the main config file.

```yaml-fragment
# conf.d/10-compute.yaml - Compute services
---
flag_metrics:
  - name: "api_slow"
    service: "nova"
    template:
      name: "api_latency"
    environments:
      - name: "production"
      - name: "staging"
        threshold: 1000

  - name: "api_success_rate_low"
    service: "nova"
    template:
      name: "api_success_rate"
    environments:
      - name: "production"
      - name: "staging"

  - name: "api_down"
    service: "nova"
    template:
      name: "api_down"
    environments:
      - name: "production"
      - name: "staging"

health_metrics:
  nova:
    service: "nova"
    component_name: "Compute Service"
    category: "compute"
    metrics:
      - "nova.api_slow"
      - "nova.api_success_rate_low"
      - "nova.api_down"
    expressions:
      - expression: "nova.api_slow || nova.api_success_rate_low"
        weight: 1
      - expression: "nova.api_down"
        weight: 2
```

### Network Services File

> **Note:** This is a configuration fragment to be merged with the main config file.

```yaml-fragment
# conf.d/20-network.yaml - Network services
---
flag_metrics:
  - name: "api_slow"
    service: "neutron"
    template:
      name: "api_latency"
    environments:
      - name: "production"
      - name: "staging"
        threshold: 1000

  - name: "api_success_rate_low"
    service: "neutron"
    template:
      name: "api_success_rate"
    environments:
      - name: "production"
      - name: "staging"

  - name: "api_down"
    service: "neutron"
    template:
      name: "api_down"
    environments:
      - name: "production"
      - name: "staging"

health_metrics:
  neutron:
    service: "neutron"
    component_name: "Network Service"
    category: "network"
    metrics:
      - "neutron.api_slow"
      - "neutron.api_success_rate_low"
      - "neutron.api_down"
    expressions:
      - expression: "neutron.api_slow || neutron.api_success_rate_low"
        weight: 1
      - expression: "neutron.api_down"
        weight: 2
```

## Production Setup with Status Dashboard

Complete production configuration including status dashboard integration.

```yaml
---
# Production configuration with status dashboard
datasource:
  url: "http://graphite.production.internal:8080"
  timeout: 30

server:
  address: "0.0.0.0"
  port: 3000

status_dashboard:
  url: "https://status.example.com"
  # Secret should be set via MP_STATUS_DASHBOARD__SECRET environment variable

metric_templates:
  api_latency:
    query: "consolidateBy(aggregate(stats.timers.openstack.api.$environment.*.$service.*.*.*.mean, 'average'), 'average')"
    op: "gt"
    threshold: 300

  api_success_rate:
    query: "asPercent(sumSeries(stats.counters.openstack.api.$environment.*.$service.*.*.{2*,3*,404}.count), sumSeries(stats.counters.openstack.api.$environment.*.$service.*.*.attempted.count))"
    op: "lt"
    threshold: 90

  api_down:
    query: "asPercent(sumSeries(stats.counters.openstack.api.$environment.*.$service.*.*.failed.count), sumSeries(stats.counters.openstack.api.$environment.*.$service.*.*.attempted.count))"
    op: "eq"
    threshold: 100

environments:
  - name: "production"
    attributes:
      region: "Region1"
      display_name: "Production"

flag_metrics:
  - name: "api_slow"
    service: "keystone"
    template:
      name: "api_latency"
    environments:
      - name: "production"

  - name: "api_success_rate_low"
    service: "keystone"
    template:
      name: "api_success_rate"
    environments:
      - name: "production"

  - name: "api_down"
    service: "keystone"
    template:
      name: "api_down"
    environments:
      - name: "production"

health_metrics:
  keystone:
    service: "keystone"
    component_name: "Identity Service"
    category: "security"
    metrics:
      - "keystone.api_slow"
      - "keystone.api_success_rate_low"
      - "keystone.api_down"
    expressions:
      - expression: "keystone.api_slow || keystone.api_success_rate_low"
        weight: 1
      - expression: "keystone.api_down"
        weight: 2
```

## Validation

Test your configuration by running the convertor:

```bash
# Basic validation - will fail fast if config is invalid
cargo run --bin cloudmon-metrics-convertor -- --config config.yaml

# With debug logging to see configuration processing
RUST_LOG=debug cargo run --bin cloudmon-metrics-convertor -- --config config.yaml
```

Successful startup indicates valid configuration. Check the logs for any warnings about missing templates or environments.

## Related Documentation

- [Overview](overview.md) - Configuration structure
- [Schema Reference](schema.md) - Complete property reference
- [Metric Templates](metric-templates.md) - Template configuration
- [Flag Metrics](flag-metrics.md) - Flag metric configuration
- [Health Metrics](health-metrics.md) - Health expression configuration
- [Environments](environments.md) - Environment configuration
