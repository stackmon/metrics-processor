# CloudMon Metrics Convertor

The **convertor** component evaluates service health by processing raw time-series metrics from a TSDB and converting them into semaphore-like health indicators (0=healthy, 1=degraded, 2=outage).

## Overview

The convertor is a stateless HTTP API server that:
1. Accepts health queries via REST API
2. Queries configured flag metrics from TSDB
3. Evaluates boolean health expressions
4. Returns computed health status with metric details

**Key Characteristics**:
- **Stateless**: No persistent storage, pure function of configuration + TSDB data
- **Real-time**: Evaluates metrics on-demand when API is queried
- **Configuration-driven**: All logic defined in YAML, no code changes needed
- **HTTP API**: RESTful endpoints for integration with dashboards and reporters

## Architecture

```
┌─────────────┐
│   Client    │ (Status Dashboard, Reporter, curl)
│   Request   │
└──────┬──────┘
       │ GET /v1/health?service=api&environment=prod
       ▼
┌─────────────────────────────────────────────────┐
│         Convertor (cloudmon-metrics)            │
│  ┌───────────────────────────────────────────┐  │
│  │  1. Parse Query Parameters                │  │
│  │     - service, environment, from, to      │  │
│  └───────────────────┬───────────────────────┘  │
│                      ▼                           │
│  ┌───────────────────────────────────────────┐  │
│  │  2. Load Configuration                    │  │
│  │     - flag_metrics for service            │  │
│  │     - health_metrics definition           │  │
│  └───────────────────┬───────────────────────┘  │
│                      ▼                           │
│  ┌───────────────────────────────────────────┐  │
│  │  3. Query TSDB                            │  │
│  │     - For each flag metric:               │  │
│  │       * Substitute variables ($service)   │  │
│  │       * Execute query (graphite.query())  │  │
│  └───────────────────┬───────────────────────┘  │
│                      ▼                           │
│  ┌───────────────────────────────────────────┐  │
│  │  4. Evaluate Flag Metrics                 │  │
│  │     - Compare results to thresholds       │  │
│  │     - Apply operators (gt, lt, eq)        │  │
│  │     - Determine flag state (true/false)   │  │
│  └───────────────────┬───────────────────────┘  │
│                      ▼                           │
│  ┌───────────────────────────────────────────┐  │
│  │  5. Evaluate Health Expressions           │  │
│  │     - Parse boolean expressions           │  │
│  │     - Substitute flag states              │  │
│  │     - Find highest matching weight        │  │
│  └───────────────────┬───────────────────────┘  │
│                      ▼                           │
│  ┌───────────────────────────────────────────┐  │
│  │  6. Build Response                        │  │
│  │     - Service name, category              │  │
│  │     - Health status (0/1/2)               │  │
│  │     - Metric details with values          │  │
│  └───────────────────┬───────────────────────┘  │
└────────────────────────┼──────────────────────────┘
                         ▼
                 JSON Response
```

## Processing Pipeline

### Step 1: Flag Metrics Evaluation

**Purpose**: Convert raw TSDB metrics into binary indicators (flags)

**Process**:
1. For each configured flag metric in the service:
   - Load metric template (query, operator, threshold)
   - Substitute variables: `$service`, `$environment`
   - Execute TSDB query for time range
   - Parse numeric result
   - Apply comparison operator:
     - `gt`: result > threshold → flag raised
     - `lt`: result < threshold → flag raised
     - `eq`: result == threshold → flag raised
   - Store flag state (true = raised, false = lowered)

**Example**:
```yaml
# Configuration
metric_templates:
  api_slow:
    query: "stats.timers.$service.$environment.mean"
    op: "gt"
    threshold: 500

flag_metrics:
  - name: "api_slow"
    service: "api"
    template:
      name: "api_slow"
    environments:
      - name: "production"
        threshold: 1000  # Override for production
```

**Evaluation**:
- Query: `stats.timers.api.production.mean` from TSDB
- Result: `1250` ms
- Comparison: `1250 > 1000` → `true`
- Flag state: `api.api_slow = RAISED`

### Step 2: Health Metrics Evaluation

**Purpose**: Combine flag states using boolean logic to determine overall service health

**Process**:
1. For each health expression:
   - Parse boolean expression (e.g., `api_slow || api_error_rate_high`)
   - Substitute flag states:
     - `api_slow` → `true`
     - `api_error_rate_high` → `false`
   - Evaluate expression: `true || false` → `true`
   - If expression matches, record weight (0/1/2)
2. Return highest matching weight as health status

**Example**:
```yaml
health_metrics:
  api:
    service: "api"
    category: "compute"
    metrics:
      - "api.api_slow"
      - "api.api_error_rate_high"
    expressions:
      - expression: "api.api_slow || api.api_error_rate_high"
        weight: 1  # Degraded
      - expression: "api.api_down"
        weight: 2  # Outage
```

**Evaluation**:
- Expression 1: `true || false` → `true` → weight 1
- Expression 2: `false` → no match
- Final health status: `1` (degraded)

## API Endpoints

### GET /v1/health

Query health status for a service.

**Parameters**:
- `service`: Service name (required)
- `environment`: Environment name (required)
- `from`: Start time (ISO 8601, required)
- `to`: End time (ISO 8601, required)

**Response**:
```json
{
  "name": "api",
  "category": "compute",
  "environment": "production",
  "metrics": [
    {
      "name": "api.api_slow",
      "value": 1250.5,
      "flag_state": true
    }
  ],
  "health_status": 1
}
```

See [API Reference](api/endpoints.md) for complete documentation.

## Configuration

The convertor requires a YAML configuration file with:
- `datasource`: TSDB connection details
- `server`: HTTP API binding (address, port)
- `metric_templates`: Query templates with operators and thresholds
- `flag_metrics`: Flag metric definitions per service
- `health_metrics`: Health expressions with weights

See [Configuration Overview](configuration/overview.md) for complete reference.

## Running Convertor

### Basic Usage

```bash
cloudmon-metrics-convertor --config config.yaml
```

### Docker

```bash
docker run -v /path/to/config.yaml:/config.yaml \
  cloudmon-metrics:latest \
  cloudmon-metrics-convertor --config /config.yaml
```

### Environment Variables

Override configuration with `MP_` prefixed environment variables:

```bash
MP_SERVER__PORT=3005 \
MP_DATASOURCE__URL=https://graphite.example.com \
cloudmon-metrics-convertor --config config.yaml
```

## Performance Considerations

### Query Optimization
- **Template reuse**: Define templates once, reference multiple times
- **Variable substitution**: Minimize template variations
- **Time range**: Limit query windows to necessary intervals

### Caching (Future Enhancement)
- Cache TSDB responses for short TTL (e.g., 30s)
- Reduces load on TSDB for frequent queries

### Concurrency
- Convertor handles multiple concurrent requests
- Each request queries TSDB independently
- No shared state between requests

## Troubleshooting

### Common Issues

**"Service not found"**
- Cause: Service not defined in `health_metrics` section
- Solution: Add service configuration

**"Template not found"**
- Cause: Flag metric references undefined template
- Solution: Add template to `metric_templates` section

**"TSDB timeout"**
- Cause: TSDB query exceeds timeout (default: 10s)
- Solution: Increase `datasource.timeout` or optimize query

See [Troubleshooting Guide](guides/troubleshooting.md) for more solutions.

## Related Documentation

- [Architecture Overview](architecture/overview.md)
- [Data Flow](architecture/data-flow.md)
- [API Reference](api/endpoints.md)
- [Configuration Reference](configuration/overview.md)
- [Reporter Component](reporter.md)
