# POST /v2/incidents

## Overview

Create a new incident in Status Dashboard when a service health issue is detected.

**Endpoint**: `POST /v2/incidents`  
**Authentication**: Required (HMAC-JWT Bearer token)  
**Frequency**: Per health issue detection (~1-10 incidents/min under normal load)

## Request

### HTTP Method
```
POST /v2/incidents HTTP/1.1
Host: {status-dashboard-url}
Authorization: Bearer {jwt-token}
Content-Type: application/json
```

### Headers

| Header | Required | Value | Description |
|--------|----------|-------|-------------|
| `Authorization` | Yes | `Bearer {jwt-token}` | HMAC-signed JWT (unchanged from V1) |
| `Content-Type` | Yes | `application/json` | Request body format |

### Request Body

**Schema**:
```yaml
type: object
required: [title, impact, components, start_date, type]
properties:
  title:
    type: string
    description: Incident title (static for auto-created)
    example: "System incident from monitoring system"
  description:
    type: string
    description: Generic description (optional, defaults to empty)
    example: "System-wide incident affecting one or multiple components. Created automatically."
  impact:
    type: integer
    enum: [0, 1, 2, 3]
    description: "Impact level: 0=none, 1=minor, 2=major, 3=critical"
    example: 2
  components:
    type: array
    items:
      type: integer
    description: Array of component IDs (resolved from cache)
    example: [218]
  start_date:
    type: string
    format: date-time
    description: Incident start time (RFC3339, health metric timestamp - 1s)
    example: "2025-01-22T10:30:44Z"
  end_date:
    type: string
    format: date-time
    description: Incident end time (optional, not used for auto-created)
  system:
    type: boolean
    default: false
    description: System-generated flag (true for auto-created)
    example: true
  type:
    type: string
    enum: [incident, maintenance]
    description: Event type (always "incident" for auto-created)
    example: "incident"
```

**Example Request Body** (typical auto-created incident):
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

**Example Request Body** (multi-component incident):
```json
{
  "title": "System incident from monitoring system",
  "description": "System-wide incident affecting one or multiple components. Created automatically.",
  "impact": 3,
  "components": [218, 254, 312],
  "start_date": "2025-01-22T10:30:44Z",
  "system": true,
  "type": "incident"
}
```

## Response

### Success Response (200 OK)

**Content-Type**: `application/json`

**Schema**:
```yaml
type: object
properties:
  result:
    type: array
    items:
      type: object
      properties:
        component_id:
          type: integer
          format: int64
          description: Component ID from request
        incident_id:
          type: integer
          format: int64
          description: Created or existing incident ID
```

**Example Response** (new incident created):
```json
{
  "result": [
    {
      "component_id": 218,
      "incident_id": 456
    }
  ]
}
```

**Example Response** (existing incident returned - duplicate detection):
```json
{
  "result": [
    {
      "component_id": 218,
      "incident_id": 123
    }
  ]
}
```

**Duplicate Handling**: If an identical incident already exists (same component + impact + active), the API returns the existing incident ID. The reporter does not need to implement deduplication logic (FR-016).

### Error Responses

#### 400 Bad Request
Invalid request body (missing required fields, invalid impact value, etc.).

```json
{
  "errMsg": "Invalid request: impact must be between 0 and 3"
}
```

#### 401 Unauthorized
Invalid or missing authentication token.

```json
{
  "errMsg": "Invalid or missing authorization token"
}
```

#### 404 Not Found
Component ID(s) not found in Status Dashboard.

```json
{
  "errMsg": "component does not exist"
}
```

#### 500 Internal Server Error
Server-side error.

```json
{
  "errMsg": "internal server error"
}
```

## Rust Implementation

### Request Struct

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct IncidentData {
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub impact: u8,
    pub components: Vec<u32>,
    pub start_date: DateTime<Utc>,
    #[serde(default)]
    pub system: bool,
    #[serde(rename = "type")]
    pub incident_type: String,
}
```

### Response Struct

```rust
#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct IncidentPostResponse {
    pub result: Vec<IncidentPostResult>,
}

#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct IncidentPostResult {
    pub component_id: u32,
    pub incident_id: u32,
}
```

### Usage Example

```rust
use reqwest::{Client, header::HeaderMap};

async fn create_incident(
    client: &Client,
    base_url: &str,
    auth_headers: &HeaderMap,
    incident: &IncidentData,
) -> Result<IncidentPostResponse, Box<dyn std::error::Error>> {
    let url = format!("{}/v2/incidents", base_url);
    
    let response = client
        .post(&url)
        .headers(auth_headers.clone())
        .json(incident)
        .send()
        .await?;
    
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await?;
        tracing::error!("Incident creation failed [{}]: {}", status, body);
        return Err(format!("API error: {} - {}", status, body).into());
    }
    
    let result = response.json::<IncidentPostResponse>().await?;
    tracing::info!(
        "Incident created: component_id={}, incident_id={}",
        result.result[0].component_id,
        result.result[0].incident_id
    );
    
    Ok(result)
}
```

### Building IncidentData

```rust
fn build_incident_data(
    component_id: u32,
    impact: u8,
    timestamp: i64,
) -> IncidentData {
    use chrono::{DateTime, Utc};
    
    // Adjust timestamp by -1 second per FR-011
    let start_date = DateTime::<Utc>::from_timestamp(timestamp - 1, 0)
        .expect("Invalid timestamp");
    
    IncidentData {
        title: "System incident from monitoring system".to_string(),
        description: "System-wide incident affecting one or multiple components. Created automatically.".to_string(),
        impact,
        components: vec![component_id],
        start_date,
        system: true,
        incident_type: "incident".to_string(),
    }
}
```

### Error Handling

```rust
async fn report_incident(
    client: &Client,
    base_url: &str,
    auth_headers: &HeaderMap,
    incident: &IncidentData,
) {
    match create_incident(client, base_url, auth_headers, incident).await {
        Ok(response) => {
            tracing::info!(
                "Successfully created/updated incident {} for component {}",
                response.result[0].incident_id,
                response.result[0].component_id
            );
        }
        Err(e) => {
            tracing::error!("Failed to create incident: {}", e);
            // Do not retry immediately - next monitoring cycle will retry (FR-015)
        }
    }
}
```

## Contract Validation

### Valid Request Examples

✅ **Minimal auto-created incident**:
```json
{
  "title": "System incident from monitoring system",
  "impact": 1,
  "components": [218],
  "start_date": "2025-01-22T10:30:44Z",
  "type": "incident"
}
```

✅ **Complete auto-created incident**:
```json
{
  "title": "System incident from monitoring system",
  "description": "System-wide incident affecting one or multiple components. Created automatically.",
  "impact": 3,
  "components": [218],
  "start_date": "2025-01-22T10:30:44Z",
  "system": true,
  "type": "incident"
}
```

### Invalid Request Examples

❌ **Missing required field `title`**:
```json
{
  "impact": 2,
  "components": [218],
  "start_date": "2025-01-22T10:30:44Z",
  "type": "incident"
}
```
*Error*: 400 Bad Request

❌ **Invalid impact value**:
```json
{
  "title": "Incident",
  "impact": 5,
  "components": [218],
  "start_date": "2025-01-22T10:30:44Z",
  "type": "incident"
}
```
*Error*: 400 Bad Request (impact must be 0-3)

❌ **Empty components array**:
```json
{
  "title": "Incident",
  "impact": 2,
  "components": [],
  "start_date": "2025-01-22T10:30:44Z",
  "type": "incident"
}
```
*Error*: 400 Bad Request (at least one component required)

❌ **Invalid date format**:
```json
{
  "title": "Incident",
  "impact": 2,
  "components": [218],
  "start_date": "2025-01-22 10:30:44",
  "type": "incident"
}
```
*Error*: 400 Bad Request (must be RFC3339 format)

## Field Constraints (FR-002)

| Field | Value | Rationale |
|-------|-------|-----------|
| `title` | `"System incident from monitoring system"` | Static generic title (FR-002) |
| `description` | `"System-wide incident affecting one or multiple components. Created automatically."` | Static generic description (FR-017, prevents sensitive data exposure) |
| `impact` | 0-3 from health metric | Direct mapping from service health (FR-002) |
| `components` | `[component_id]` | Resolved from cache lookup (FR-004) |
| `start_date` | Health timestamp - 1s | RFC3339 format, adjusted per FR-011 |
| `system` | `true` | Always true for auto-created (FR-009) |
| `type` | `"incident"` | Always "incident" for auto-created (FR-010) |

## Sensitive Data Separation (FR-017)

### Data NOT Sent to API (Logged Locally Only)

The following information MUST NOT be included in the incident payload to prevent exposing sensitive operational data on the public Status Dashboard:

- ❌ Service name (e.g., "swift", "nova")
- ❌ Environment name (e.g., "production", "staging")
- ❌ Component name (e.g., "Object Storage Service")
- ❌ Component attributes (e.g., `region=EU-DE`)
- ❌ Triggered metric names (e.g., "latency_p95", "error_rate")
- ❌ Metric values (e.g., "latency=450ms")

### Data Logged Locally (For Diagnostics)

```rust
tracing::info!(
    timestamp = %start_date,
    service = %service_name,
    environment = %env_name,
    component_name = %component.name,
    component_attrs = ?component.attributes,
    component_id = component_id,
    impact = impact,
    triggered_metrics = ?triggered_metric_names,
    "Creating incident for health issue"
);
```

### Data Sent to API (Public, Generic)

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

## Performance Considerations

- **Request Size**: ~300 bytes per incident (small payload)
- **Frequency**: ~1-10 incidents/min under normal load, higher during widespread issues
~~- **Timeout**: 10 seconds per FR-014 (increased from 2s)~~
- **Retry Strategy**: No immediate retry on failure, rely on next monitoring cycle (~60s per FR-015)

## Security

- **Authentication**: HMAC-JWT Bearer token (unchanged from V1, FR-008)
- **Data Privacy**: Generic title/description prevent sensitive data exposure (FR-017)
- **Component IDs**: Integer IDs expose less information than names/attributes
- **Public Dashboard**: All incident data is publicly visible on Status Dashboard

## Idempotency

The Status Dashboard API implements built-in duplicate detection:
- If an identical incident exists (same component + impact + still active), the API returns the existing incident ID
- The reporter does NOT need to track created incidents (FR-016)
- Each health issue detection results in a new POST request, API handles deduplication
