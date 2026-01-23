# Data Model: Status Dashboard API V2 Migration

**Feature**: Reporter Migration to Status Dashboard API V2  
**Branch**: `003-sd-api-v2-migration`  
**Date**: 2025-01-23

## Overview

This document defines the data entities and their relationships for the Status Dashboard API V2 migration. The migration introduces a component ID caching layer and restructures incident data to align with the V2 API schema.

---

## Core Entities

### 1. ComponentAttribute

**Purpose**: Represents a key-value attribute that qualifies a component (e.g., `region=EU-DE`, `category=Storage`)

**Rust Definition**:
```rust
#[derive(Clone, Deserialize, Serialize, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct ComponentAttribute {
    pub name: String,   // Attribute name (e.g., "region", "category", "type")
    pub value: String,  // Attribute value (e.g., "EU-DE", "Storage")
}
```

**JSON Representation** (Status Dashboard API V2):
```json
{
  "name": "region",
  "value": "EU-DE"
}
```

**Validation Rules**:
- `name`: Non-empty string, typically one of `["region", "category", "type"]` (per OpenAPI enum)
- `value`: Non-empty string

**Traits**:
- `PartialOrd`, `Ord`: Required for sorting attributes before caching
- `Hash`, `Eq`: Required for use in HashMap keys
- `Serialize`, `Deserialize`: JSON API interaction

**Relationships**:
- **Owned by**: `Component` (from config), `StatusDashboardComponent` (from API)
- **Used in**: Component cache key construction

---

### 2. Component (Config)

**Purpose**: Represents a component definition from the reporter's configuration file. Used to look up component IDs in the cache.

**Rust Definition**:
```rust
#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct Component {
    pub name: String,                    // Component name (e.g., "Object Storage Service")
    pub attributes: Vec<ComponentAttribute>,  // Attributes from config + environment
}
```

**Source**: Reporter configuration (`config.yaml`)

**Example**:
```yaml
# In config.yaml health_metrics section
health_metrics:
  swift:
    component_name: "Object Storage Service"
    # attributes come from environment.attributes

environments:
  - name: production
    attributes:
      region: "EU-DE"
      category: "Storage"
```

**Construction Logic** (from `reporter.rs`):
```rust
// Combines component_name from health_metric + attributes from environment
let component = Component {
    name: health_metric.component_name.clone(),
    attributes: env.attributes.clone(),
};
```

**Relationships**:
- **Created from**: Configuration file (`config.yaml`)
- **Used for**: Component ID cache lookup
- **Key construction**: `(component.name, sorted(component.attributes))` → cache key

---

### 3. StatusDashboardComponent (API Response)

**Purpose**: Represents a component as returned by the Status Dashboard API `/v2/components` endpoint. Used to build the component ID cache.

**Rust Definition**:
```rust
#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct StatusDashboardComponent {
    pub id: u32,                         // Component ID (primary key in Status Dashboard)
    pub name: String,                    // Component name
    #[serde(default)]
    pub attributes: Vec<ComponentAttribute>,  // Component attributes (may be empty)
}
```

**JSON Representation** (API response from `GET /v2/components`):
```json
{
  "id": 218,
  "name": "Object Storage Service",
  "attributes": [
    {"name": "category", "value": "Storage"},
    {"name": "region", "value": "EU-DE"}
  ]
}
```

**Source**: Status Dashboard API `/v2/components` endpoint

**Validation Rules**:
- `id`: Positive integer (u32)
- `name`: Non-empty string
- `attributes`: Array (may be empty per `#[serde(default)]`)

**Relationships**:
- **Fetched from**: Status Dashboard API
- **Used to build**: Component ID cache (`ComponentCache`)
- **Cache entry**: `(name, sorted(attributes))` → `id`

---

### 4. ComponentCache

**Purpose**: In-memory cache mapping component names and attributes to component IDs. Avoids repeated API calls during monitoring cycles.

**Rust Definition**:
```rust
type ComponentCache = HashMap<(String, Vec<ComponentAttribute>), u32>;
// Key: (component_name, sorted_attributes)
// Value: component_id
```

**Example Cache State**:
```rust
{
    ("Object Storage Service", vec![
        ComponentAttribute { name: "category", value: "Storage" },
        ComponentAttribute { name: "region", value: "EU-DE" },
    ]): 218,
    
    ("Compute Service", vec![
        ComponentAttribute { name: "category", value: "Compute" },
        ComponentAttribute { name: "region", value: "EU-NL" },
    ]): 254,
}
```

**Cache Operations**:
1. **Build** (startup):
   ```rust
   fn build_component_id_cache(components: Vec<StatusDashboardComponent>) 
       -> ComponentCache 
   {
       components.into_iter().map(|c| {
           let mut attrs = c.attributes;
           attrs.sort();  // Ensure deterministic key
           ((c.name, attrs), c.id)
       }).collect()
   }
   ```

2. **Lookup**:
   ```rust
   fn lookup_component_id(
       cache: &ComponentCache,
       component: &Component
   ) -> Option<u32> {
       let mut attrs = component.attributes.clone();
       attrs.sort();  // Match cache key format
       cache.get(&(component.name.clone(), attrs)).copied()
   }
   ```

3. **Refresh** (on miss):
   ```rust
   async fn refresh_cache(client: &Client, url: &str) 
       -> Result<ComponentCache> 
   {
       let components = fetch_components(client, url).await?;
       Ok(build_component_id_cache(components))
   }
   ```

**Lifecycle**:
- **Created**: Reporter startup (with 3 retries, 60s delays per FR-006)
- **Refreshed**: On cache miss during incident creation (1 attempt, per FR-005)
- **Invalidated**: Never (components are stable; refresh only on miss)

**Subset Matching** (FR-012):
The cache stores full component attributes from the Status Dashboard. Config may specify fewer attributes:
```rust
// Config component
Component { 
    name: "Storage", 
    attributes: vec![region=EU-DE] 
}

// Dashboard component (in cache)
StatusDashboardComponent { 
    id: 218, 
    name: "Storage", 
    attributes: vec![region=EU-DE, type=block] 
}

// Lookup fails because keys don't match exactly!
// Solution: FR-012 specifies subset matching, but cache uses exact key matching.
// Implementation must iterate cache to find subset matches.
```

**Corrected Lookup Algorithm** (for subset matching):
```rust
fn find_component_id(
    cache: &ComponentCache,
    target: &Component
) -> Option<u32> {
    cache.iter()
        .filter(|((name, _attrs), _id)| name == &target.name)
        .find(|((_name, cache_attrs), _id)| {
            // Config attrs must be subset of cache attrs
            target.attributes.iter().all(|target_attr| {
                cache_attrs.iter().any(|cache_attr| {
                    cache_attr.name == target_attr.name 
                    && cache_attr.value == target_attr.value
                })
            })
        })
        .map(|((_name, _attrs), id)| *id)
}
```

**Performance**: O(n) worst case where n = cache size (~100 components), acceptable for 60s monitoring intervals.

---

### 5. IncidentData (V2 API Request)

**Purpose**: Represents the incident payload sent to Status Dashboard API V2 `/v2/incidents` endpoint.

**Rust Definition**:
```rust
#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct IncidentData {
    pub title: String,              // Static: "System incident from monitoring system"
    #[serde(default)]
    pub description: String,        // Static generic message (FR-017)
    pub impact: u8,                 // 0=none, 1=minor, 2=major, 3=critical
    pub components: Vec<u32>,       // Component IDs (resolved from cache)
    pub start_date: DateTime<Utc>,  // Health metric timestamp - 1s (RFC3339)
    #[serde(default)]
    pub system: bool,               // Always true for auto-created incidents
    #[serde(rename = "type")]
    pub incident_type: String,      // Always "incident" for auto-created
}
```

**JSON Representation** (POST request body):
```json
{
  "title": "System incident from monitoring system",
  "description": "System-wide incident affecting one or multiple components. Created automatically.",
  "impact": 2,
  "components": [218],
  "start_date": "2025-01-22T10:30:44Z",
  "system": true,
  "type": "incident"
}
```

**Field Mapping from Health Metric**:

| Source | Field | Value | Transformation |
|--------|-------|-------|----------------|
| Health API | `timestamp` (i64) | Epoch seconds | `DateTime::from_timestamp(ts - 1, 0)` |
| Health API | `impact` (u8) | 0-3 | Direct copy |
| Config | Component name + attrs | "Storage", {region:EU-DE} | Resolve to component ID via cache |
| Static | `title` | - | "System incident from monitoring system" |
| Static | `description` | - | "System-wide incident..." |
| Static | `system` | - | `true` |
| Static | `incident_type` | - | `"incident"` |

**Construction Logic**:
```rust
async fn build_incident_data(
    service_health: &ServiceHealthData,
    component_id: u32,
) -> IncidentData {
    let (timestamp, impact) = service_health.metrics.last().unwrap();
    
    let start_date = DateTime::<Utc>::from_timestamp(
        *timestamp - 1,  // -1 second per FR-011
        0
    ).unwrap();
    
    IncidentData {
        title: "System incident from monitoring system".to_string(),
        description: "System-wide incident affecting one or multiple components. Created automatically.".to_string(),
        impact: *impact,
        components: vec![component_id],
        start_date,
        system: true,
        incident_type: "incident".to_string(),
    }
}
```

**Validation Rules**:
- `impact`: Must be in range [0, 3]
- `components`: Non-empty array (at least one component ID)
- `start_date`: Valid RFC3339 datetime
- `type`: Must be `"incident"` (not `"maintenance"` or `"info"`)

**API Response** (on success):
```json
{
  "result": [
    {
      "component_id": 218,
      "incident_id": 456  // Existing incident ID if duplicate, or new ID
    }
  ]
}
```

**Relationships**:
- **Created from**: `ServiceHealthResponse` (health API) + `Component` (config) + `ComponentCache` (ID lookup)
- **Sent to**: Status Dashboard API `/v2/incidents`
- **Security**: Generic title/description prevent exposing sensitive data (FR-017)

---

### 6. ServiceHealthResponse (Existing, unchanged)

**Purpose**: Response from the local convertor API `/api/v1/health` containing service health metrics.

**Rust Definition** (from `src/api/v1.rs`):
```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct ServiceHealthResponse {
    pub name: String,              // Service name (e.g., "swift")
    pub service_category: String,  // Category (e.g., "Storage")
    pub environment: String,       // Environment name (e.g., "production")
    pub metrics: ServiceHealthData,  // Health data points
}

pub type ServiceHealthData = Vec<(i64, u8)>;
// Vec of (timestamp_epoch_seconds, impact_0_to_3)
```

**Example**:
```json
{
  "name": "swift",
  "service_category": "Storage",
  "environment": "production",
  "metrics": [
    [1706000000, 0],
    [1706000060, 0],
    [1706000120, 2]  // Impact level 2 (major issue)
  ]
}
```

**Usage in Reporter**:
```rust
// Reporter queries convertor API
let response: ServiceHealthResponse = req_client
    .get("http://localhost:8080/api/v1/health")
    .query(&[
        ("environment", "production"),
        ("service", "swift"),
        ("from", "-5min"),
        ("to", "-2min")
    ])
    .send().await?
    .json().await?;

// Check last metric
if let Some((timestamp, impact)) = response.metrics.last() {
    if *impact > 0 {
        // Create incident using *impact and *timestamp
    }
}
```

**Relationships**:
- **Source**: Local convertor API (unchanged by this migration)
- **Consumed by**: Reporter's monitoring loop
- **Used to create**: `IncidentData` when impact > 0

---

## Data Flow

### Startup Flow

```
┌─────────────────────────────────────────────────────────────────┐
│ 1. Reporter Startup                                             │
└───────────────┬─────────────────────────────────────────────────┘
                │
                ▼
┌─────────────────────────────────────────────────────────────────┐
│ 2. Fetch Components (with retry)                                │
│    GET /v2/components → Vec<StatusDashboardComponent>           │
│    Retry: 3 attempts, 60s delay (FR-006)                        │
└───────────────┬─────────────────────────────────────────────────┘
                │
                ▼
┌─────────────────────────────────────────────────────────────────┐
│ 3. Build Component Cache                                        │
│    ComponentCache = HashMap<(name, attrs), id>                  │
│    Sort attributes before inserting                             │
└───────────────┬─────────────────────────────────────────────────┘
                │
                ▼
┌─────────────────────────────────────────────────────────────────┐
│ 4. Start Monitoring Loop                                        │
│    Every 60 seconds                                             │
└─────────────────────────────────────────────────────────────────┘
```

### Incident Creation Flow

```
┌─────────────────────────────────────────────────────────────────┐
│ 1. Query Health API                                             │
│    GET /api/v1/health?env=prod&service=swift                    │
│    Response: ServiceHealthResponse                              │
└───────────────┬─────────────────────────────────────────────────┘
                │
                ▼
┌─────────────────────────────────────────────────────────────────┐
│ 2. Check Impact Level                                           │
│    if metrics.last().impact > 0 { proceed }                     │
└───────────────┬─────────────────────────────────────────────────┘
                │
                ▼
┌─────────────────────────────────────────────────────────────────┐
│ 3. Lookup Component ID                                          │
│    component = config.get(service_name)                         │
│    component_id = cache.find((component.name, component.attrs)) │
└───────────────┬─────────────────────────────────────────────────┘
                │
          ┌─────┴─────┐
          │           │
    Found │           │ Not Found
          ▼           ▼
     ┌─────────┐  ┌────────────────────────────────┐
     │ Create  │  │ Refresh Cache (1 attempt)      │
     │Incident │  │ Retry lookup once (FR-005)     │
     └────┬────┘  └──────────┬─────────────────────┘
          │                  │
          │         ┌────────┴────────┐
          │   Found │                 │ Still Not Found
          │         ▼                 ▼
          │  ┌─────────────┐  ┌──────────────────┐
          │  │   Create    │  │ Log Warning      │
          │  │  Incident   │  │ Skip incident    │
          │  └──────┬──────┘  │ Continue loop    │
          │         │         └──────────────────┘
          └─────────┴──────┐
                           ▼
┌─────────────────────────────────────────────────────────────────┐
│ 4. Build IncidentData                                           │
│    - title: static                                              │
│    - description: static                                        │
│    - impact: from health metric                                 │
│    - components: [component_id]                                 │
│    - start_date: timestamp - 1s (RFC3339)                       │
│    - system: true                                               │
│    - type: "incident"                                           │
└───────────────┬─────────────────────────────────────────────────┘
                │
                ▼
┌─────────────────────────────────────────────────────────────────┐
│ 5. POST to /v2/incidents                                        │
│    Authorization: Bearer <HMAC-JWT>                             │
│    Body: IncidentData (JSON)                                    │
│    Timeout: 10s (FR-014)                                        │
└───────────────┬─────────────────────────────────────────────────┘
                │
          ┌─────┴─────┐
          │           │
    Success│           │ Error
          ▼           ▼
     ┌─────────┐  ┌───────────────────────────┐
     │Log INFO │  │ Log ERROR (status + body) │
     │Continue │  │ Continue to next service  │
     │to next  │  │ (retry in next cycle)     │
     └─────────┘  └───────────────────────────┘
```

---

## Entity Relationships Diagram

```
┌──────────────────────────────────────────────────────────────────┐
│                         Configuration                            │
│                        (config.yaml)                             │
└────────────────┬─────────────────────────────────────────────────┘
                 │
                 │ Defines
                 ▼
┌────────────────────────────────┐      ┌───────────────────────┐
│       Component (Config)       │      │  ComponentAttribute   │
├────────────────────────────────┤      ├───────────────────────┤
│ + name: String                 │◆─────│ + name: String        │
│ + attributes: Vec<Attribute>   │1   * │ + value: String       │
└────────────────┬───────────────┘      └───────────────────────┘
                 │                                 ▲
                 │ Lookup                          │
                 ▼                                 │ Uses
┌────────────────────────────────────────────┐    │
│          ComponentCache                    │    │
│   HashMap<(String, Vec<Attr>), u32>        │    │
├────────────────────────────────────────────┤    │
│ Key: (component_name, sorted_attributes)   │    │
│ Value: component_id                        │    │
└────────────────┬───────────────────────────┘    │
                 ▲                                 │
                 │ Built from                      │
                 │                                 │
┌────────────────┴───────────────┐                │
│  StatusDashboardComponent      │                │
│      (from API)                │◆───────────────┘
├────────────────────────────────┤1              *
│ + id: u32                      │
│ + name: String                 │
│ + attributes: Vec<Attribute>   │
└────────────────────────────────┘
         ▲
         │ Fetched from
         │
┌────────┴──────────────────────────────────────────────────┐
│     Status Dashboard API: GET /v2/components              │
└───────────────────────────────────────────────────────────┘


┌────────────────────────────────┐
│   ServiceHealthResponse        │
│   (from convertor API)         │
├────────────────────────────────┤
│ + name: String                 │
│ + service_category: String     │
│ + environment: String          │
│ + metrics: Vec<(i64, u8)>      │  (timestamp, impact)
└────────────────┬───────────────┘
                 │
                 │ impact > 0?
                 ▼
         ┌───────────────┐
         │  Resolve via  │
         │     Cache     │
         └───────┬───────┘
                 │
                 ▼ component_id
┌────────────────────────────────┐
│        IncidentData            │
│     (V2 API request)           │
├────────────────────────────────┤
│ + title: String                │
│ + description: String          │
│ + impact: u8                   │
│ + components: Vec<u32>         │──── Resolved from cache
│ + start_date: DateTime<Utc>   │──── From health metric (ts - 1s)
│ + system: bool                 │──── Always true
│ + incident_type: String        │──── Always "incident"
└────────────────┬───────────────┘
                 │
                 │ POST
                 ▼
┌───────────────────────────────────────────────────────────┐
│   Status Dashboard API: POST /v2/incidents                │
└───────────────────────────────────────────────────────────┘
```

---

## State Transitions

### Component Cache States

```
[Uninitialized] 
    │
    │ Startup: fetch_components_with_retry()
    │ Attempts: 3, Delay: 60s
    │
    ├── Success ──→ [Loaded]
    │                  │
    │                  │ Monitoring loop
    │                  │ Cache miss?
    │                  │
    │                  ├── Yes ──→ refresh_cache() (1 attempt)
    │                  │            │
    │                  │            ├── Success ──→ [Loaded] (updated)
    │                  │            │
    │                  │            └── Fail ──→ [Stale] (log warning, continue)
    │                  │
    │                  └── No ──→ [Loaded] (continue)
    │
    └── Fail (after 3 retries) ──→ [Failed] (panic, reporter exits)
```

### Incident Creation States

```
[Monitoring] 
    │
    │ Query health API
    │
    ├── impact = 0 ──→ [No Action] (continue to next service)
    │
    └── impact > 0 ──→ [Resolving Component]
                        │
                        │ Lookup component_id in cache
                        │
                        ├── Found ──→ [Creating Incident]
                        │              │
                        │              │ POST /v2/incidents
                        │              │
                        │              ├── Success (200) ──→ [Incident Created]
                        │              │                      │
                        │              │                      └─→ Log INFO, continue
                        │              │
                        │              └── Fail (4xx/5xx/timeout) ──→ [Error]
                        │                                              │
                        │                                              └─→ Log ERROR, continue
                        │                                                  (retry in next cycle)
                        │
                        └── Not Found ──→ [Refreshing Cache]
                                          │
                                          │ refresh_cache() (1 attempt)
                                          │
                                          ├── Found after refresh ──→ [Creating Incident]
                                          │
                                          └── Still not found ──→ [Component Missing]
                                                                   │
                                                                   └─→ Log WARNING, skip incident
```

---

## Data Validation

### Input Validation

| Entity | Field | Validation | Error Handling |
|--------|-------|------------|----------------|
| `StatusDashboardComponent` | `id` | u32 > 0 | Serde deserialization error → log + skip |
| `StatusDashboardComponent` | `name` | Non-empty string | Serde deserialization error → log + skip |
| `ComponentAttribute` | `name` | Non-empty string | Serde deserialization error → log + skip |
| `ComponentAttribute` | `value` | Non-empty string | Serde deserialization error → log + skip |
| `IncidentData` | `impact` | 0 ≤ u8 ≤ 3 | Assert in code (from health metric, already validated) |
| `IncidentData` | `components` | Non-empty Vec | Assert (only create incident if component_id found) |
| `IncidentData` | `start_date` | Valid timestamp | chrono handles validation; panic if invalid |

### Output Validation

| Field | Constraint | Enforcement |
|-------|-----------|-------------|
| `IncidentData.title` | Static string | Hardcoded in code |
| `IncidentData.description` | Static string | Hardcoded in code |
| `IncidentData.system` | Always `true` | Hardcoded in code |
| `IncidentData.incident_type` | Always `"incident"` | Hardcoded in code |
| `IncidentData.start_date` | RFC3339 format | `chrono::DateTime::to_rfc3339()` |

---

## Security Considerations

### Data Separation (FR-017)

**Sensitive Data** (logged locally, NEVER sent to API):
- Service name (e.g., "swift")
- Environment name (e.g., "production")
- Component name (e.g., "Object Storage Service")
- Component attributes (e.g., `region=EU-DE`)
- Triggered metric names (e.g., "latency_p95", "error_rate")
- Metric values (e.g., "latency=450ms")

**Public Data** (sent to Status Dashboard API):
- Static generic title: "System incident from monitoring system"
- Static generic description: "System-wide incident affecting one or multiple components. Created automatically."
- Impact level (integer 0-3, no context)
- Component IDs (integers, no names/attributes)
- Start date (timestamp only, no context)

**Rationale**: Status Dashboard is public-facing. Exposing service names, metric details, or specific component attributes would reveal internal infrastructure details.

---

## Performance Characteristics

| Operation | Complexity | Frequency | Optimization |
|-----------|-----------|-----------|--------------|
| Cache build | O(n log n) | Once at startup + rare refreshes | Acceptable; n ~100 |
| Component lookup | O(n) worst case | Per incident (~1-10/min) | Acceptable for n ~100 |
| Incident creation | O(1) | Per health issue (~1-10/min) | HTTP timeout 10s |
| Health query | O(1) | Every 60s per service | Existing, unchanged |

**Memory Usage**:
- `ComponentCache`: ~100 entries × ~200 bytes/entry = ~20 KB
- `StatusDashboardComponent` list: ~100 × ~200 bytes = ~20 KB (transient during cache build)
- Negligible compared to reporter's base memory footprint (~10 MB)

---

## Testing Strategy

### Unit Tests

1. **Component Cache Building**:
   - Test `build_component_id_cache()` with various attribute orders
   - Verify attributes are sorted in cache keys
   - Test empty attributes list

2. **Component Matching**:
   - Test exact match
   - Test subset matching (config has fewer attributes)
   - Test no match (different attribute values)
   - Test no match (different component name)

3. **Incident Data Construction**:
   - Test timestamp adjustment (-1 second)
   - Test RFC3339 formatting
   - Test static field values

### Integration Tests

1. **Cache Load & Refresh**:
   - Mock `/v2/components` endpoint
   - Test successful cache load
   - Test retry logic (3 attempts, 60s delays)
   - Test cache refresh on miss

2. **Incident Creation**:
   - Mock `/v2/incidents` endpoint
   - Test successful incident creation
   - Test duplicate incident handling (API returns existing ID)
   - Test error handling (4xx, 5xx, timeout)

3. **End-to-End Flow**:
   - Mock both convertor and Status Dashboard APIs
   - Test full flow: health query → component lookup → incident creation
   - Test cache miss → refresh → retry
   - Test component not found → skip incident

---

## Summary

This data model defines 6 core entities for the V2 migration:

1. **ComponentAttribute**: Key-value pairs qualifying components
2. **Component** (config): Reporter's view of components from config
3. **StatusDashboardComponent**: API's view of components
4. **ComponentCache**: In-memory mapping for efficient lookups
5. **IncidentData**: V2 incident request payload
6. **ServiceHealthResponse**: Existing health data (unchanged)

Key design decisions:
- **Cache structure**: HashMap with sorted attribute keys for deterministic lookups
- **Subset matching**: Iterate cache to find components where config attrs ⊆ dashboard attrs
- **Static incident fields**: Prevent exposing sensitive operational data on public dashboard
- **Timestamp handling**: RFC3339 with -1 second adjustment per FR-011

All entities align with OpenAPI schema and functional requirements (FR-001 through FR-017).
