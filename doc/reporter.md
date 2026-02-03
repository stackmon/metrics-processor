# CloudMon Metrics Reporter

The **reporter** component is a background service that polls the convertor API and creates incidents in the Status Dashboard when health issues are detected.

## Overview

The reporter acts as a bridge between the convertor's real-time health evaluation and external status dashboards:
1. Initializes component ID cache from Status Dashboard API V2
2. Polls convertor API at regular intervals (60 seconds)
3. Checks if service health has degraded (impact > 0)
4. Creates incidents via Status Dashboard API
5. Handles HMAC-JWT authentication

**Key Characteristics**:
- **Background service**: Runs as daemon or scheduled job
- **Component caching**: Maintains ID cache with automatic refresh on miss
- **V2 API integration**: Uses Status Dashboard V2 endpoints for incident creation
- **Stateless polling**: Queries convertor each interval
- **Startup reliability**: 3 retry attempts with 60s delays for initial cache load

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│        Reporter (endless loop)                               │
│                                                              │
│  startup:                                                    │
│    ┌───────────────────────────────────────┐                │
│    │ 1. Fetch Components (3 retries)       │                │
│    │    GET /v2/components                 │                │
│    │    Build component ID cache           │                │
│    └─────────────┬─────────────────────────┘                │
│                  ▼                                           │
│  while true:                                                 │
│    sleep(60s)                                                │
│    ┌───────────────────────────────────────┐                │
│    │ 2. Query Convertor API                │                │
│    │    GET /api/v1/health for all services│                │
│    └─────────────┬─────────────────────────┘                │
│                  ▼                                           │
│    ┌───────────────────────────────────────┐                │
│    │ 3. Check Health Status                │                │
│    │    if impact > 0: create incident     │                │
│    └─────────────┬─────────────────────────┘                │
│                  ▼                                           │
│    ┌───────────────────────────────────────┐                │
│    │ 4. Resolve Component ID               │                │
│    │    Lookup in cache (refresh if miss)  │                │
│    └─────────────┬─────────────────────────┘                │
│                  ▼                                           │
│    ┌───────────────────────────────────────┐                │
│    │ 5. Create Incident via V2 API         │                │
│    │    POST /v2/events                    │                │
│    └───────────────────────────────────────┘                │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

## Processing Flow

### 1. Component Cache Initialization

At startup, the reporter fetches all components and builds an ID cache:

```rust
// Fetch components from Status Dashboard V2 API
let components = fetch_components(&client, &url, &headers).await?;

// Build cache: HashMap<(name, sorted_attributes), component_id>
let cache = build_component_id_cache(components);
```

**Retry Logic**:
- 3 attempts with 60-second delays between retries
- Reporter exits if all attempts fail (FR-007)

### 2. Polling Loop

The reporter runs an infinite loop with 60-second intervals:

```rust
loop {
    // For each environment and service
    for env in environments {
        for service in services {
            let health = query_convertor(service, env).await?;
            
            if health.impact > 0 {
                // Resolve component ID from cache
                let component_id = find_component_id(&cache, &component)?;
                
                // Create incident via V2 API
                let incident = build_incident_data(component_id, impact, timestamp);
                create_incident(&client, &url, &headers, &incident).await?;
            }
        }
    }
    sleep(Duration::from_secs(60)).await;
}
```

### 3. Component ID Resolution

Components are looked up using subset attribute matching:

```rust
// Config attributes must be a SUBSET of cache attributes
// Example: config has {region: "EU-DE"}
//          cache has {region: "EU-DE", category: "Storage"}
// Result: MATCH (config attrs are subset of cache attrs)
```

**Cache Miss Handling**:
- If component not found, refresh cache once
- Retry lookup after refresh
- Log warning and skip if still not found

### 4. Incident Creation

Incidents are created with static, secure payloads:

```json
{
  "title": "System incident from monitoring system",
  "description": "System-wide incident affecting one or multiple components. Created automatically.",
  "impact": 2,
  "components": [218],
  "start_date": "2024-01-20T12:00:00Z",
  "system": true,
  "type": "incident"
}
```

**Important**:
- Title and description are static (not user-controlled) for security
- Timestamp is RFC3339 format, minus 1 second from metric timestamp
- `system: true` indicates auto-generated incident

### 5. Authentication

The reporter uses HMAC-JWT for authentication (unchanged from V1):

```rust
// Generate HMAC-JWT token
let headers = build_auth_headers(secret.as_deref());
// Headers contain: Authorization: Bearer <jwt-token>
```

**Token Format**:
- Algorithm: HMAC-SHA256
- Claims: `{"stackmon": "dummy"}`
- Optional: No secret = no auth header (for environments without auth)

## Module Structure

The Status Dashboard integration is consolidated in `src/sd.rs`:

```rust
// src/sd.rs - Status Dashboard integration module

// Data Structures
pub struct ComponentAttribute { name, value }
pub struct Component { name, attributes }
pub struct StatusDashboardComponent { id, name, attributes }
pub struct IncidentData { title, description, impact, components, start_date, system, type }
pub type ComponentCache = HashMap<(String, Vec<ComponentAttribute>), u32>;

// Authentication
pub fn build_auth_headers(secret: Option<&str>) -> HeaderMap

// V2 API Functions
pub async fn fetch_components(...) -> Result<Vec<StatusDashboardComponent>>
pub fn build_component_id_cache(...) -> ComponentCache
pub fn find_component_id(...) -> Option<u32>
pub fn build_incident_data(...) -> IncidentData
pub async fn create_incident(...) -> Result<()>
```

## Configuration

The reporter requires configuration for:

### Convertor Connection

```yaml
convertor:
  url: "http://localhost:3005"
  timeout: 10  # seconds
```

### Status Dashboard

```yaml
status_dashboard:
  url: "https://dashboard.example.com"
  secret: "your-jwt-secret"
```

| Property | Type   | Required | Default | Description                           |
|----------|--------|----------|---------|---------------------------------------|
| `url`    | string | Yes      | -       | Status Dashboard API URL              |
| `secret` | string | No       | -       | JWT signing secret for authentication |

### Health Query Configuration

```yaml
health_query:
  query_from: "-5min"  # Start of health metric query window (default: "-5min")
  query_to: "-2min"    # End of health metric query window (default: "-2min")
```

| Property     | Type   | Required | Default | Description                                 |
|--------------|--------|----------|---------|---------------------------------------------|
| `query_from` | string | No       | `-5min` | Start time offset for health metric queries |
| `query_to`   | string | No       | `-2min` | End time offset for health metric queries   |

### Polling Configuration

```yaml
reporter:
  poll_interval: 60  # seconds
  services:
    - name: "api"
      environment: "production"
    - name: "database"
      environment: "production"
```

See [Configuration Reference](configuration/overview.md) for complete details.

## Running Reporter

### Basic Usage

```bash
cloudmon-metrics-reporter --config config.yaml
```

### Docker

```bash
docker run -v /path/to/config.yaml:/config.yaml \
  cloudmon-metrics:latest \
  cloudmon-metrics-reporter --config /config.yaml
```

### Kubernetes

Deploy as a Deployment with single replica:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: cloudmon-reporter
spec:
  replicas: 1  # Only one instance needed
  selector:
    matchLabels:
      app: cloudmon-reporter
  template:
    metadata:
      labels:
        app: cloudmon-reporter
    spec:
      containers:
      - name: reporter
        image: cloudmon-metrics:latest
        command: ["cloudmon-metrics-reporter"]
        args: ["--config", "/config/config.yaml"]
        volumeMounts:
        - name: config
          mountPath: /config
      volumes:
      - name: config
        configMap:
          name: cloudmon-config
```

### Environment Variables

Override configuration:

```bash
MP_STATUS_DASHBOARD__SECRET=new-secret \
MP_CONVERTOR__URL=http://convertor-svc:3005 \
cloudmon-metrics-reporter --config config.yaml
```

## Operational Considerations

### High Availability

**Single Instance Recommended**:
- Reporter is stateless but should run single instance
- Multiple instances would send duplicate notifications
- Use Kubernetes `replicas: 1` with pod disruption budget

**Failure Recovery**:
- If reporter crashes, next poll cycle catches up
- No persistent state to recover
- Dashboard handles duplicate notifications gracefully

### Monitoring

**Recommended Metrics**:
- Poll cycle duration
- Notification success rate
- API errors (convertor, dashboard)
- JWT token generation failures

**Logging**:
```bash
RUST_LOG=info cloudmon-metrics-reporter --config config.yaml
```

#### Log Examples

**Startup - Component Fetch Success**:
```
2026-01-29T10:30:00.123456Z  INFO cloudmon_metrics_reporter: starting cloudmon-metrics-reporter
2026-01-29T10:30:00.234567Z  INFO cloudmon_metrics_reporter: attempting to fetch components from Status Dashboard attempt=1 max_attempts=3
2026-01-29T10:30:01.345678Z  INFO cloudmon_metrics_reporter: successfully fetched components from Status Dashboard attempt=1 component_count=42
2026-01-29T10:30:01.456789Z  INFO cloudmon_metrics_reporter: starting metric reporter thread
```

**Incident Creation - Full Decision Context**:
When the reporter decides to create an incident, it logs all the information needed to understand why, including the metric template configuration:
```
2026-01-29T10:30:44.123456Z  INFO cloudmon_metrics_reporter: creating incident: health metric indicates service degradation environment="prod-eu" service="as" component_name="Auto Scaling" component_id=218 query_from="-5min" query_to="-2min" metric_timestamp=1738145400 impact=2 triggered_metrics=["as.api_down(query=asPercent(smartSummarize(...), smartSummarize(...)), op=eq, threshold=100)"] matched_expression="as.api_down"
2026-01-29T10:30:45.234567Z  INFO cloudmon_metrics_reporter: incident created successfully component_id=218 impact=2
```

**Incident with Multiple Triggered Metrics (weight=1, degraded)**:
```
2026-01-29T10:30:44.123456Z  INFO cloudmon_metrics_reporter: creating incident: health metric indicates service degradation environment="prod-eu" service="as" component_name="Auto Scaling" component_id=218 query_from="-5min" query_to="-2min" metric_timestamp=1738145400 impact=1 triggered_metrics=["as.api_slow(query=smartSummarize(...), op=gt, threshold=1000)", "as.api_success_rate_low(query=asPercent(...), op=lt, threshold=99)"] matched_expression="as.api_slow || as.api_success_rate_low"
```

**Component Cache Miss and Refresh**:
```
2026-01-29T10:31:00.123456Z  INFO cloudmon_metrics_reporter: component not found in cache, attempting cache refresh component_name="New Service" service="new-service" environment="prod-eu"
2026-01-29T10:31:01.234567Z  INFO cloudmon_metrics_reporter: cache refreshed component_count=43
```

**Error - Failed to Create Incident**:
```
2026-01-29T10:30:45.123456Z ERROR cloudmon_metrics_reporter: failed to create incident error="HTTP 422: Invalid component ID" component_id=999 service="compute" environment="prod-eu"
```

**Error - Component Not Found After Refresh**:
```
2026-01-29T10:31:02.123456Z  WARN cloudmon_metrics_reporter: component not found in cache even after refresh, skipping incident creation component_name="Unknown Service" service="unknown" environment="prod-eu"
```

**Startup Failure - Component Fetch Failed**:
```
2026-01-29T10:30:00.123456Z  INFO cloudmon_metrics_reporter: attempting to fetch components from Status Dashboard attempt=1 max_attempts=3
2026-01-29T10:30:01.234567Z  WARN cloudmon_metrics_reporter: failed to fetch components, will retry after delay error="Connection refused" attempt=1 max_attempts=3 retry_delay_seconds=60
2026-01-29T10:31:01.345678Z  WARN cloudmon_metrics_reporter: failed to fetch components, will retry after delay error="Connection refused" attempt=2 max_attempts=3 retry_delay_seconds=60
2026-01-29T10:32:01.456789Z ERROR cloudmon_metrics_reporter: failed to fetch components after all retry attempts, reporter cannot start error="Connection refused" attempt=3 max_attempts=3
```

### Performance

**Scaling**:
- Reporter does not scale horizontally (duplicate notifications)
- Vertical scaling not needed (low resource usage)
- Typical usage: <50MB memory, <1% CPU

**Network**:
- Outbound HTTP to convertor API
- Outbound HTTPS to status dashboard
- No inbound connections needed

## Error Handling

### Convertor API Failures

**Behavior**:
- Retry with exponential backoff
- Log error and continue to next poll
- Do not send stale data to dashboard

### Dashboard API Failures

**Behavior**:
- Retry up to 3 times
- Log error and continue
- Next poll cycle will retry if health still degraded

### Authentication Failures

**Cause**: Invalid JWT secret
**Solution**: Update `status_dashboard.secret` in configuration

## Use Cases

### 1. Customer-Facing Status Page

Reporter updates public status page when services degrade:
- Poll every 60 seconds
- Send updates on degraded (1) or outage (2)
- Dashboard shows incidents to customers

### 2. Internal Alerting

Reporter triggers internal alerts:
- Poll every 30 seconds
- Integrate with PagerDuty or Slack
- Dashboard routes to on-call team

### 3. SLA Tracking

Reporter records incidents for SLA reporting:
- Poll every 300 seconds
- Dashboard logs all incidents
- Generate monthly uptime reports

## Troubleshooting

### "Connection refused to convertor"

**Cause**: Convertor not running or incorrect URL
**Solution**: Verify convertor is accessible at configured URL

```bash
curl http://localhost:3005/v1/health?service=api&environment=prod&from=2024-01-01T00:00:00Z&to=2024-01-01T01:00:00Z
```

### "Dashboard authentication failed"

**Cause**: Invalid JWT secret
**Solution**: Ensure `status_dashboard.secret` matches dashboard configuration

### "No services being polled"

**Cause**: Services not configured in reporter section
**Solution**: Add services to `reporter.services` list in configuration

See [Troubleshooting Guide](guides/troubleshooting.md) for more solutions.

## Related Documentation

- [Architecture Overview](architecture/overview.md)
- [API Reference](api/endpoints.md)
- [Configuration Reference](configuration/overview.md)
- [Convertor Component](convertor.md)
- [Deployment Guide](guides/deployment.md)
