# Flag Metrics Configuration

Flag metrics are binary indicators (raised/lowered) that represent whether a specific condition is met. They form the foundation for health metric expressions.

## Configuration

```yaml
flag_metrics:
  - name: "slow_response"
    service: "api-gateway"
    template:
      name: "api_latency"
    environments:
      - name: "production"
      - name: "staging"
        threshold: 1000
```

## Properties

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `name` | string | Yes | Unique metric identifier within the service |
| `service` | string | Yes | Service this metric belongs to |
| `template` | object | Yes | Reference to a metric template |
| `environments` | array | Yes | Environment-specific configurations |

### Template Reference

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `name` | string | Yes | Template name (key in `metric_templates`) |
| `vars` | object | No | Additional template variables |

### Environment Configuration

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `name` | string | Yes | Environment name (must exist in `environments`) |
| `threshold` | number | No | Override threshold for this environment |

## How Flag Metrics Work

1. **Query Execution**: The template query is executed with variable substitution
2. **Value Comparison**: The result is compared to the threshold using the operator
3. **Flag State**: The flag is raised (`true`) or lowered (`false`) based on the comparison

```
Query Result: 750ms
Threshold: 500ms
Operator: gt (greater than)
Result: 750 > 500 = true → Flag RAISED
```

## Comparison Operators

Operators are defined in the template but affect flag behavior:

| Operator | Flag Raised When | Example |
|----------|------------------|---------|
| `gt` | value > threshold | Latency 750ms > 500ms threshold |
| `lt` | value < threshold | Success rate 85% < 90% threshold |
| `eq` | value == threshold | Error rate 100% == 100% threshold |

## Threshold Overrides

The threshold can be customized per environment:

```yaml
metric_templates:
  api_latency:
    query: "stats.timers.api.$environment.$service.mean"
    op: "gt"
    threshold: 500  # Default threshold

flag_metrics:
  - name: "slow_response"
    service: "api-gateway"
    template:
      name: "api_latency"
    environments:
      - name: "production"
        # Uses default 500ms threshold
      - name: "staging"
        threshold: 1000  # Override: staging is more lenient
      - name: "development"
        threshold: 2000  # Override: dev is even more lenient
```

### Common Override Patterns

**Production vs Non-Production**:
```yaml
environments:
  - name: "production"
    # Strict: 500ms
  - name: "staging"
    threshold: 750  # 50% more lenient
  - name: "development"
    threshold: 1500  # 3x more lenient
```

**Regional Differences**:
```yaml
environments:
  - name: "us-east"
    # Default threshold
  - name: "ap-southeast"
    threshold: 600  # Account for network latency
```

## Flag Metric Naming

Flag metrics are internally referenced using the format: `{service}.{name}`

```yaml
flag_metrics:
  - name: "slow_response"      # → api-gateway.slow_response
    service: "api-gateway"

  - name: "error_rate_high"    # → api-gateway.error_rate_high
    service: "api-gateway"
```

This naming is important when referencing flags in health metric expressions.

## Examples

### Basic Service Monitoring

```yaml
flag_metrics:
  - name: "api_slow"
    service: "user-service"
    template:
      name: "api_latency"
    environments:
      - name: "production"
```

### Multi-Environment with Overrides

```yaml
flag_metrics:
  - name: "success_rate_low"
    service: "payment-service"
    template:
      name: "api_success_rate"
    environments:
      - name: "production"
        threshold: 99.9  # Strict SLA
      - name: "staging"
        threshold: 95.0  # Relaxed for testing
      - name: "development"
        threshold: 90.0  # Very relaxed
```

### Multiple Flags per Service

```yaml
flag_metrics:
  # API performance flags
  - name: "api_slow"
    service: "compute"
    template:
      name: "api_latency"
    environments:
      - name: "production"

  - name: "api_success_rate_low"
    service: "compute"
    template:
      name: "api_success_rate"
    environments:
      - name: "production"

  # API availability flag
  - name: "api_down"
    service: "compute"
    template:
      name: "api_down"
    environments:
      - name: "production"
```

### Complete Service Configuration

```yaml
metric_templates:
  api_latency:
    query: "stats.timers.api.$environment.$service.mean"
    op: "gt"
    threshold: 500

  api_success_rate:
    query: "asPercent(stats.counters.api.$environment.$service.2xx, stats.counters.api.$environment.$service.total)"
    op: "lt"
    threshold: 95

  api_down:
    query: "asPercent(stats.counters.api.$environment.$service.5xx, stats.counters.api.$environment.$service.total)"
    op: "eq"
    threshold: 100

flag_metrics:
  - name: "api_slow"
    service: "identity"
    template:
      name: "api_latency"
    environments:
      - name: "production"
      - name: "staging"
        threshold: 1000

  - name: "api_success_rate_low"
    service: "identity"
    template:
      name: "api_success_rate"
    environments:
      - name: "production"
      - name: "staging"

  - name: "api_down"
    service: "identity"
    template:
      name: "api_down"
    environments:
      - name: "production"
      - name: "staging"
```

## Using with conf.d

For large deployments, organize flag metrics in separate files:

```
conf.d/
├── 10-compute-services.yaml
├── 20-network-services.yaml
└── 30-storage-services.yaml
```

```yaml
# conf.d/10-compute-services.yaml
flag_metrics:
  - name: "api_slow"
    service: "nova"
    template:
      name: "api_latency"
    environments:
      - name: "production"
  # ... more compute service flags
```

## Best Practices

### 1. Consistent Naming

Use consistent naming patterns across services:

```yaml
# Good: Consistent pattern
- name: "api_slow"
- name: "api_success_rate_low"
- name: "api_down"

# Avoid: Inconsistent naming
- name: "slow"
- name: "low_success"
- name: "down"
```

### 2. Define All Environments

Ensure every flag metric covers all required environments:

```yaml
flag_metrics:
  - name: "api_slow"
    service: "compute"
    template:
      name: "api_latency"
    environments:
      - name: "production"
      - name: "staging"
      - name: "development"  # Don't forget non-prod!
```

### 3. Document Thresholds

Add comments explaining threshold values:

```yaml
flag_metrics:
  - name: "api_slow"
    service: "compute"
    template:
      name: "api_latency"
    environments:
      - name: "production"
        # SLA: 99th percentile < 500ms
      - name: "staging"
        threshold: 1000  # Shared infrastructure, higher latency expected
```

## Related Documentation

- [Metric Templates](metric-templates.md) - Template configuration
- [Health Metrics](health-metrics.md) - Using flags in expressions
- [Environments](environments.md) - Environment configuration
- [Examples](examples.md) - Complete configuration samples
