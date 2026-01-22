# Metric Templates

Metric templates define reusable TSDB queries with variable substitution. Templates allow you to define a query pattern once and reuse it across multiple services and environments.

## Configuration

```yaml
metric_templates:
  api_latency:
    query: "stats.timers.api.$environment.$service.mean"
    op: "gt"
    threshold: 500
```

## Properties

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `query` | string | Yes | TSDB query with variable placeholders |
| `op` | string | Yes | Comparison operator: `lt`, `gt`, `eq` |
| `threshold` | number | Yes | Default threshold value |

## Variable Substitution

Templates support variable substitution using the `$variable` syntax. Variables are replaced with actual values when the template is used by a flag metric.

### Built-in Variables

| Variable | Description | Source |
|----------|-------------|--------|
| `$service` | Service name | From `flag_metrics[].service` |
| `$environment` | Environment name | From `flag_metrics[].environments[].name` |

### Example

Template definition:
```yaml
metric_templates:
  api_success_rate:
    query: "asPercent(sumSeries(stats.counters.api.$environment.$service.2xx), sumSeries(stats.counters.api.$environment.$service.total))"
    op: "lt"
    threshold: 95
```

Flag metric using the template:
```yaml
flag_metrics:
  - name: "success_rate_low"
    service: "user-service"
    template:
      name: "api_success_rate"
    environments:
      - name: "production"
```

Resulting query for production:
```
asPercent(sumSeries(stats.counters.api.production.user-service.2xx), sumSeries(stats.counters.api.production.user-service.total))
```

## Comparison Operators

The `op` field defines how the query result is compared to the threshold:

| Operator | Meaning | Flag Raised When | Use Case |
|----------|---------|------------------|----------|
| `lt` | Less than | value < threshold | Success rates, availability |
| `gt` | Greater than | value > threshold | Latency, error counts |
| `eq` | Equal to | value == threshold | Exact match conditions |

### Examples

```yaml
metric_templates:
  # Flag raised when success rate drops below 90%
  low_success_rate:
    query: "stats.gauges.api.$environment.$service.success_rate"
    op: "lt"
    threshold: 90

  # Flag raised when latency exceeds 500ms
  high_latency:
    query: "stats.timers.api.$environment.$service.mean"
    op: "gt"
    threshold: 500

  # Flag raised when error rate hits 100% (complete outage)
  total_failure:
    query: "stats.gauges.api.$environment.$service.error_rate"
    op: "eq"
    threshold: 100
```

## Real-World Template Examples

### API Success Rate

```yaml
metric_templates:
  api_success_rate_low:
    query: "asPercent(sumSeries(stats.counters.openstack.api.$environment.*.$service.*.*.{2*,3*,404}.count), sumSeries(stats.counters.openstack.api.$environment.*.$service.*.*.attempted.count))"
    op: "lt"
    threshold: 90
```

### API Latency

```yaml
metric_templates:
  api_slow:
    query: "consolidateBy(aggregate(stats.timers.openstack.api.$environment.*.$service.*.*.*.mean, 'average'), 'average')"
    op: "gt"
    threshold: 300
```

### Complete Service Down

```yaml
metric_templates:
  api_down:
    query: "asPercent(sumSeries(stats.counters.openstack.api.$environment.*.$service.*.*.failed.count), sumSeries(stats.counters.openstack.api.$environment.*.$service.*.*.attempted.count))"
    op: "eq"
    threshold: 100
```

### Error Rate

```yaml
metric_templates:
  error_rate_high:
    query: "asPercent(sumSeries(stats.counters.api.$environment.$service.5xx), sumSeries(stats.counters.api.$environment.$service.total))"
    op: "gt"
    threshold: 5
```

## Threshold Behavior

Thresholds defined in templates serve as defaults. Individual flag metrics can override the threshold per environment:

```yaml
metric_templates:
  api_latency:
    query: "stats.timers.api.$environment.$service.mean"
    op: "gt"
    threshold: 500  # Default: 500ms

flag_metrics:
  - name: "slow_response"
    service: "api-gateway"
    template:
      name: "api_latency"
    environments:
      - name: "production"
        # Uses default 500ms threshold
      - name: "staging"
        threshold: 1000  # Override: 1000ms for staging
```

## Best Practices

### 1. Name Templates Descriptively

```yaml
# Good: Describes what the flag indicates
metric_templates:
  api_success_rate_below_sla:
    query: "..."

# Avoid: Vague naming
metric_templates:
  metric1:
    query: "..."
```

### 2. Group Related Templates

```yaml
metric_templates:
  # API Performance
  api_latency_p99:
    query: "stats.timers.api.$environment.$service.p99"
    op: "gt"
    threshold: 1000

  api_latency_mean:
    query: "stats.timers.api.$environment.$service.mean"
    op: "gt"
    threshold: 300

  # API Reliability
  api_success_rate:
    query: "..."
    op: "lt"
    threshold: 99

  api_error_rate:
    query: "..."
    op: "gt"
    threshold: 1
```

### 3. Use Separate conf.d File

For large deployments, keep templates in a dedicated file:

```
conf.d/
└── 01-templates.yaml
```

```yaml
# conf.d/01-templates.yaml
metric_templates:
  api_latency:
    query: "stats.timers.api.$environment.$service.mean"
    op: "gt"
    threshold: 500
  # ... more templates
```

## Troubleshooting

### Variable Not Substituted

If you see `$service` or `$environment` in your logs instead of actual values:

1. Verify variable syntax uses `$` prefix (not `${...}`)
2. Check that the flag metric properly references the template

### Query Returns No Data

1. Test the query directly in Graphite UI
2. Verify the metric path exists for the service/environment combination
3. Check time range in API request

## Related Documentation

- [Flag Metrics](flag-metrics.md) - Using templates in flag metrics
- [Schema Reference](schema.md) - Complete property reference
- [Examples](examples.md) - Working configuration samples
