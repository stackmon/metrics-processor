# Cloudmon-metrics

CloudMon Metrics Processor converts raw time-series metrics from monitoring systems into simple, semaphore-like health indicators for service status visualization.

## What Problem Does It Solve?

When monitoring cloud infrastructure, you often have dozens of different metric types for each service:
- **Latency metrics**: Response times, processing durations
- **Error metrics**: HTTP 5xx rates, exception counts
- **Throughput metrics**: Requests per second, queue depths
- **Availability metrics**: Service uptime, health check results

Visualizing overall service health from these disparate metrics is challenging. Different monitoring dashboards show different views, and it's hard to get a unified "at-a-glance" status. Operations teams need something like a traffic light to quickly understand: **Is this service healthy, degraded, or down?**

## The Solution: Convertor + Reporter

CloudMon Metrics Processor provides a two-component architecture:

### 1. Convertor: Metric Evaluation Engine

The **convertor** component:
- Queries raw metrics from your Time-Series Database (TSDB)
- Evaluates **flag metrics** (binary indicators: is latency high? are errors elevated?)
- Combines flags using **health expressions** (boolean logic: `api_slow || api_error_rate_high`)
- Produces semaphore values: 
  - 🟢 **0 (Green)**: Service healthy
  - 🟡 **1 (Yellow)**: Service degraded
  - 🔴 **2 (Red)**: Service outage
- Exposes HTTP API for querying health status

### 2. Reporter: Status Dashboard Integration

The **reporter** component:
- Polls convertor API at regular intervals
- Sends health status updates to your status dashboard (e.g., Atlassian Statuspage)
- Handles authentication and dashboard-specific protocols

## Architecture Overview

```
┌─────────────┐     ┌────────────────┐     ┌──────────────┐     ┌─────────────────┐
│   Graphite  │────▶│   Convertor    │────▶│   Reporter   │────▶│ Status Dashboard│
│   (TSDB)    │     │  (evaluates)   │     │  (notifies)  │     │   (displays)    │
└─────────────┘     └────────────────┘     └──────────────┘     └─────────────────┘
      Raw metrics         Flag metrics          Health status          Semaphore UI
```

## Key Concepts

### Flag Metrics
Binary indicators derived from raw metrics using comparison operations:
- Query TSDB: `stats.timers.api.production.mean`
- Compare: `> 500` (ms)
- Result: `true` (flag raised) or `false` (flag lowered)

### Health Metrics
Composite health status combining multiple flag metrics:
- Expression: `api_slow || api_error_rate_high`
- Weight: 
  - If expression matches: return weight (0=healthy, 1=degraded, 2=outage)
  - Multiple expressions: use highest matching weight

### Configuration-Driven
All logic is defined in YAML configuration - no code changes needed to:
- Add new services
- Define custom metric thresholds
- Create complex health logic

## Quick Links

- **[Getting Started](getting-started/quickstart.md)**: Set up your environment in 30 minutes
- **[Architecture](architecture/overview.md)**: Detailed system design and data flow
- **[Configuration](configuration/overview.md)**: Complete configuration reference
- **[API Reference](api/endpoints.md)**: HTTP API documentation
- **[Integration Guide](integration/adding-backends.md)**: Add support for new TSDB backends

## Use Cases

### 1. Multi-Service Health Dashboard
Monitor 10+ microservices with different metric types, unified into single status board.

### 2. Smart Alerting
Combine multiple signals (latency + errors + throughput) to reduce false positives.

### 3. Customer-Facing Status Page
Convert internal metrics to simple health indicators for public status pages.

### 4. SLA Reporting
Track service health over time with standardized semaphore values (0/1/2).

## Technology Stack

- **Language**: Rust (edition 2021)
- **HTTP Framework**: Axum 0.6
- **Runtime**: Tokio (async)
- **TSDB Support**: Graphite (extensible to Prometheus, InfluxDB, etc.)
- **Configuration**: YAML with environment variable overrides

## Project Structure

```
metrics-processor/
├── src/
│   ├── bin/
│   │   ├── convertor.rs    # Convertor binary entry point
│   │   └── reporter.rs     # Reporter binary entry point
│   ├── api.rs              # HTTP API module
│   ├── config.rs           # Configuration parsing
│   ├── types.rs            # Core data structures
│   ├── graphite.rs         # Graphite TSDB client
│   └── common.rs           # Shared utilities
├── doc/                    # This documentation
├── tests/                  # Integration tests
└── openapi-schema.yaml     # API specification
```

## Documentation Organization

This documentation is organized for two audiences:

### For Human Developers
- [**Getting Started**](getting-started/quickstart.md): Onboarding guide
- [**Architecture**](architecture/overview.md): System design
- [**Operational Guides**](guides/troubleshooting.md): Troubleshooting, deployment

### For AI/IDE Tools
- [**Schemas**](schemas/README.md): Machine-readable configuration schemas
- [**Patterns**](schemas/patterns.json): Code conventions for code generation
- [**OpenAPI**](api/endpoints.md): API contracts

## Components

- [**Convertor**](convertor.md): Metric evaluation and HTTP API server
- [**Reporter**](reporter.md): Status dashboard notification client

## Contributing

See [Development Guide](getting-started/development.md) for:
- Development workflow
- Testing practices
- Code quality standards
- Debugging techniques

## Version

Current version: **0.2.0**

---

**Next Step**: New to the project? Start with the [Quickstart Guide](getting-started/quickstart.md)

