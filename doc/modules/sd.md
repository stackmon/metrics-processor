# Status Dashboard Module (`sd`)

The `sd` module provides all functionality for integrating with the Status Dashboard API, including component management, incident creation, cache operations, and authentication.

## Module Location

- **Source**: `src/sd.rs`
- **Public export**: `cloudmon_metrics::sd`

## Overview

This module consolidates all Status Dashboard V2 API integration logic in one place, providing:

- Component fetching and caching
- Component ID resolution with subset attribute matching
- Incident creation with static payloads
- HMAC-JWT authentication

## Data Structures

### ComponentAttribute

Key-value pair for identifying components:

```rust
#[derive(Clone, Deserialize, Serialize, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct ComponentAttribute {
    pub name: String,
    pub value: String,
}
```

Derives `Ord` and `PartialOrd` for deterministic sorting in cache keys.

### Component

Component definition from configuration:

```rust
#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct Component {
    pub name: String,
    pub attributes: Vec<ComponentAttribute>,
}
```

### StatusDashboardComponent

API response from `GET /v2/components`:

```rust
#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct StatusDashboardComponent {
    pub id: u32,
    pub name: String,
    #[serde(default)]
    pub attributes: Vec<ComponentAttribute>,
}
```

### IncidentData

API request for `POST /v2/events`:

```rust
#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct IncidentData {
    pub title: String,
    pub description: String,
    pub impact: u8,
    pub components: Vec<u32>,
    pub start_date: String,  // RFC3339 format
    pub system: bool,
    #[serde(rename = "type")]
    pub incident_type: String,
}
```

### ComponentCache

Type alias for the component ID cache:

```rust
pub type ComponentCache = HashMap<(String, Vec<ComponentAttribute>), u32>;
```

Key: `(component_name, sorted_attributes)` → Value: `component_id`

## Functions

### Authentication

#### `build_auth_headers`

```rust
pub fn build_auth_headers(secret: Option<&str>) -> HeaderMap
```

Generates HMAC-JWT authorization headers for Status Dashboard API.

- Creates Bearer token using HMAC-SHA256 signing
- Returns empty HeaderMap if no secret provided (optional auth)

**Example**:
```rust
let headers = build_auth_headers(Some("my-secret"));
// Headers contain: Authorization: Bearer eyJ...
```

### Component Management

#### `fetch_components`

```rust
pub async fn fetch_components(
    client: &reqwest::Client,
    base_url: &str,
    headers: &HeaderMap,
) -> anyhow::Result<Vec<StatusDashboardComponent>>
```

Fetches all components from Status Dashboard API V2 (`GET /v2/components`).

#### `build_component_id_cache`

```rust
pub fn build_component_id_cache(
    components: Vec<StatusDashboardComponent>
) -> ComponentCache
```

Builds component ID cache from fetched components. Sorts attributes for deterministic cache keys.

#### `find_component_id`

```rust
pub fn find_component_id(
    cache: &ComponentCache,
    target: &Component
) -> Option<u32>
```

Finds component ID in cache with **subset attribute matching**:
- Config attributes must be a subset of cache attributes
- Example: config `{region: "EU-DE"}` matches cache `{region: "EU-DE", category: "Storage"}`

### Incident Management

#### `build_incident_data`

```rust
pub fn build_incident_data(
    component_id: u32,
    impact: u8,
    timestamp: i64
) -> IncidentData
```

Builds incident data structure for V2 API:
- **Static title**: "System incident from monitoring system"
- **Static description**: "System-wide incident affecting one or multiple components. Created automatically."
- **Timestamp**: RFC3339 format, minus 1 second from input
- **system**: true (indicates auto-generated)

#### `create_incident`

```rust
pub async fn create_incident(
    client: &reqwest::Client,
    base_url: &str,
    headers: &HeaderMap,
    incident_data: &IncidentData,
) -> anyhow::Result<()>
```

Creates incident via Status Dashboard API V2 (`POST /v2/events`).

## Usage Example

```rust
use cloudmon_metrics::sd::{
    build_auth_headers, build_component_id_cache, build_incident_data,
    create_incident, fetch_components, find_component_id,
    Component, ComponentAttribute,
};

// Build auth headers
let headers = build_auth_headers(config.secret.as_deref());

// Fetch and cache components
let components = fetch_components(&client, &url, &headers).await?;
let cache = build_component_id_cache(components);

// Find component ID
let target = Component {
    name: "Object Storage Service".to_string(),
    attributes: vec![ComponentAttribute {
        name: "region".to_string(),
        value: "EU-DE".to_string(),
    }],
};
let component_id = find_component_id(&cache, &target)?;

// Create incident
let incident = build_incident_data(component_id, 2, timestamp);
create_incident(&client, &url, &headers, &incident).await?;
```

## Testing

Integration tests are in `tests/integration_sd.rs`:

```bash
cargo test --test integration_sd
```

**Test coverage**:
- `test_fetch_components_success` - API fetching
- `test_build_component_id_cache` - Cache structure
- `test_find_component_id_subset_matching` - Subset matching logic
- `test_build_incident_data_structure` - Static payload generation
- `test_timestamp_rfc3339_minus_one_second` - Timestamp handling
- `test_create_incident_success` - API posting
- `test_build_auth_headers` - JWT generation
- Additional edge case tests

## Related Documentation

- [Reporter Overview](../reporter.md) - How reporter uses this module
- [API Contracts](../../specs/003-sd-api-v2-migration/contracts/) - V2 API specifications
- [Spec](../../specs/003-sd-api-v2-migration/spec.md) - Feature specification
