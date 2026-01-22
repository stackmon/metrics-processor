# Environments Configuration

Environments define the deployment contexts (e.g., production, staging) where your services run. Each flag metric must specify which environments it applies to.

## Configuration

```yaml
environments:
  - name: "production"
    attributes:
      region: "eu-west-1"
      datacenter: "dc1"
  - name: "staging"
  - name: "development"
```

## Properties

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `name` | string | Yes | Unique environment identifier |
| `attributes` | object | No | Key-value metadata for the environment |

## Basic Configuration

At minimum, define environment names:

```yaml
environments:
  - name: "production"
  - name: "staging"
  - name: "development"
```

## Environment Attributes

Attributes provide additional metadata that can be passed to the status dashboard:

```yaml
environments:
  - name: "production"
    attributes:
      region: "eu-west-1"
      availability_zone: "eu-west-1a"
      tier: "critical"

  - name: "staging"
    attributes:
      region: "eu-west-1"
      tier: "non-critical"
```

### Common Attribute Patterns

**Regional Deployment**:
```yaml
environments:
  - name: "us-east-prod"
    attributes:
      region: "us-east-1"
      type: "production"
  - name: "us-west-prod"
    attributes:
      region: "us-west-2"
      type: "production"
  - name: "eu-prod"
    attributes:
      region: "eu-west-1"
      type: "production"
```

**Environment Tiers**:
```yaml
environments:
  - name: "production"
    attributes:
      tier: "1"
      sla: "99.9%"
  - name: "staging"
    attributes:
      tier: "2"
      sla: "99%"
  - name: "development"
    attributes:
      tier: "3"
      sla: "none"
```

## Variable Substitution

Environment names are available in metric templates via the `$environment` variable:

```yaml
metric_templates:
  api_latency:
    query: "stats.timers.api.$environment.$service.mean"
    op: "gt"
    threshold: 500

environments:
  - name: "production"
  - name: "staging"

flag_metrics:
  - name: "api_slow"
    service: "compute"
    template:
      name: "api_latency"
    environments:
      - name: "production"   # Query: stats.timers.api.production.compute.mean
      - name: "staging"      # Query: stats.timers.api.staging.compute.mean
```

## Environment-Specific Thresholds

Different environments can have different thresholds for the same metric:

```yaml
environments:
  - name: "production"
  - name: "staging"
  - name: "development"

flag_metrics:
  - name: "api_slow"
    service: "compute"
    template:
      name: "api_latency"  # Default threshold: 500ms
    environments:
      - name: "production"
        # Uses default 500ms - strict for production
      - name: "staging"
        threshold: 1000     # 1s - more lenient
      - name: "development"
        threshold: 2000     # 2s - very lenient
```

### Threshold Strategy by Environment Type

| Environment | Strategy | Example |
|-------------|----------|---------|
| Production | Strict SLA-based | 500ms, 99.9% availability |
| Staging | Moderate | 1000ms, 99% availability |
| Development | Lenient | 2000ms, 90% availability |

## Naming Conventions

### Simple Names

For single-region deployments:

```yaml
environments:
  - name: "production"
  - name: "staging"
  - name: "development"
```

### Region-Prefixed Names

For multi-region deployments:

```yaml
environments:
  - name: "us-production"
  - name: "eu-production"
  - name: "ap-production"
```

### Datacenter-Specific Names

For datacenter-aware deployments:

```yaml
environments:
  - name: "dc1-production"
  - name: "dc2-production"
  - name: "dc1-staging"
```

## Examples

### Single Environment (Development)

```yaml
environments:
  - name: "local"
```

### Standard Three-Tier

```yaml
environments:
  - name: "production"
    attributes:
      type: "prod"
      alert_channel: "#production-alerts"
  - name: "staging"
    attributes:
      type: "non-prod"
      alert_channel: "#staging-alerts"
  - name: "development"
    attributes:
      type: "non-prod"
```

### Multi-Region Production

```yaml
environments:
  - name: "us-east-1"
    attributes:
      region: "us-east-1"
      type: "production"
      primary: "true"
  - name: "us-west-2"
    attributes:
      region: "us-west-2"
      type: "production"
      primary: "false"
  - name: "eu-west-1"
    attributes:
      region: "eu-west-1"
      type: "production"
      primary: "false"
  - name: "staging"
    attributes:
      region: "us-east-1"
      type: "staging"
```

### Cloud Provider Specific

```yaml
environments:
  - name: "aws-production"
    attributes:
      cloud: "aws"
      region: "us-east-1"
  - name: "gcp-production"
    attributes:
      cloud: "gcp"
      region: "us-central1"
  - name: "azure-production"
    attributes:
      cloud: "azure"
      region: "eastus"
```

## Integration with Status Dashboard

When `status_dashboard` is configured, environment attributes are passed along with health updates:

```yaml
environments:
  - name: "production"
    attributes:
      region: "Region1"
      display_name: "Production (Region 1)"

status_dashboard:
  url: "https://status.example.com"
```

The status dashboard receives environment information to display health status per environment/region.

## Best Practices

### 1. Consistent Naming

Use consistent naming across all environments:

```yaml
# Good: Consistent pattern
environments:
  - name: "prod-us-east"
  - name: "prod-us-west"
  - name: "prod-eu"

# Avoid: Inconsistent naming
environments:
  - name: "production"
  - name: "US-West-Prod"
  - name: "eu_production"
```

### 2. Match TSDB Metric Paths

Ensure environment names match your TSDB metric path segments:

```yaml
# If your metrics are: stats.api.production.service.latency
environments:
  - name: "production"  # Matches metric path segment

# If your metrics are: stats.api.prod_us_east.service.latency
environments:
  - name: "prod_us_east"  # Matches metric path segment
```

### 3. Document Attributes

Add comments explaining attribute usage:

```yaml
environments:
  - name: "production"
    attributes:
      region: "eu-west-1"        # AWS region for status dashboard
      tier: "1"                  # Priority tier for alerting
      maintenance_window: "sun"  # Scheduled maintenance day
```

### 4. Keep Environment List Manageable

If you have many environments, consider using conf.d to organize:

```yaml
# conf.d/environments.yaml
environments:
  - name: "production"
  - name: "staging"
  - name: "development"
```

## Troubleshooting

### Environment Not Found

If you see "Environment for service not supported" errors:

1. Verify the environment is defined in `environments`
2. Check the environment name matches in `flag_metrics`
3. Ensure consistent spelling and case

### Missing Metrics for Environment

If queries return no data for an environment:

1. Verify the environment name matches your TSDB metric paths
2. Check that metrics exist for this environment in your TSDB
3. Test the query directly in your TSDB UI

## Related Documentation

- [Overview](overview.md) - Configuration structure
- [Flag Metrics](flag-metrics.md) - Using environments in flag metrics
- [Metric Templates](metric-templates.md) - Variable substitution
- [Examples](examples.md) - Complete configuration samples
