# API Module

The API module (`src/api.rs` and `src/api/v1.rs`) provides HTTP endpoints for the metrics-processor service using the Axum web framework.

## Module Structure

```
src/api.rs          # Module declaration
src/api/v1.rs       # V1 API implementation
```

## API v1 Routes

The `get_v1_routes()` function constructs the router:

```rust
pub fn get_v1_routes() -> Router<AppState> {
    return Router::new()
        .route("/", get(root))
        .route("/info", get(info))
        .route("/health", get(handler_health));
}
```

### Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/v1/` | `root` | Returns API name |
| GET | `/v1/info` | `info` | Returns API info text |
| GET | `/v1/health` | `handler_health` | Returns service health metrics |

## Request/Response Types

### HealthQuery

Query parameters for the `/health` endpoint:

```rust
#[derive(Debug, Deserialize)]
pub struct HealthQuery {
    /// Start point to query metrics (RFC3339 timestamp)
    pub from: String,
    /// End point to query metrics (RFC3339 timestamp)
    pub to: String,
    /// Maximum data points to return (default: 100)
    #[serde(default = "default_max_data_points")]
    pub max_data_points: u32,
    /// Service name to query
    pub service: String,
    /// Environment name
    pub environment: String,
}
```

**Example request:**
```
GET /v1/health?from=2024-01-01T00:00:00Z&to=2024-01-02T00:00:00Z&service=srvA&environment=env1
```

### ServiceHealthResponse

Response structure for the `/health` endpoint:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct ServiceHealthResponse {
    /// Service name
    pub name: String,
    /// Service category (e.g., "compute", "storage")
    pub service_category: String,
    /// Environment name
    pub environment: String,
    /// Health metric data points: Vec<(timestamp, health_value)>
    pub metrics: ServiceHealthData,
}
```

**Example response:**
```json
{
  "name": "srvA",
  "service_category": "compute",
  "environment": "env1",
  "metrics": [[1704067200, 1], [1704070800, 0]]
}
```

## Handler Implementation

### handler_health

The main health endpoint handler:

```rust
pub async fn handler_health(
    query: Query<HealthQuery>,
    State(state): State<AppState>
) -> Response {
    // 1. Look up service in health_metrics config
    // 2. Call get_service_health() from common module
    // 3. Return ServiceHealthResponse or error
}
```

**Error Responses:**

| Status | Condition |
|--------|-----------|
| 200 OK | Success |
| 409 Conflict | Service or environment not supported |
| 500 Internal Server Error | Expression evaluation or Graphite error |

## State Management

All handlers receive `AppState` via Axum's state extraction:

```rust
State(state): State<AppState>
```

The `AppState` contains:
- Processed configuration
- Pre-computed metric templates
- HTTP client for Graphite queries
- Flag and health metric definitions

## Authentication

Currently, the API does not implement authentication. The `status_dashboard.jwt_secret` configuration option suggests JWT-based authentication may be planned for integration with status dashboard services.

## Integration with Other Modules

```
api::v1
    │
    ├──► types::AppState      (state extraction)
    ├──► common::get_service_health()  (business logic)
    └──► types::CloudMonError (error handling)
```
