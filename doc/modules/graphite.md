# Graphite Module

The Graphite module (`src/graphite.rs`) provides a Graphite TSDB-compatible API interface, enabling integration with Grafana and other Graphite-compatible tools.

## Overview

This module implements:
- Graphite render API for time series data
- Metrics discovery API (`/metrics/find`)
- Grafana-compatible endpoints

## Key Types

### GraphiteData

Response structure from Graphite queries:

```rust
#[derive(Deserialize, Serialize, Debug)]
pub struct GraphiteData {
    /// Metric target name
    pub target: String,
    /// Array of (value, timestamp) tuples
    pub datapoints: Vec<(Option<f32>, u32)>,
}
```

### MetricsQuery

Query parameters for metrics discovery:

```rust
#[derive(Debug, Deserialize)]
pub struct MetricsQuery {
    /// Query pattern (e.g., "flag.*", "health.env1.*")
    pub query: String,
    /// Optional start time
    pub from: Option<String>,
    /// Optional end time
    pub until: Option<String>,
}
```

### Metric

Metric metadata for discovery responses:

```rust
#[derive(Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Metric {
    #[serde(rename(serialize = "allowChildren"))]
    pub allow_children: u8,
    pub expandable: u8,
    pub leaf: u8,
    pub id: String,
    pub text: String,
}
```

### RenderRequest

Parameters for render API:

```rust
#[derive(Default, Debug, Deserialize)]
pub struct RenderRequest {
    /// Target metric path
    pub target: Option<String>,
    /// Start time
    pub from: Option<String>,
    /// End time
    pub until: Option<String>,
    /// Maximum data points to return
    #[serde(rename(deserialize = "maxDataPoints"))]
    pub max_data_points: Option<u16>,
}
```

## Routes

```rust
pub fn get_graphite_routes() -> Router<AppState> {
    Router::new()
        .route("/functions", get(handler_functions))
        .route("/metrics/find", get(handler_metrics_find_get).post(handler_metrics_find_post))
        .route("/render", get(handler_render).post(handler_render))
        .route("/tags/autoComplete/tags", get(handler_tags))
}
```

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/functions` | `handler_functions` | Returns empty object (Grafana compatibility) |
| GET/POST | `/metrics/find` | `handler_metrics_find_*` | Discover available metrics |
| GET/POST | `/render` | `handler_render` | Render time series data |
| GET | `/tags/autoComplete/tags` | `handler_tags` | Returns empty array (Grafana compatibility) |

## Metric Discovery

### Virtual Metric Hierarchy

The module exposes a virtual metric hierarchy:

```
├── flag
│   └── {environment}
│       └── {service}
│           └── {metric_name}
└── health
    └── {environment}
        └── {service_name}
```

### find_metrics() Function

```rust
pub fn find_metrics(find_request: MetricsQuery, state: AppState) -> Vec<Metric>
```

Query patterns:
- `*` - Returns top-level: `["flag", "health"]`
- `flag.*` or `health.*` - Returns environments
- `flag.{env}.*` - Returns services
- `flag.{env}.{service}.*` - Returns metric names
- `health.{env}.*` - Returns health metric names

## Render API

### handler_render

Handles both GET and POST requests for time series data.

**Flag Metrics** (`flag.{env}.{service}.{metric}`):
1. Looks up metric configuration
2. Queries upstream Graphite with resolved query
3. Converts raw values to binary flags (0/1) based on threshold

**Health Metrics** (`health.{env}.{service}`):
1. Calls `get_service_health()` from common module
2. Returns aggregated health scores

### Response Format

```json
[
  {
    "target": "service.metric-name",
    "datapoints": [[1.0, 1704067200], [0.0, 1704070800]]
  }
]
```

## Graphite Client

### get_graphite_data()

Core function for querying upstream Graphite:

```rust
pub async fn get_graphite_data(
    client: &reqwest::Client,
    url: &str,
    targets: &HashMap<String, String>,  // alias -> query
    from: Option<DateTime<FixedOffset>>,
    from_raw: Option<String>,
    to: Option<DateTime<FixedOffset>>,
    to_raw: Option<String>,
    max_data_points: u16,
) -> Result<Vec<GraphiteData>, CloudMonError>
```

**Query Construction:**
```rust
let query_params: Vec<(_, String)> = [
    ("format", "json".to_string()),
    ("maxDataPoints", max_data_points.to_string()),
].into();
// Add from, until, and target parameters
```

**Query Aliasing:**
```rust
fn alias_graphite_query(query: &str, alias: &str) -> String {
    format!("alias({},'{}')", query, alias)
}
```

This wraps each query with Graphite's `alias()` function to preserve the logical metric name in responses.

## Request Extraction

### JsonOrForm Extractor

Custom Axum extractor that accepts both JSON and form-encoded bodies:

```rust
#[derive(Default, Debug)]
pub struct JsonOrForm<T>(T);

#[async_trait]
impl<S, B, T> FromRequest<S, B> for JsonOrForm<T>
where
    // ... constraints
{
    async fn from_request(req: Request<B>, _state: &S) -> Result<Self, Self::Rejection> {
        let content_type = req.headers().get(CONTENT_TYPE);
        
        if content_type.starts_with("application/json") {
            // Extract as JSON
        }
        if content_type.starts_with("application/x-www-form-urlencoded") {
            // Extract as Form
        }
        
        Err(StatusCode::UNSUPPORTED_MEDIA_TYPE)
    }
}
```

This enables compatibility with various Graphite clients (Grafana uses form encoding).

## Integration Flow

```
Grafana/Client
      │
      ▼
┌─────────────────┐
│ /metrics/find   │──► find_metrics() ──► AppState.flag_metrics
└─────────────────┘                       AppState.health_metrics
                                          AppState.environments
      │
      ▼
┌─────────────────┐
│    /render      │──► handler_render()
└─────────────────┘
      │
      ├──► Flag: get_graphite_data() ──► Upstream Graphite
      │                                         │
      │                                         ▼
      │                                  Convert to 0/1 flags
      │
      └──► Health: get_service_health() ──► Aggregate expressions
```

## Error Handling

- Returns empty array `[]` for unrecognized metric paths
- Returns `CloudMonError::GraphiteError` for upstream failures
- Logs warnings for unknown targets in responses
