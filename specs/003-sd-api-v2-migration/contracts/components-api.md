# GET /v2/components

## Overview

Fetch all components from Status Dashboard to build the component ID cache.

**Endpoint**: `GET /v2/components`  
**Authentication**: Optional (HMAC-JWT Bearer token if configured)  
**Frequency**: Startup + on-demand cache refresh

## Request

### HTTP Method
```
GET /v2/components HTTP/1.1
Host: {status-dashboard-url}
Authorization: Bearer {jwt-token}
```

### Headers

| Header | Required | Value | Description |
|--------|----------|-------|-------------|
| `Authorization` | Optional | `Bearer {jwt-token}` | HMAC-signed JWT if secret configured |

### Query Parameters

None.

### Request Body

None (GET request).

## Response

### Success Response (200 OK)

**Content-Type**: `application/json`

**Schema**:
```yaml
type: array
items:
  type: object
  required: [id, name, attributes]
  properties:
    id:
      type: integer
      format: int64
      description: Component ID (primary key)
      example: 218
    name:
      type: string
      description: Component name
      example: "Object Storage Service"
    attributes:
      type: array
      items:
        type: object
        properties:
          name:
            type: string
            enum: [category, region, type]
            description: Attribute name
            example: "category"
          value:
            type: string
            description: Attribute value
            example: "Storage"
```

**Example Response**:
```json
[
  {
    "id": 218,
    "name": "Object Storage Service",
    "attributes": [
      {
        "name": "category",
        "value": "Storage"
      },
      {
        "name": "region",
        "value": "EU-DE"
      }
    ]
  },
  {
    "id": 254,
    "name": "Compute Service",
    "attributes": [
      {
        "name": "category",
        "value": "Compute"
      },
      {
        "name": "region",
        "value": "EU-NL"
      },
      {
        "name": "type",
        "value": "vm"
      }
    ]
  },
  {
    "id": 312,
    "name": "Database Service",
    "attributes": []
  }
]
```

### Error Responses

#### 401 Unauthorized
Invalid or missing authentication token (if auth required).

```json
{
  "errMsg": "Invalid or missing authorization token"
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
// No request body struct needed (GET request)
```

### Response Struct

```rust
#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct StatusDashboardComponent {
    pub id: u32,
    pub name: String,
    #[serde(default)]
    pub attributes: Vec<ComponentAttribute>,
}

#[derive(Clone, Deserialize, Serialize, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct ComponentAttribute {
    pub name: String,
    pub value: String,
}
```

### Usage Example

```rust
use reqwest::{Client, header::{HeaderMap, AUTHORIZATION}};
use serde::Deserialize;

async fn fetch_components(
    client: &Client,
    base_url: &str,
    auth_headers: &HeaderMap,
) -> Result<Vec<StatusDashboardComponent>, Box<dyn std::error::Error>> {
    let url = format!("{}/v2/components", base_url);
    
    let response = client
        .get(&url)
        .headers(auth_headers.clone())
        .send()
        .await?;
    
    response.error_for_status_ref()?;
    
    let components = response.json::<Vec<StatusDashboardComponent>>().await?;
    
    tracing::info!("Fetched {} components from Status Dashboard", components.len());
    
    Ok(components)
}
```

## Cache Building

Once components are fetched, build the cache:

```rust
use std::collections::HashMap;

fn build_component_id_cache(
    components: Vec<StatusDashboardComponent>
) -> HashMap<(String, Vec<ComponentAttribute>), u32> {
    components.into_iter().map(|c| {
        let mut attrs = c.attributes;
        attrs.sort();  // Ensure deterministic cache key
        ((c.name, attrs), c.id)
    }).collect()
}
```

## Error Handling

```rust
async fn fetch_components_with_retry(
    client: &Client,
    base_url: &str,
    auth_headers: &HeaderMap,
) -> Option<Vec<StatusDashboardComponent>> {
    for attempt in 1..=3 {
        match fetch_components(client, base_url, auth_headers).await {
            Ok(components) => {
                tracing::info!("Successfully fetched {} components", components.len());
                return Some(components);
            }
            Err(e) => {
                tracing::error!("Failed to fetch components (attempt {}/3): {}", attempt, e);
                if attempt < 3 {
                    tracing::info!("Retrying in 60 seconds...");
                    tokio::time::sleep(Duration::from_secs(60)).await;
                } else {
                    tracing::error!("Could not fetch components after 3 attempts");
                    return None;
                }
            }
        }
    }
    None
}
```

## Contract Validation

### Valid Response Examples

✅ **Complete component with attributes**:
```json
{
  "id": 218,
  "name": "Object Storage Service",
  "attributes": [
    {"name": "region", "value": "EU-DE"}
  ]
}
```

✅ **Component without attributes**:
```json
{
  "id": 312,
  "name": "Database Service",
  "attributes": []
}
```

### Invalid Response Examples

❌ **Missing required field `id`**:
```json
{
  "name": "Storage",
  "attributes": []
}
```
*Error*: Serde deserialization fails

❌ **Invalid attribute structure**:
```json
{
  "id": 218,
  "name": "Storage",
  "attributes": [
    {"key": "region", "val": "EU-DE"}  // Should be "name" and "value"
  ]
}
```
*Error*: Serde deserialization fails

## Performance Considerations

- **Response Size**: ~100 components × ~200 bytes = ~20 KB (small payload)
- **Frequency**: Once at startup + rare refreshes (only on cache miss)
- **Timeout**: Use 10-second timeout per FR-014
- **Caching**: Store in memory for duration of reporter process

## Security

- **Authentication**: Same HMAC-JWT mechanism as V1 API (FR-008)
- **Data Exposure**: Component names and attributes are public data (Status Dashboard is public)
- **Authorization**: Reporter only needs read access to components endpoint
