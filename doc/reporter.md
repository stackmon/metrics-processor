# CloudMon Metrics Reporter

The **reporter** component is a background service that polls the convertor API and sends health status updates to a status dashboard (e.g., Atlassian Statuspage).

## Overview

The reporter acts as a bridge between the convertor's real-time health evaluation and external status dashboards:
1. Polls convertor API at regular intervals
2. Checks if service health has degraded (status > 0)
3. Sends notifications to status dashboard
4. Handles authentication and dashboard-specific protocols

**Key Characteristics**:
- **Background service**: Runs as daemon or scheduled job
- **Stateless polling**: Queries convertor each interval
- **Conditional notifications**: Only notifies when health degraded
- **Dashboard integration**: Handles JWT authentication and API protocols

## Architecture

```
┌──────────────────────────────────────────────┐
│        Reporter (endless loop)               │
│                                              │
│  while true:                                 │
│    sleep(poll_interval)                      │
│    ┌───────────────────────────────────┐    │
│    │ 1. Query Convertor API            │    │
│    │    GET /v1/health for all services│    │
│    └─────────────┬─────────────────────┘    │
│                  ▼                           │
│    ┌───────────────────────────────────┐    │
│    │ 2. Check Health Status            │    │
│    │    if status > 0: send update     │    │
│    └─────────────┬─────────────────────┘    │
│                  ▼                           │
│    ┌───────────────────────────────────┐    │
│    │ 3. Generate JWT Token             │    │
│    │    HMAC-SHA256 with secret        │    │
│    └─────────────┬─────────────────────┘    │
│                  ▼                           │
│    ┌───────────────────────────────────┐    │
│    │ 4. Send to Status Dashboard       │    │
│    │    POST to dashboard API          │    │
│    └───────────────────────────────────┘    │
│                                              │
└──────────────────────────────────────────────┘
```

## Processing Flow

### 1. Polling Loop

The reporter runs an infinite loop:

```rust
loop {
    // Sleep for configured interval
    tokio::time::sleep(Duration::from_secs(poll_interval)).await;
    
    // Query all services
    for service in services {
        let health = query_convertor(service, environment).await;
        
        if health.status > 0 {
            send_to_dashboard(health).await;
        }
    }
}
```

**Configuration**:
- Poll interval: Typically 60-300 seconds
- Services to monitor: Defined in configuration
- Environments: Usually production only

### 2. Health Status Check

**Logic**:
- Status 0 (healthy): No action, service operating normally
- Status 1 (degraded): Send incident to dashboard
- Status 2 (outage): Send critical incident to dashboard

**Threshold Behavior**:
- Reporter does not interpret status values
- Dashboard receives raw status (0/1/2)
- Dashboard decides incident creation/update logic

### 3. Dashboard Integration

#### Authentication

The reporter uses JWT tokens for authentication:

```
Header:
{
  "alg": "HS256",
  "typ": "JWT"
}

Payload:
{
  "service": "api",
  "environment": "production",
  "status": 1,
  "timestamp": 1640000000
}

Signature:
HMAC-SHA256(
  base64(header) + "." + base64(payload),
  secret
)
```

**Token Generation**:
1. Create payload with service info
2. Sign with HMAC-SHA256 using shared secret
3. Encode as JWT token
4. Include in `Authorization: Bearer <token>` header

#### API Request

```bash
curl -X POST https://dashboard.example.com/api/incidents \
  -H "Authorization: Bearer eyJhbGc..." \
  -H "Content-Type: application/json" \
  -d '{
    "service": "api",
    "environment": "production",
    "status": 1,
    "message": "Service degraded",
    "timestamp": "2024-01-20T12:00:00Z"
  }'
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

# Expected logs:
INFO Polling convertor for service: api
INFO Health status: 1 (degraded)
INFO Sent notification to dashboard: success
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
