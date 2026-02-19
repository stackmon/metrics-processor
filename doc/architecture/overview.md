# Architecture Overview

This document describes the system architecture of CloudMon Metrics Processor, a Rust application that converts raw time-series metrics into simple health indicators.

## System Overview

CloudMon Metrics Processor transforms complex monitoring data into actionable health statuses through a two-component architecture:

```
┌─────────────┐     ┌────────────────┐     ┌──────────────┐     ┌─────────────────┐
│   Graphite  │────▶│   Convertor    │────▶│   Reporter   │────▶│ Status Dashboard│
│   (TSDB)    │     │  (evaluates)   │     │  (notifies)  │     │   (displays)    │
└─────────────┘     └────────────────┘     └──────────────┘     └─────────────────┘
      Raw metrics         Flag metrics          Health status          Semaphore UI
```

## Component Relationships

### 1. Convertor (Core Engine)

**Location**: `src/bin/convertor.rs`

The Convertor is the primary component responsible for:

- **Metric Evaluation**: Queries raw metrics from TSDB (Graphite) and evaluates them against configured thresholds
- **Flag Computation**: Converts raw metric values into binary flags (raised/lowered)
- **Health Calculation**: Combines flag states using boolean expressions to produce semaphore values
- **HTTP API**: Exposes endpoints for health queries and Grafana-compatible data access

**Dependencies**:
- `src/api/v1.rs` - REST API handlers for health queries
- `src/graphite.rs` - TSDB communication and Grafana-compatible endpoints
- `src/common.rs` - Shared metric evaluation logic
- `src/types.rs` - Core domain types (AppState, FlagMetric, HealthMetric)
- `src/config.rs` - Configuration parsing and validation

### 2. Reporter (Notification Client)

**Location**: `src/bin/reporter.rs`

The Reporter is a background service that:

- **Polls Convertor**: Queries the Convertor API at configurable intervals (default: 60s)
- **Detects Issues**: Identifies when health status indicates degradation or outage
- **Sends Notifications**: Posts status updates to external dashboards (e.g., Atlassian Statuspage)
- **Handles Authentication**: Manages JWT tokens for secure dashboard communication

**Dependencies**:
- Convertor API (localhost HTTP calls)
- Status Dashboard API (external HTTP POST)
- `src/config.rs` - Shared configuration parsing

### 3. TSDB (Graphite)

**Role**: External data source providing raw time-series metrics

- **Query Interface**: Standard Graphite render API
- **Data Format**: JSON with target name and datapoints array
- **Integration**: Via `src/graphite.rs` module

### 4. Status Dashboard

**Role**: External consumer of health status

- **Supported**: CloudMon Status Dashboard (custom API)
- **Protocol**: REST API with JWT authentication
- **Data Format**: Component status with name, impact level, and attributes

## Key Design Decisions

### 1. Configuration-Driven Logic

All metric evaluation logic is defined in YAML configuration rather than code:

```yaml
metric_templates:
  api_slow:
    query: "stats.timers.api.$environment.$service.mean"
    op: "gt"
    threshold: 500

health_metrics:
  my_service:
    expressions:
      - expression: "api_slow || error_rate_high"
        weight: 1
```

**Rationale**: Operations teams can add services, adjust thresholds, and modify health logic without code changes or deployments.

### 2. Separation of Concerns

Convertor and Reporter are separate binaries that communicate via HTTP:

**Rationale**:
- **Scalability**: Multiple Reporters can poll a single Convertor
- **Resilience**: Reporter failure doesn't affect Convertor
- **Flexibility**: Different polling intervals per Reporter instance
- **Testing**: Each component can be tested independently

### 3. Grafana Compatibility

The Convertor implements Grafana-compatible endpoints (`/metrics/find`, `/render`):

**Rationale**: Enables direct visualization of flag and health metrics in Grafana dashboards alongside raw TSDB data.

### 4. Template-Based Queries

Flag metrics use templates with variable substitution:

```yaml
query: "stats.timers.api.$environment.$service.mean"
```

**Rationale**: Reduces configuration duplication when the same metric pattern applies to multiple services/environments.

### 5. Weighted Expressions

Health expressions include weights (0=healthy, 1=degraded, 2=outage):

```yaml
expressions:
  - expression: "api_slow"
    weight: 1  # degraded
  - expression: "api_down"
    weight: 2  # outage
```

**Rationale**: Different conditions can trigger different severity levels. The highest matching weight determines final status.

## Technology Stack Rationale

### Rust (Edition 2021)

**Why Rust?**
- **Performance**: Low latency for real-time metric evaluation
- **Memory Safety**: No garbage collection pauses, predictable resource usage
- **Reliability**: Strong type system catches errors at compile time
- **Ecosystem**: Excellent async support via Tokio

### Axum 0.6 (HTTP Framework)

**Why Axum?**
- **Tokio Native**: First-class async/await integration
- **Type-Safe Routing**: Compile-time route verification
- **Middleware**: Tower ecosystem for tracing, request IDs
- **Performance**: Minimal overhead, competitive benchmarks

### Tokio (Async Runtime)

**Why Tokio?**
- **Industry Standard**: Most widely used async runtime in Rust
- **Full-Featured**: Timer, signal handling, task spawning
- **Well-Documented**: Extensive guides and examples

### evalexpr (Expression Evaluation)

**Why evalexpr?**
- **Safe Evaluation**: No arbitrary code execution
- **Boolean Support**: Native `||`, `&&` operators
- **Variable Context**: Dynamic variable binding
- **Lightweight**: Minimal dependencies

### reqwest (HTTP Client)

**Why reqwest?**
- **Async Support**: Tokio-compatible
- **TLS/SSL**: Built-in HTTPS support
- **Connection Pooling**: Efficient for repeated TSDB queries
- **Timeout Handling**: Configurable per-request timeouts

## Module Structure

```
src/
├── lib.rs              # Library crate root (re-exports modules)
├── api.rs              # API module declaration
├── api/
│   └── v1.rs           # REST API v1 handlers (/health, /info)
├── config.rs           # Configuration parsing (YAML + env vars)
├── types.rs            # Domain types and AppState
├── graphite.rs         # Graphite TSDB client + Grafana compat
├── common.rs           # Shared utilities (flag evaluation)
└── bin/
    ├── convertor.rs    # Convertor binary entry point
    └── reporter.rs     # Reporter binary entry point
```

### Module Responsibilities

| Module | Responsibility |
|--------|----------------|
| `config` | Parse YAML config, merge env vars, validate structure |
| `types` | Define FlagMetric, HealthMetric, AppState; process templates |
| `graphite` | Query TSDB, implement Grafana endpoints |
| `common` | Evaluate flag states, compute service health |
| `api/v1` | Handle /health requests, format responses |
| `convertor` | Initialize app, configure routes, start server |
| `reporter` | Poll convertor, post to status dashboard |

## Deployment Architecture

### Standalone Deployment

```
┌─────────────────────────────────────────────┐
│                  Host/Container             │
│  ┌─────────────────────────────────────┐    │
│  │  cloudmon-metrics-convertor         │    │
│  │  - Listens on :3000                 │    │
│  │  - Reads config.yaml                │    │
│  │  - Queries Graphite                 │    │
│  └─────────────────────────────────────┘    │
│  ┌─────────────────────────────────────┐    │
│  │  cloudmon-metrics-reporter          │    │
│  │  - Polls localhost:3000             │    │
│  │  - Posts to Status Dashboard        │    │
│  └─────────────────────────────────────┘    │
└─────────────────────────────────────────────┘
```

### Kubernetes Deployment

```
┌─────────────────────────────────────────────────────────────────┐
│                        Kubernetes Cluster                       │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  Deployment: convertor                                  │    │
│  │  ┌─────────────────┐  ┌─────────────────┐               │    │
│  │  │   Pod (replica) │  │   Pod (replica) │  ...          │    │
│  │  │   convertor:3000│  │   convertor:3000│               │    │
│  │  └─────────────────┘  └─────────────────┘               │    │
│  │           ▲                    ▲                        │    │
│  │           └──────────┬─────────┘                        │    │
│  │                      │                                  │    │
│  │           ┌──────────▼──────────┐                       │    │
│  │           │    Service: convertor                       │    │
│  │           │    ClusterIP:3000   │                       │    │
│  │           └─────────────────────┘                       │    │
│  └─────────────────────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  Deployment: reporter                                   │    │
│  │  ┌─────────────────┐                                    │    │
│  │  │   Pod           │ ──polls──▶ Service:convertor       │    │
│  │  │   reporter      │ ──posts──▶ Status Dashboard        │    │
│  │  └─────────────────┘                                    │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
```

### Configuration Sources

1. **config.yaml**: Main configuration file
2. **conf.d/*.yaml**: Additional configuration fragments (merged)
3. **Environment Variables**: Override any config value with `MP_` prefix

```bash
# Environment variable example
MP_STATUS_DASHBOARD__JWT_SECRET=my-jwt-secret
# Translates to: status_dashboard.jwt_secret = "my-jwt-secret"
```

## Security Considerations

### Authentication

- **Status Dashboard**: JWT tokens signed with HMAC-SHA256
- **Internal APIs**: No authentication (expected behind firewall)

### Network Security

- **Convertor**: Should be internal-only (not exposed to internet)
- **Reporter → Dashboard**: HTTPS recommended for external communication

### Configuration Secrets

- Sensitive values (JWT secrets) should be injected via environment variables
- Config files should not contain production secrets

## Performance Characteristics

### Convertor

- **Stateless**: No persistence, all state from config + TSDB
- **Low Memory**: Typically <100MB for moderate workloads
- **Async I/O**: Non-blocking TSDB queries

### Reporter

- **Polling Interval**: 60 seconds (configurable)
- **Batch Processing**: Processes all environments/services per cycle
- **Retry Logic**: Logs errors but continues operation

## Related Documentation

- [Data Flow](data-flow.md): Detailed processing pipeline
- [Diagrams](diagrams.md): Visual architecture representations
- [Configuration](../configuration/overview.md): Full config reference
- [API Reference](../api/endpoints.md): HTTP endpoint documentation
