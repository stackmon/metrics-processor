# Graphite Implementation

This document describes the existing Graphite TSDB integration, serving as a reference implementation for other backends.

## Overview

The Graphite module (`src/graphite.rs`) provides:
1. Data fetching from Graphite's render API
2. Grafana-compatible API endpoints for metrics discovery
3. Response parsing and normalization

## GraphiteData Structure

The core data structure returned by Graphite queries:

```rust
#[derive(Deserialize, Serialize, Debug)]
pub struct GraphiteData {
    /// Target name (metric identifier)
    pub target: String,
    /// Array of (value, timestamp) tuples
    pub datapoints: Vec<(Option<f32>, u32)>,
}
```

**Example JSON Response:**
```json
[
  {
    "target": "service.metric-1",
    "datapoints": [
      [1.5, 1704067200],
      [2.0, 1704067260],
      [null, 1704067320]
    ]
  }
]
```

## Query Format and URL Construction

### Render API Endpoint

Queries are sent to Graphite's `/render` endpoint:

```
GET {base_url}/render?format=json&target=...&from=...&until=...
```

### Query Parameters

| Parameter | Description | Example |
|-----------|-------------|---------|
| `format` | Response format | `json` |
| `target` | Query expression (multiple allowed) | `alias(query,'name')` |
| `from` | Start time | `00:00_20220101` or `now-1h` |
| `until` | End time | `00:00_20220201` or `now` |
| `maxDataPoints` | Max points to return | `100` |

### Query Aliasing

The implementation wraps queries with Graphite's `alias()` function to preserve metric names:

```rust
fn alias_graphite_query(query: &str, alias: &str) -> String {
    format!("alias({},'{}')", query, alias)
}
```

This ensures the response `target` field matches the expected metric name regardless of the actual query expression.

### Time Format

Graphite uses a specific datetime format:

```rust
// Parsed datetime to Graphite format
xfrom.format("%H:%M_%Y%m%d").to_string()  // "00:00_20220101"
```

Raw strings (like `now-1h`) are passed through unchanged.

## Core Data Fetching Function

### Function Signature

```rust
pub async fn get_graphite_data(
    client: &reqwest::Client,
    url: &str,
    targets: &HashMap<String, String>,
    from: Option<DateTime<FixedOffset>>,
    from_raw: Option<String>,
    to: Option<DateTime<FixedOffset>>,
    to_raw: Option<String>,
    max_data_points: u16,
) -> Result<Vec<GraphiteData>, CloudMonError>
```

### Implementation Details

```rust
pub async fn get_graphite_data(
    client: &reqwest::Client,
    url: &str,
    targets: &HashMap<String, String>,
    from: Option<DateTime<FixedOffset>>,
    from_raw: Option<String>,
    to: Option<DateTime<FixedOffset>>,
    to_raw: Option<String>,
    max_data_points: u16,
) -> Result<Vec<GraphiteData>, CloudMonError> {
    // Build query parameters
    let mut query_params: Vec<(_, String)> = [
        ("format", "json".to_string()),
        ("maxDataPoints", max_data_points.to_string()),
    ].into();
    
    // Add time range (prefer parsed datetime, fallback to raw string)
    if let Some(xfrom) = from {
        query_params.push(("from", xfrom.format("%H:%M_%Y%m%d").to_string()));
    } else if let Some(xfrom) = from_raw {
        query_params.push(("from", xfrom.clone()));
    }
    if let Some(xto) = to {
        query_params.push(("until", xto.format("%H:%M_%Y%m%d").to_string()));
    } else if let Some(xto) = to_raw {
        query_params.push(("until", xto.clone()));
    }
    
    // Add targets with aliasing
    query_params.extend(
        targets.iter().map(|x| ("target", alias_graphite_query(x.1, x.0))),
    );
    
    // Execute request
    let res = client
        .get(format!("{}/render", url))
        .query(&query_params)
        .send()
        .await;
    
    // Handle response
    match res {
        Ok(rsp) => {
            if rsp.status().is_client_error() {
                tracing::error!("Error: {:?}", rsp.text().await);
                return Err(CloudMonError::GraphiteError);
            }
            match rsp.json().await {
                Ok(dt) => Ok(dt),
                Err(_) => Err(CloudMonError::GraphiteError),
            }
        }
        Err(_) => Err(CloudMonError::GraphiteError),
    }
}
```

## Response Parsing

Graphite's JSON response is automatically deserialized using serde:

```rust
#[derive(Deserialize, Serialize, Debug)]
pub struct GraphiteData {
    pub target: String,
    pub datapoints: Vec<(Option<f32>, u32)>,
}
```

**Key Points:**
- `target`: The alias name set in the query
- `datapoints`: Array of `[value, timestamp]` pairs
- `null` values in JSON become `None` in Rust

## Configuration

### Datasource Config

```yaml
datasource:
  url: 'https://graphite.example.com'
  timeout: 10
```

### Config Struct (from `config.rs`)

```rust
#[derive(Clone, Debug, Deserialize)]
pub struct Datasource {
    pub url: String,
    #[serde(default = "default_timeout")]
    pub timeout: u16,
}
```

## Grafana-Compatible API Routes

The module also exposes Grafana datasource API endpoints:

```rust
pub fn get_graphite_routes() -> Router<AppState> {
    Router::new()
        .route("/functions", get(handler_functions))
        .route("/metrics/find", get(handler_metrics_find_get).post(handler_metrics_find_post))
        .route("/render", get(handler_render).post(handler_render))
        .route("/tags/autoComplete/tags", get(handler_tags))
}
```

### Endpoint Purposes

| Endpoint | Purpose |
|----------|---------|
| `/functions` | List supported Graphite functions |
| `/metrics/find` | Discover available metrics |
| `/render` | Execute queries and return data |
| `/tags/autoComplete/tags` | Tag autocomplete (returns empty) |

## Testing

### Unit Test Example

```rust
#[test]
fn test_get_graphite_data() {
    let mut server = mockito::Server::new();
    let mock = server
        .mock("GET", "/render")
        .expect(1)
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("target".into(), "alias(query,'alias')".into()),
            Matcher::UrlEncoded("from".into(), "00:00_20220101".into()),
            Matcher::UrlEncoded("until".into(), "00:00_20220201".into()),
            Matcher::UrlEncoded("maxDataPoints".into(), "15".into()),
        ]))
        .create();
    
    let mut targets: HashMap<String, String> = HashMap::new();
    targets.insert("alias".to_string(), "query".to_string());
    
    let res = aw!(graphite::get_graphite_data(
        &req_client,
        format!("{}", server.url()).as_str(),
        &targets,
        from,
        None,
        to,
        None,
        15,
    ));
    mock.assert();
}
```

## Integration with Processor

### Flag Metrics Processing

In `handler_render`, raw Graphite data is converted to flag states:

```rust
match get_graphite_data(...).await {
    Ok(mut raw_data) => {
        for data_element in raw_data.iter_mut() {
            match state.flag_metrics.get(&data_element.target) {
                Some(metric_cfg) => {
                    let metric = metric_cfg.get(environment).unwrap();
                    for (val, _) in data_element.datapoints.iter_mut() {
                        *val = if get_metric_flag_state(val, metric) {
                            Some(1.0)  // Flag is true
                        } else {
                            Some(0.0)  // Flag is false
                        };
                    }
                }
                None => { /* unknown target */ }
            }
        }
    }
    Err(_) => { /* handle error */ }
}
```

### Health Metrics Processing

In `common.rs`, Graphite data feeds the health calculation:

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

// Process into metrics_map for health evaluation
for data_element in raw_data.iter() {
    match state.flag_metrics.get(&data_element.target) {
        Some(metric_cfg) => {
            let metric = metric_cfg.get(environment).unwrap();
            for (val, ts) in data_element.datapoints.iter() {
                if let Some(_) = val {
                    metrics_map.entry(*ts).or_insert(HashMap::new())
                        .insert(data_element.target.clone(), get_metric_flag_state(val, metric));
                }
            }
        }
        None => { /* unknown target */ }
    }
}
```

## See Also

- [TSDB Interface](./interface.md) - Interface requirements
- [Adding Backends](./adding-backends.md) - How to add new backends
