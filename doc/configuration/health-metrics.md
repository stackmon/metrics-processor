# Health Metrics Configuration

Health metrics aggregate multiple flag metrics into a single service health status using boolean expressions. The final health state is represented as a semaphore: green (healthy), yellow (degraded), or red (outage).

## Configuration

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

## Properties

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `service` | string | Yes | Service identifier |
| `component_name` | string | No | Display name for dashboards |
| `category` | string | Yes | Service category (e.g., `compute`, `network`) |
| `metrics` | array | Yes | List of flag metric names used in expressions |
| `expressions` | array | Yes | Boolean expressions with weights |

### Expression Definition

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `expression` | string | Yes | Boolean expression using flag metric names |
| `weight` | integer | Yes | Severity weight (0, 1, or 2) |

## Weight System

The weight determines the health status when an expression evaluates to `true`:

| Weight | State | Semaphore | Description |
|--------|-------|-----------|-------------|
| `0` | Healthy | 🟢 Green | Service operating normally |
| `1` | Degraded | 🟡 Yellow | Service experiencing issues but functional |
| `2` | Outage | 🔴 Red | Service unavailable or critically impaired |

**Evaluation Rule**: The final health status is the **maximum weight** among all expressions that evaluate to `true`.

### Example Evaluation

```yaml
expressions:
  - expression: "svc.api_slow"
    weight: 1
  - expression: "svc.api_down"
    weight: 2
```

| api_slow | api_down | Result | Status |
|----------|----------|--------|--------|
| false | false | weight 0 | 🟢 Healthy |
| true | false | weight 1 | 🟡 Degraded |
| false | true | weight 2 | 🔴 Outage |
| true | true | weight 2 | 🔴 Outage (max) |

## Boolean Operators

Expressions support standard boolean operators:

| Operator | Meaning | Example |
|----------|---------|---------|
| `\|\|` | OR | `a \|\| b` - true if either flag is raised |
| `&&` | AND | `a && b` - true only if both flags are raised |
| `!` | NOT | `!a` - true if flag is NOT raised |
| `+` | OR (alias) | `a + b` - same as `\|\|` |

### Operator Precedence

1. `!` (NOT) - highest
2. `&&` (AND)
3. `||` or `+` (OR) - lowest

Use parentheses to control evaluation order: `(a || b) && c`

## Expression Examples

### Simple Flag Check

```yaml
expressions:
  - expression: "service.api_slow"
    weight: 1
```

### OR - Any Condition

Flag raised if **any** condition is true:

```yaml
expressions:
  - expression: "service.api_slow || service.high_error_rate"
    weight: 1
```

### AND - All Conditions

Flag raised only if **all** conditions are true:

```yaml
expressions:
  - expression: "service.api_slow && service.high_error_rate"
    weight: 2  # Severe: both slow AND errors
```

### NOT - Inversion

```yaml
expressions:
  # Healthy only when NOT slow
  - expression: "!service.api_slow"
    weight: 0
```

### Complex Expressions

```yaml
expressions:
  # Degraded: slow OR moderate errors
  - expression: "service.api_slow || service.error_rate_medium"
    weight: 1

  # Outage: down OR (slow AND high errors)
  - expression: "service.api_down || (service.api_slow && service.error_rate_high)"
    weight: 2
```

### Alternative OR Syntax

The `+` operator works as an OR alias:

```yaml
expressions:
  - expression: "service.metric_a + service.metric_b && service.metric_c"
    weight: 1
```

## Metric Name Reference

Metrics in expressions must use the full `{service}.{name}` format and be listed in the `metrics` array:

```yaml
health_metrics:
  compute:
    service: "compute"
    category: "compute"
    metrics:
      - "compute.api_slow"       # Must list all metrics used
      - "compute.api_down"
      - "compute.high_latency"
    expressions:
      - expression: "compute.api_slow || compute.high_latency"
        weight: 1
      - expression: "compute.api_down"
        weight: 2
```

### Handling Hyphens in Names

Metric names containing hyphens are automatically converted to underscores in expressions:

```yaml
flag_metrics:
  - name: "api-slow"              # Original name with hyphen
    service: "my-service"

health_metrics:
  my-service:
    metrics:
      - "my-service.api-slow"     # Use original name here
    expressions:
      - expression: "my_service.api_slow"  # Use underscores in expression
        weight: 1
```

**Note**: The processor automatically handles this conversion, but you must use underscores in the expression string.

## Complete Examples

### Basic Service Health

```yaml
health_metrics:
  identity:
    service: "identity"
    component_name: "Identity Service"
    category: "security"
    metrics:
      - "identity.api_slow"
      - "identity.api_down"
    expressions:
      - expression: "identity.api_slow"
        weight: 1
      - expression: "identity.api_down"
        weight: 2
```

### Multi-Condition Service

```yaml
health_metrics:
  storage:
    service: "storage"
    component_name: "Object Storage"
    category: "storage"
    metrics:
      - "storage.api_slow"
      - "storage.api_success_rate_low"
      - "storage.api_down"
      - "storage.disk_full"
    expressions:
      # Yellow: performance issues
      - expression: "storage.api_slow || storage.api_success_rate_low"
        weight: 1
      # Red: service unavailable or critical infrastructure
      - expression: "storage.api_down || storage.disk_full"
        weight: 2
```

### Tiered Alert Severity

```yaml
health_metrics:
  database:
    service: "database"
    component_name: "Database Cluster"
    category: "data"
    metrics:
      - "database.replication_lag"
      - "database.connection_pool_high"
      - "database.query_slow"
      - "database.master_down"
    expressions:
      # Minor: single performance indicator
      - expression: "database.query_slow"
        weight: 1
      # Moderate: multiple issues or connection problems
      - expression: "database.replication_lag || database.connection_pool_high"
        weight: 1
      # Severe: combined performance degradation
      - expression: "database.query_slow && database.connection_pool_high"
        weight: 2
      # Critical: master failure
      - expression: "database.master_down"
        weight: 2
```

## Categories

Common category values for organizing services:

| Category | Description | Examples |
|----------|-------------|----------|
| `compute` | Compute services | VMs, containers, serverless |
| `network` | Network services | Load balancers, DNS, VPN |
| `storage` | Storage services | Object storage, block storage |
| `database` | Database services | SQL, NoSQL, caching |
| `security` | Security services | Identity, certificates, secrets |
| `management` | Management services | Monitoring, logging, orchestration |

```yaml
health_metrics:
  nova:
    category: "compute"
  neutron:
    category: "network"
  cinder:
    category: "storage"
  keystone:
    category: "security"
```

## Best Practices

### 1. Order Expressions by Severity

List expressions from lowest to highest weight for readability:

```yaml
expressions:
  - expression: "svc.minor_issue"
    weight: 1
  - expression: "svc.critical_issue"
    weight: 2
```

### 2. Use Descriptive Component Names

```yaml
health_metrics:
  nova:
    service: "nova"
    component_name: "Compute Service (Nova)"  # Clear for dashboards
```

### 3. Keep Expressions Readable

```yaml
# Good: Clear, simple expressions
expressions:
  - expression: "svc.api_slow || svc.error_rate_high"
    weight: 1

# Avoid: Overly complex single expressions
expressions:
  - expression: "((a || b) && (c || d)) || (e && !f)"
    weight: 1
```

### 4. Document Complex Logic

```yaml
expressions:
  # Degraded when API is slow OR error rate exceeds threshold
  - expression: "svc.api_slow || svc.error_rate_high"
    weight: 1
  # Outage when service is completely down
  - expression: "svc.api_down"
    weight: 2
```

## Related Documentation

- [Flag Metrics](flag-metrics.md) - Flag metric configuration
- [Schema Reference](schema.md) - Complete property reference
- [Examples](examples.md) - Working configuration samples
