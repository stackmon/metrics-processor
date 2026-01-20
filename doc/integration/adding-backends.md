# Adding New TSDB Backends

This guide walks through implementing a new Time Series Database backend for the metrics-processor.

## Overview

Adding a new backend involves:
1. Creating a new module with the data fetching function
2. Defining the response data structure
3. Implementing query translation
4. Integrating with the configuration
5. Updating the common module to use the new backend
6. Adding tests

## Step 1: Create the Backend Module

Create a new file `src/{backend_name}.rs`:

```rust
//! {BackendName} communication module
//!
//! Module for communication with {BackendName} TSDB

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::types::CloudMonError;

/// Response data structure matching the interface requirements
#[derive(Deserialize, Serialize, Debug)]
pub struct {BackendName}Data {
    pub target: String,
    pub datapoints: Vec<(Option<f32>, u32)>,
}
```

## Step 2: Implement the Data Fetching Function

### Function Template

```rust
pub async fn get_{backend}_data(
    client: &reqwest::Client,
    url: &str,
    targets: &HashMap<String, String>,
    from: Option<DateTime<FixedOffset>>,
    from_raw: Option<String>,
    to: Option<DateTime<FixedOffset>>,
    to_raw: Option<String>,
    max_data_points: u16,
) -> Result<Vec<{BackendName}Data>, CloudMonError> {
    // Implementation here
}
```

## Prometheus Implementation Example

### Response Structure

Prometheus returns data in a different format that needs translation:

```rust
// src/prometheus.rs

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::types::CloudMonError;

/// Normalized output format (matches GraphiteData)
#[derive(Deserialize, Serialize, Debug)]
pub struct PrometheusData {
    pub target: String,
    pub datapoints: Vec<(Option<f32>, u32)>,
}

/// Prometheus API response structure
#[derive(Deserialize, Debug)]
struct PrometheusResponse {
    status: String,
    data: PrometheusResponseData,
}

#[derive(Deserialize, Debug)]
struct PrometheusResponseData {
    #[serde(rename = "resultType")]
    result_type: String,
    result: Vec<PrometheusResult>,
}

#[derive(Deserialize, Debug)]
struct PrometheusResult {
    metric: HashMap<String, String>,
    values: Vec<(f64, String)>,  // [timestamp, value_string]
}
```

### Query Construction

```rust
fn build_prometheus_query(query: &str, alias: &str) -> String {
    // Prometheus doesn't have native aliasing, store mapping for response processing
    query.to_string()
}

fn format_prometheus_time(dt: &DateTime<FixedOffset>) -> String {
    dt.timestamp().to_string()
}

pub async fn get_prometheus_data(
    client: &reqwest::Client,
    url: &str,
    targets: &HashMap<String, String>,  // alias -> query
    from: Option<DateTime<FixedOffset>>,
    from_raw: Option<String>,
    to: Option<DateTime<FixedOffset>>,
    to_raw: Option<String>,
    max_data_points: u16,
) -> Result<Vec<PrometheusData>, CloudMonError> {
    let mut results: Vec<PrometheusData> = Vec::new();
    
    // Calculate step based on time range and max_data_points
    let step = calculate_step(from.as_ref(), to.as_ref(), max_data_points);
    
    // Prometheus requires individual queries (or use query_range with multiple queries)
    for (alias, query) in targets.iter() {
        let mut query_params: Vec<(&str, String)> = vec![
            ("query", query.clone()),
            ("step", step.to_string()),
        ];
        
        // Add time range
        if let Some(ref xfrom) = from {
            query_params.push(("start", format_prometheus_time(xfrom)));
        } else if let Some(ref xfrom) = from_raw {
            query_params.push(("start", xfrom.clone()));
        }
        
        if let Some(ref xto) = to {
            query_params.push(("end", format_prometheus_time(xto)));
        } else if let Some(ref xto) = to_raw {
            query_params.push(("end", xto.clone()));
        }
        
        let res = client
            .get(format!("{}/api/v1/query_range", url))
            .query(&query_params)
            .send()
            .await;
        
        match res {
            Ok(rsp) => {
                if !rsp.status().is_success() {
                    tracing::error!("Prometheus error: {:?}", rsp.text().await);
                    return Err(CloudMonError::GraphiteError);  // TODO: Add PrometheusError
                }
                
                let prom_response: PrometheusResponse = match rsp.json().await {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::error!("Failed to parse Prometheus response: {:?}", e);
                        return Err(CloudMonError::GraphiteError);
                    }
                };
                
                // Convert Prometheus format to standard format
                if let Some(result) = prom_response.data.result.first() {
                    let datapoints: Vec<(Option<f32>, u32)> = result.values
                        .iter()
                        .map(|(ts, val)| {
                            let value = val.parse::<f32>().ok();
                            (*ts as u32, value)
                        })
                        .map(|(ts, val)| (val, ts))  // Swap to (value, timestamp)
                        .collect();
                    
                    results.push(PrometheusData {
                        target: alias.clone(),  // Use the alias as target
                        datapoints,
                    });
                }
            }
            Err(e) => {
                tracing::error!("Prometheus request failed: {:?}", e);
                return Err(CloudMonError::GraphiteError);
            }
        }
    }
    
    Ok(results)
}

fn calculate_step(
    from: Option<&DateTime<FixedOffset>>,
    to: Option<&DateTime<FixedOffset>>,
    max_data_points: u16,
) -> u64 {
    match (from, to) {
        (Some(f), Some(t)) => {
            let duration = (t.timestamp() - f.timestamp()) as u64;
            std::cmp::max(1, duration / max_data_points as u64)
        }
        _ => 60  // Default 1 minute step
    }
}
```

## InfluxDB Implementation Example

### Response Structure

```rust
// src/influxdb.rs

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::types::CloudMonError;

/// Normalized output format
#[derive(Deserialize, Serialize, Debug)]
pub struct InfluxDBData {
    pub target: String,
    pub datapoints: Vec<(Option<f32>, u32)>,
}

/// InfluxDB 2.x query response (Flux)
#[derive(Deserialize, Debug)]
struct InfluxDBResponse {
    results: Vec<InfluxDBResult>,
}

#[derive(Deserialize, Debug)]
struct InfluxDBResult {
    series: Option<Vec<InfluxDBSeries>>,
}

#[derive(Deserialize, Debug)]
struct InfluxDBSeries {
    name: String,
    columns: Vec<String>,
    values: Vec<Vec<serde_json::Value>>,
}
```

### Query Construction

```rust
fn build_influxdb_query(
    query: &str,
    bucket: &str,
    from: &str,
    to: &str,
) -> String {
    // Flux query format for InfluxDB 2.x
    format!(
        r#"from(bucket: "{}")
            |> range(start: {}, stop: {})
            |> filter(fn: (r) => {})
            |> aggregateWindow(every: 1m, fn: mean, createEmpty: false)"#,
        bucket, from, to, query
    )
}

pub async fn get_influxdb_data(
    client: &reqwest::Client,
    url: &str,
    targets: &HashMap<String, String>,
    from: Option<DateTime<FixedOffset>>,
    from_raw: Option<String>,
    to: Option<DateTime<FixedOffset>>,
    to_raw: Option<String>,
    max_data_points: u16,
    org: &str,     // InfluxDB-specific: organization
    token: &str,   // InfluxDB-specific: API token
    bucket: &str,  // InfluxDB-specific: bucket name
) -> Result<Vec<InfluxDBData>, CloudMonError> {
    let mut results: Vec<InfluxDBData> = Vec::new();
    
    let from_str = from
        .map(|dt| dt.to_rfc3339())
        .or(from_raw)
        .unwrap_or_else(|| "-1h".to_string());
    
    let to_str = to
        .map(|dt| dt.to_rfc3339())
        .or(to_raw)
        .unwrap_or_else(|| "now()".to_string());
    
    for (alias, query) in targets.iter() {
        let flux_query = build_influxdb_query(query, bucket, &from_str, &to_str);
        
        let res = client
            .post(format!("{}/api/v2/query", url))
            .header("Authorization", format!("Token {}", token))
            .header("Content-Type", "application/vnd.flux")
            .query(&[("org", org)])
            .body(flux_query)
            .send()
            .await;
        
        match res {
            Ok(rsp) => {
                if !rsp.status().is_success() {
                    tracing::error!("InfluxDB error: {:?}", rsp.text().await);
                    return Err(CloudMonError::GraphiteError);  // TODO: Add InfluxDBError
                }
                
                // Parse CSV response (InfluxDB returns annotated CSV by default)
                let body = rsp.text().await.map_err(|_| CloudMonError::GraphiteError)?;
                let datapoints = parse_influxdb_csv(&body)?;
                
                results.push(InfluxDBData {
                    target: alias.clone(),
                    datapoints,
                });
            }
            Err(e) => {
                tracing::error!("InfluxDB request failed: {:?}", e);
                return Err(CloudMonError::GraphiteError);
            }
        }
    }
    
    Ok(results)
}

fn parse_influxdb_csv(csv: &str) -> Result<Vec<(Option<f32>, u32)>, CloudMonError> {
    // Parse InfluxDB annotated CSV format
    // Implementation depends on specific CSV structure
    let mut datapoints: Vec<(Option<f32>, u32)> = Vec::new();
    
    for line in csv.lines() {
        if line.starts_with('#') || line.is_empty() || line.starts_with(',') {
            continue;  // Skip annotations and headers
        }
        
        let parts: Vec<&str> = line.split(',').collect();
        // Typical columns: "", result, table, _start, _stop, _time, _value, _field, _measurement
        if parts.len() >= 7 {
            if let (Ok(timestamp), Ok(value)) = (
                parts[5].parse::<i64>(),  // _time as unix timestamp
                parts[6].parse::<f32>(),  // _value
            ) {
                datapoints.push((Some(value), timestamp as u32));
            }
        }
    }
    
    Ok(datapoints)
}
```

## Step 3: Configuration Integration

### Update config.rs

Add the new backend to the `DatasourceType` enum:

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatasourceType {
    Graphite,
    Prometheus,
    InfluxDB,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Datasource {
    #[serde(default = "default_datasource_type")]
    pub datasource_type: DatasourceType,
    pub url: String,
    #[serde(default = "default_timeout")]
    pub timeout: u16,
    // Backend-specific optional fields
    pub org: Option<String>,      // InfluxDB
    pub bucket: Option<String>,   // InfluxDB
    pub token: Option<String>,    // InfluxDB
}

fn default_datasource_type() -> DatasourceType {
    DatasourceType::Graphite
}
```

### Example Configuration

```yaml
# Prometheus
datasource:
  type: prometheus
  url: 'https://prometheus.example.com'
  timeout: 10

# InfluxDB
datasource:
  type: influxdb
  url: 'https://influxdb.example.com'
  timeout: 10
  org: 'my-org'
  bucket: 'metrics'
  token: '${INFLUXDB_TOKEN}'  # Use environment variable
```

## Step 4: Update lib.rs

Register the new module:

```rust
pub mod config;
pub mod common;
pub mod graphite;
pub mod prometheus;  // Add new backend
pub mod influxdb;    // Add new backend
pub mod types;
```

## Step 5: Update common.rs for Backend Selection

Modify `get_service_health` to use the configured backend:

```rust
use crate::{graphite, prometheus, influxdb};
use crate::config::DatasourceType;

pub async fn get_service_health(
    state: &AppState,
    service: &str,
    environment: &str,
    from: &str,
    to: &str,
    max_data_points: u16,
) -> Result<ServiceHealthData, CloudMonError> {
    // ... existing setup code ...
    
    // Fetch data based on configured backend
    let raw_data = match state.config.datasource.datasource_type {
        DatasourceType::Graphite => {
            graphite::get_graphite_data(
                &state.req_client,
                &state.config.datasource.url,
                &targets,
                from_dt, from_raw, to_dt, to_raw,
                max_data_points,
            ).await?
        }
        DatasourceType::Prometheus => {
            prometheus::get_prometheus_data(
                &state.req_client,
                &state.config.datasource.url,
                &targets,
                from_dt, from_raw, to_dt, to_raw,
                max_data_points,
            ).await?
        }
        DatasourceType::InfluxDB => {
            influxdb::get_influxdb_data(
                &state.req_client,
                &state.config.datasource.url,
                &targets,
                from_dt, from_raw, to_dt, to_raw,
                max_data_points,
                state.config.datasource.org.as_deref().unwrap_or(""),
                state.config.datasource.token.as_deref().unwrap_or(""),
                state.config.datasource.bucket.as_deref().unwrap_or(""),
            ).await?
        }
    };
    
    // ... existing processing code ...
}
```

## Step 6: Add Error Types

Update `types.rs` to add backend-specific errors:

```rust
pub enum CloudMonError {
    ServiceNotSupported,
    EnvNotSupported,
    ExpressionError,
    GraphiteError,
    PrometheusError,
    InfluxDBError,
}

impl fmt::Display for CloudMonError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            // ... existing matches ...
            CloudMonError::PrometheusError => write!(f, "Prometheus query error"),
            CloudMonError::InfluxDBError => write!(f, "InfluxDB query error"),
        }
    }
}
```

## Step 7: Testing

### Unit Tests with Mocked Responses

```rust
#[cfg(test)]
mod test {
    use super::*;
    use mockito::Matcher;
    
    #[tokio::test]
    async fn test_get_prometheus_data() {
        let mut server = mockito::Server::new_async().await;
        
        let mock = server
            .mock("GET", "/api/v1/query_range")
            .match_query(Matcher::AllOf(vec![
                Matcher::UrlEncoded("query".into(), "up{job=\"test\"}".into()),
            ]))
            .with_body(r#"{
                "status": "success",
                "data": {
                    "resultType": "matrix",
                    "result": [{
                        "metric": {"__name__": "up"},
                        "values": [[1704067200, "1"], [1704067260, "1"]]
                    }]
                }
            }"#)
            .create_async()
            .await;
        
        let client = reqwest::Client::new();
        let mut targets = HashMap::new();
        targets.insert("test-metric".to_string(), "up{job=\"test\"}".to_string());
        
        let result = get_prometheus_data(
            &client,
            &server.url(),
            &targets,
            None, Some("2024-01-01T00:00:00Z".to_string()),
            None, Some("2024-01-01T01:00:00Z".to_string()),
            100,
        ).await;
        
        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data.len(), 1);
        assert_eq!(data[0].target, "test-metric");
        
        mock.assert_async().await;
    }
}
```

### Integration Tests

Create `tests/prometheus_integration.rs`:

```rust
//! Integration tests for Prometheus backend
//! 
//! These tests require a running Prometheus instance.
//! Set PROMETHEUS_URL environment variable to run.

#[tokio::test]
#[ignore]  // Run with: cargo test -- --ignored
async fn test_prometheus_live_query() {
    let url = std::env::var("PROMETHEUS_URL")
        .expect("PROMETHEUS_URL must be set for integration tests");
    
    // Test implementation
}
```

## Implementation Checklist

- [ ] Create `src/{backend}.rs` module
- [ ] Define response data structure
- [ ] Implement `get_{backend}_data` function
- [ ] Handle query aliasing/naming
- [ ] Parse backend-specific response format
- [ ] Convert to standard `(Option<f32>, u32)` datapoints
- [ ] Add error handling with logging
- [ ] Update `DatasourceType` enum in `config.rs`
- [ ] Add backend-specific config fields if needed
- [ ] Register module in `lib.rs`
- [ ] Update `common.rs` to dispatch to correct backend
- [ ] Add unit tests with mocked responses
- [ ] Add integration tests (optional)
- [ ] Update documentation

## Query Translation Reference

| Graphite | Prometheus | InfluxDB (Flux) |
|----------|------------|-----------------|
| `alias(query, 'name')` | Label in response | Pipe to `rename()` |
| `sumSeries(...)` | `sum(...)` | `\|> sum()` |
| `averageSeries(...)` | `avg(...)` | `\|> mean()` |
| `from=-1h` | `start=-1h` | `range(start: -1h)` |
| `maxDataPoints=100` | `step` calculation | `aggregateWindow()` |

## See Also

- [TSDB Interface](./interface.md) - Interface requirements
- [Graphite Implementation](./graphite.md) - Reference implementation
