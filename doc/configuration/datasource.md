# Datasource Configuration

The `datasource` section configures the connection to your Time Series Database (TSDB), which provides the raw metrics data.

## Configuration

```yaml
datasource:
  url: "http://graphite.example.com:8080"
  timeout: 15
```

## Properties

| Property | Type | Required | Default | Description |
|----------|------|----------|---------|-------------|
| `url` | string | Yes | - | Full URL to the TSDB instance |
| `timeout` | integer | No | `10` | Query timeout in seconds |

## URL Configuration

The URL should point to your Graphite-compatible TSDB endpoint:

```yaml
# Local development
datasource:
  url: "http://localhost:8080"

# Production with HTTPS
datasource:
  url: "https://graphite.production.example.com"

# Custom port
datasource:
  url: "http://graphite.internal:2003"
```

## Timeout Configuration

The timeout controls how long the metrics-processor waits for TSDB query responses:

```yaml
datasource:
  url: "http://graphite.example.com:8080"
  timeout: 30  # Wait up to 30 seconds for queries
```

**Recommendations**:
- **Development**: 5-10 seconds (fast feedback)
- **Production**: 15-30 seconds (accommodate slow queries)
- **Large datasets**: 60+ seconds (complex aggregations)

## Environment Variable Override

Override datasource settings via environment variables:

```bash
# Override URL (useful for containerized deployments)
export MP_DATASOURCE__URL="http://graphite-prod:8080"

# Override timeout
export MP_DATASOURCE__TIMEOUT=30
```

## Examples

### Basic Development Setup

```yaml
datasource:
  url: "http://localhost:8080"
  timeout: 5
```

### Production Setup

```yaml
datasource:
  url: "https://graphite.production.example.com"
  timeout: 30
```

### Docker Compose Setup

```yaml
# config.yaml
datasource:
  url: "http://graphite:8080"
  timeout: 15
```

```yaml
# docker-compose.yaml
services:
  metrics-processor:
    environment:
      - MP_DATASOURCE__URL=http://graphite:8080
```

## Supported TSDB Types

Currently, the metrics-processor supports:

- **Graphite**: Full support for Graphite render API

The TSDB type is automatically detected from the query response format.

## Connection Verification

To verify your datasource connection, start the convertor and check the logs:

```bash
RUST_LOG=debug cargo run --bin cloudmon-metrics-convertor -- --config config.yaml
```

Successful connection shows queries being executed. Connection failures appear as timeout or connection refused errors.

## Troubleshooting

### Connection Refused

```
Error: connection refused
```

**Causes**:
- TSDB not running
- Wrong URL or port
- Firewall blocking connection

**Solutions**:
1. Verify TSDB is running: `curl http://your-graphite:8080/render?format=json`
2. Check URL in configuration
3. Verify network connectivity

### Query Timeout

```
Error: request timed out
```

**Causes**:
- Slow TSDB queries
- Network latency
- Insufficient timeout value

**Solutions**:
1. Increase `timeout` value
2. Optimize TSDB queries in templates
3. Check TSDB performance

### Invalid URL

```
Error: invalid URL
```

**Solutions**:
1. Ensure URL includes protocol (`http://` or `https://`)
2. Verify no trailing slashes or paths

## Related Documentation

- [Overview](overview.md) - Configuration structure
- [Metric Templates](metric-templates.md) - Query configuration
- [Schema Reference](schema.md) - Complete property reference
