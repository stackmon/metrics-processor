# TSDB Interface

This document defines the interface requirements for Time Series Database (TSDB) backends in the metrics-processor.

## Overview

The metrics-processor retrieves time series data from external TSDBs to compute service health metrics and flag states. Any TSDB backend must implement the query execution and response parsing interfaces defined below.

## Core Interface Requirements

### Query Execution Interface

TSDB backends must implement a data fetching function with the following signature pattern:

```rust
pub async fn get_tsdb_data(
    client: &reqwest::Client,
    url: &str,
    targets: &HashMap<String, String>,  // alias -> query mapping
    from: Option<DateTime<FixedOffset>>,
    from_raw: Option<String>,
    to: Option<DateTime<FixedOffset>>,
    to_raw: Option<String>,
    max_data_points: u16,
) -> Result<Vec<TsdbData>, CloudMonError>
```

#### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `client` | `&reqwest::Client` | Shared HTTP client from `AppState` |
| `url` | `&str` | Base URL of the TSDB instance |
| `targets` | `&HashMap<String, String>` | Map of alias names to query expressions |
| `from` | `Option<DateTime<FixedOffset>>` | Start time (parsed datetime) |
| `from_raw` | `Option<String>` | Start time (raw string, e.g., "now-1h") |
| `to` | `Option<DateTime<FixedOffset>>` | End time (parsed datetime) |
| `to_raw` | `Option<String>` | End time (raw string) |
| `max_data_points` | `u16` | Maximum data points to return |

### Response Data Structure

All TSDB backends must return data in a normalized format compatible with the processor:

```rust
pub struct TsdbData {
    /// Target/metric name (used as lookup key)
    pub target: String,
    /// Array of (value, timestamp) tuples
    pub datapoints: Vec<(Option<f32>, u32)>,
}
```

#### Data Point Format

- **Value**: `Option<f32>` - The metric value, `None` for null/missing data
- **Timestamp**: `u32` - Unix timestamp in seconds

### Error Handling Patterns

Backends must return `CloudMonError` for failures:

```rust
pub enum CloudMonError {
    ServiceNotSupported,
    EnvNotSupported,
    ExpressionError,
    GraphiteError,  // Rename to generic TsdbError for new backends
}
```

#### Error Scenarios

| Scenario | Error Type | Handling |
|----------|------------|----------|
| HTTP client errors | `CloudMonError::GraphiteError` | Log and return error |
| 4xx response codes | `CloudMonError::GraphiteError` | Log response body, return error |
| JSON parse failures | `CloudMonError::GraphiteError` | Return error |
| Connection timeout | `CloudMonError::GraphiteError` | Retry logic in client |

### Response Parsing Expectations

1. **Parse JSON response** into the standard `TsdbData` structure
2. **Preserve target names** exactly as aliased in the query
3. **Handle null values** by setting `None` in the datapoints
4. **Maintain timestamp ordering** (typically ascending)

## Configuration Requirements

### Datasource Configuration

The configuration must include TSDB connection details:

```yaml
datasource:
  url: 'https://graphite.example.com'
  timeout: 10  # seconds, optional (default: 10)
```

### Configuration Struct

```rust
#[derive(Clone, Debug, Deserialize)]
pub struct Datasource {
    pub url: String,
    #[serde(default = "default_timeout")]
    pub timeout: u16,
}
```

### Future Extension: TSDB Type Selection

For multi-backend support, extend configuration:

```yaml
datasource:
  type: graphite  # or prometheus, influxdb
  url: 'https://tsdb.example.com'
  timeout: 10
```

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatasourceType {
    Graphite,
    Prometheus,
    InfluxDB,
}
```

## Integration Points

### AppState Integration

The TSDB client is accessed via `AppState`:

```rust
pub struct AppState {
    pub config: Config,
    pub req_client: reqwest::Client,  // Shared HTTP client
    // ... other fields
}
```

### Usage in Common Module

The `get_service_health` function in `common.rs` calls the TSDB:

```rust
let raw_data: Vec<graphite::GraphiteData> = graphite::get_graphite_data(
    &state.req_client,
    &state.config.datasource.url.as_str(),
    &graphite_targets,
    from_datetime,
    from_raw,
    to_datetime,
    to_raw,
    max_data_points,
).await?;
```

## Implementation Checklist

When implementing a new TSDB backend:

- [ ] Implement async data fetching function
- [ ] Return data in `TsdbData` format (target + datapoints)
- [ ] Handle all HTTP error cases
- [ ] Parse TSDB-specific response format
- [ ] Support both raw string and parsed datetime parameters
- [ ] Implement query aliasing for target name preservation
- [ ] Add configuration options if needed
- [ ] Update `DatasourceType` enum
- [ ] Add integration tests with mocked responses

## See Also

- [Graphite Implementation](./graphite.md) - Reference implementation
- [Adding Backends](./adding-backends.md) - Step-by-step guide
