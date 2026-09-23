# Quickstart: Status Dashboard API V2 Migration

**Feature**: Reporter Migration to Status Dashboard API V2  
**Branch**: `003-sd-api-v2-migration`  
**Date**: 2025-01-23

## Overview

This guide provides a quickstart for implementing the Status Dashboard API V2 migration. The migration replaces the V1 component status endpoint with V2 incident creation and adds component ID caching.

**Key Changes**:
- ✅ Component ID cache at startup (with retry)
- ✅ New incident structure with static title/description
- ✅ Structured diagnostic logging (not sent to API)
- ✅ Authorization unchanged (HMAC-JWT)

---

## Prerequisites

### 1. Dependencies

Add `anyhow` crate for error handling:

```toml
# Cargo.toml
[dependencies]
anyhow = "~1.0"
chrono = "~0.4"  # Already present
serde = { version = "~1.0", features = ["derive"] }  # Already present
serde_json = "~1.0"  # Already present
reqwest = { version = "~0.11", default-features = false, features = ["rustls-tls", "json"] }  # Already present
```

### 2. Status Dashboard Requirements

- Status Dashboard must be running with V2 API endpoints available
- All monitored components must be registered in Status Dashboard
- Component names and attributes in config must match Status Dashboard exactly (or be subsets)

### 3. Configuration

No configuration changes required. Existing `config.yaml` is compatible:

```yaml
status_dashboard:
  url: "https://status-dashboard.example.com"
  jwt_secret: "your-hmac-secret"  # Optional, for auth

environments:
  - name: production
    attributes:
      region: "EU-DE"
      category: "Storage"

health_metrics:
  swift:
    component_name: "Object Storage Service"
    # ... other health metric config
```

---

## Implementation Steps

### Step 1: Define Data Structures

The Status Dashboard integration is consolidated in `src/sd.rs` library module. Add/update these structs:

```rust
// src/sd.rs - Status Dashboard integration module

use anyhow;
use hmac::{Hmac, Mac};
use jwt::SignWithKey;
use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::{BTreeMap, HashMap};

// Update ComponentAttribute to support sorting and hashing
#[derive(Clone, Deserialize, Serialize, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct ComponentAttribute {
    pub name: String,
    pub value: String,
}

// Existing Component struct (no changes needed)
#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct Component {
    pub name: String,
    pub attributes: Vec<ComponentAttribute>,
}

// NEW: API response from GET /v2/components
#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct StatusDashboardComponent {
    pub id: u32,
    pub name: String,
    #[serde(default)]
    pub attributes: Vec<ComponentAttribute>,
}

// NEW: API request for POST /v2/incidents
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

// Component ID cache type
type ComponentCache = HashMap<(String, Vec<ComponentAttribute>), u32>;
```

### Step 2: Implement Component Fetching

```rust
/// Fetch components from Status Dashboard API
async fn fetch_components(
    req_client: &reqwest::Client,
    components_url: &str,
) -> Result<Vec<StatusDashboardComponent>> {
    let response = req_client.get(components_url).send().await?;
    response.error_for_status_ref()?;
    let components = response.json::<Vec<StatusDashboardComponent>>().await?;
    Ok(components)
}

/// Fetch components with retry logic (3 attempts, 60s delays)
async fn fetch_components_with_retry(
    req_client: &reqwest::Client,
    components_url: &str,
) -> Option<Vec<StatusDashboardComponent>> {
    let mut attempts = 0;
    loop {
        match fetch_components(req_client, components_url).await {
            Ok(components) => {
                tracing::info!("Successfully fetched {} components.", components.len());
                return Some(components);
            }
            Err(e) => {
                attempts += 1;
                tracing::error!("Failed to fetch components (attempt {}/3): {}", attempts, e);
                if attempts >= 3 {
                    tracing::error!("Could not fetch components after 3 attempts. Giving up.");
                    return None;
                }
                tracing::info!("Retrying in 60 seconds...");
                sleep(Duration::from_secs(60)).await;
            }
        }
    }
}
```

### Step 3: Implement Cache Building

```rust
/// Build component ID cache from fetched components
fn build_component_id_cache(
    components: Vec<StatusDashboardComponent>,
) -> ComponentCache {
    components
        .into_iter()
        .map(|c| {
            let mut attrs = c.attributes;
            attrs.sort();  // Ensure deterministic cache keys
            ((c.name, attrs), c.id)
        })
        .collect()
}

/// Update cache (with optional retry on startup)
async fn update_component_cache(
    req_client: &reqwest::Client,
    components_url: &str,
    with_retry: bool,
) -> Result<ComponentCache> {
    tracing::info!("Updating component cache...");

    let fetch_future = if with_retry {
        fetch_components_with_retry(req_client, components_url).await
    } else {
        fetch_components(req_client, components_url).await.ok()
    };

    match fetch_future {
        Some(components) if !components.is_empty() => {
            let cache = build_component_id_cache(components);
            tracing::info!("Successfully updated component cache. New size: {}", cache.len());
            Ok(cache)
        }
        Some(_) => {
            anyhow::bail!("Component list from status-dashboard is empty.")
        }
        None => anyhow::bail!("Failed to fetch component list from status-dashboard."),
    }
}
```

### Step 4: Implement Component Lookup with Subset Matching

```rust
/// Find component ID in cache with subset attribute matching
fn find_component_id(
    cache: &ComponentCache,
    target: &Component,
) -> Option<u32> {
    // Iterate cache to find matching component
    cache.iter()
        .filter(|((name, _attrs), _id)| name == &target.name)
        .find(|((_name, cache_attrs), _id)| {
            // Config attrs must be subset of cache attrs (FR-012)
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

### Step 5: Implement Incident Creation

```rust
/// Build incident data from health metric
fn build_incident_data(
    component_id: u32,
    impact: u8,
    timestamp: i64,
) -> IncidentData {
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

/// Create incident via API
async fn create_incident(
    req_client: &reqwest::Client,
    incidents_url: &str,
    headers: &HeaderMap,
    incident: &IncidentData,
) -> Result<()> {
    let response = req_client
        .post(incidents_url)
        .headers(headers.clone())
        .json(incident)
        .send()
        .await?;
    
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await?;
        tracing::error!("Incident creation failed [{}]: {}", status, body);
        return Err(anyhow::anyhow!("API error: {} - {}", status, body));
    }
    
    tracing::info!("Incident created successfully");
    Ok(())
}
```

### Step 6: Update metric_watcher Function

Replace the monitoring loop in `metric_watcher()`:

```rust
async fn metric_watcher(config: &Config) {
    tracing::info!("Starting metric reporter thread");
    
    let req_client: reqwest::Client = ClientBuilder::new()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    
    // Build component lookup table from config (unchanged)
    let mut components_from_config: HashMap<String, HashMap<String, Component>> = HashMap::new();
    for env in config.environments.iter() {
        // ... existing component building logic ...
    }
    
    // Status Dashboard configuration
    let sdb_config = config
        .status_dashboard
        .as_ref()
        .expect("Status dashboard section is missing");
    
    // NEW: V2 endpoints
    let components_url = format!("{}/v2/components", sdb_config.url);
    let incidents_url = format!("{}/v2/incidents", sdb_config.url);
    
    // Setup authorization headers (unchanged)
    let mut headers = HeaderMap::new();
    if let Some(ref secret) = sdb_config.secret {
        let key: Hmac<Sha256> = Hmac::new_from_slice(secret.as_bytes()).unwrap();
        let mut claims = BTreeMap::new();
        claims.insert("stackmon", "dummy");
        let token_str = claims.sign_with_key(&key).unwrap();
        let bearer = format!("Bearer {}", token_str);
        headers.insert(AUTHORIZATION, bearer.parse().unwrap());
    }
    
    // NEW: Load component cache at startup with retry (FR-006, FR-007)
    let mut component_cache = update_component_cache(&req_client, &components_url, true)
        .await
        .expect("Failed to load component cache. Reporter cannot start.");
    
    tracing::info!("Component cache loaded with {} entries", component_cache.len());
    
    // Monitoring loop
    loop {
        for env in config.environments.iter() {
            for (service_name, _component_config) in config.health_metrics.iter() {
                // Query health API (unchanged)
                match req_client
                    .get(format!("http://localhost:{}/api/v1/health", config.server.port))
                    .query(&[
                        ("environment", env.name.clone()),
                        ("service", service_name.clone()),
                        ("from", "-5min".to_string()),
                        ("to", "-2min".to_string()),
                    ])
                    .send()
                    .await
                {
                    Ok(rsp) => {
                        if rsp.status().is_client_error() {
                            tracing::error!("Got API error {:?}", rsp.text().await);
                        } else {
                            match rsp.json::<ServiceHealthResponse>().await {
                                Ok(mut data) => {
                                    if let Some((timestamp, impact)) = data.metrics.pop() {
                                        if impact > 0 {
                                            // Get component from config
                                            let component = components_from_config
                                                .get(&env.name)
                                                .and_then(|env_map| env_map.get(service_name))
                                                .expect("Component not found in config");
                                            
                                            // NEW: Look up component ID in cache
                                            let component_id = match find_component_id(&component_cache, component) {
                                                Some(id) => id,
                                                None => {
                                                    // Cache miss: refresh and retry (FR-005)
                                                    tracing::warn!("Component not found in cache: {} {:?}", component.name, component.attributes);
                                                    tracing::info!("Refreshing component cache...");
                                                    
                                                    match update_component_cache(&req_client, &components_url, false).await {
                                                        Ok(new_cache) => {
                                                            component_cache = new_cache;
                                                            match find_component_id(&component_cache, component) {
                                                                Some(id) => id,
                                                                None => {
                                                                    tracing::warn!("Component still not found after cache refresh: {} {:?}", component.name, component.attributes);
                                                                    continue;  // Skip incident creation
                                                                }
                                                            }
                                                        }
                                                        Err(e) => {
                                                            tracing::error!("Failed to refresh cache: {}", e);
                                                            continue;  // Skip incident creation
                                                        }
                                                    }
                                                }
                                            };
                                            
                                            // NEW: Log diagnostic details (FR-017)
                                            tracing::info!(
                                                timestamp = timestamp,
                                                service = %service_name,
                                                environment = %env.name,
                                                component_name = %component.name,
                                                component_attrs = ?component.attributes,
                                                component_id = component_id,
                                                impact = impact,
                                                "Creating incident for health issue"
                                            );
                                            
                                            // NEW: Build and create incident
                                            let incident = build_incident_data(component_id, impact, timestamp);
                                            
                                            match create_incident(&req_client, &incidents_url, &headers, &incident).await {
                                                Ok(_) => {
                                                    tracing::info!("Incident reported successfully");
                                                }
                                                Err(e) => {
                                                    tracing::error!("Failed to create incident: {}", e);
                                                    // Continue to next service (FR-015)
                                                }
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::error!("Cannot process response: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Error querying health API: {}", e);
                    }
                }
            }
        }
        
        // Sleep between monitoring cycles
        sleep(Duration::from_secs(60)).await;
    }
}
```

---

## Testing

### Unit Tests

Add to `src/bin/reporter.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_component_id_cache() {
        let components = vec![
            StatusDashboardComponent {
                id: 218,
                name: "Storage".to_string(),
                attributes: vec![
                    ComponentAttribute { name: "region".to_string(), value: "EU-DE".to_string() },
                    ComponentAttribute { name: "category".to_string(), value: "Storage".to_string() },
                ],
            },
        ];
        
        let cache = build_component_id_cache(components);
        
        // Attributes should be sorted in cache key
        let key = (
            "Storage".to_string(),
            vec![
                ComponentAttribute { name: "category".to_string(), value: "Storage".to_string() },
                ComponentAttribute { name: "region".to_string(), value: "EU-DE".to_string() },
            ],
        );
        
        assert_eq!(cache.get(&key), Some(&218));
    }

    #[test]
    fn test_find_component_id_exact_match() {
        let mut cache = ComponentCache::new();
        cache.insert(
            (
                "Storage".to_string(),
                vec![ComponentAttribute { name: "region".to_string(), value: "EU-DE".to_string() }],
            ),
            218,
        );
        
        let component = Component {
            name: "Storage".to_string(),
            attributes: vec![ComponentAttribute { name: "region".to_string(), value: "EU-DE".to_string() }],
        };
        
        assert_eq!(find_component_id(&cache, &component), Some(218));
    }

    #[test]
    fn test_find_component_id_subset_match() {
        let mut cache = ComponentCache::new();
        cache.insert(
            (
                "Storage".to_string(),
                vec![
                    ComponentAttribute { name: "category".to_string(), value: "Storage".to_string() },
                    ComponentAttribute { name: "region".to_string(), value: "EU-DE".to_string() },
                ],
            ),
            218,
        );
        
        // Config has only region (subset of cache)
        let component = Component {
            name: "Storage".to_string(),
            attributes: vec![ComponentAttribute { name: "region".to_string(), value: "EU-DE".to_string() }],
        };
        
        assert_eq!(find_component_id(&cache, &component), Some(218));
    }

    #[test]
    fn test_find_component_id_no_match() {
        let mut cache = ComponentCache::new();
        cache.insert(
            ("Storage".to_string(), vec![]),
            218,
        );
        
        let component = Component {
            name: "Compute".to_string(),
            attributes: vec![],
        };
        
        assert_eq!(find_component_id(&cache, &component), None);
    }
}
```

### Integration Tests

Create `tests/reporter_v2_integration.rs`:

```rust
use mockito::{Mock, Server};
use cloudmon_metrics::config::Config;

#[tokio::test]
async fn test_fetch_components_success() {
    let mut server = Server::new_async().await;
    
    let mock = server.mock("GET", "/v2/components")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"[
            {
                "id": 218,
                "name": "Storage",
                "attributes": [{"name": "region", "value": "EU-DE"}]
            }
        ]"#)
        .create_async()
        .await;
    
    let client = reqwest::Client::new();
    let url = format!("{}/v2/components", server.url());
    
    let components = fetch_components(&client, &url).await.unwrap();
    
    assert_eq!(components.len(), 1);
    assert_eq!(components[0].id, 218);
    mock.assert_async().await;
}

#[tokio::test]
async fn test_create_incident_success() {
    let mut server = Server::new_async().await;
    
    let mock = server.mock("POST", "/v2/incidents")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"result": [{"component_id": 218, "incident_id": 456}]}"#)
        .create_async()
        .await;
    
    let client = reqwest::Client::new();
    let url = format!("{}/v2/incidents", server.url());
    let headers = HeaderMap::new();
    
    let incident = IncidentData {
        title: "Test".to_string(),
        description: "Test".to_string(),
        impact: 2,
        components: vec![218],
        start_date: chrono::Utc::now(),
        system: true,
        incident_type: "incident".to_string(),
    };
    
    let result = create_incident(&client, &url, &headers, &incident).await;
    
    assert!(result.is_ok());
    mock.assert_async().await;
}
```

Run tests:
```bash
cargo test
```

---

## Verification

### 1. Check Component Cache Loading

Start the reporter and verify logs:

```bash
RUST_LOG=info cargo run --bin cloudmon-metrics-reporter
```

Expected output:
```
INFO Updating component cache...
INFO Successfully fetched 100 components.
INFO Successfully updated component cache. New size: 100
INFO Component cache loaded with 100 entries
INFO Starting metric reporter thread
```

### 2. Trigger an Incident

Create a health issue and check logs:

```
INFO Creating incident for health issue timestamp=1706000120 service="swift" environment="production" component_name="Object Storage Service" component_attrs=[ComponentAttribute { name: "region", value: "EU-DE" }] component_id=218 impact=2
INFO Incident created successfully
INFO Incident reported successfully
```

### 3. Verify in Status Dashboard

Check Status Dashboard UI:
- Incident should appear with title "System incident from monitoring system"
- `system` flag should be true
- Impact level should match health metric
- Component should be correctly associated

### 4. Test Cache Refresh

1. Add a new component to Status Dashboard
2. Update config to reference new component
3. Trigger health issue for new component
4. Verify logs show cache refresh:

```
WARN Component not found in cache: "New Service" [...]
INFO Refreshing component cache...
INFO Successfully updated component cache. New size: 101
INFO Creating incident for health issue ... component_id=350 ...
```

---

## Troubleshooting

### Issue: "Failed to load component cache. Reporter cannot start."

**Cause**: Cannot fetch components from Status Dashboard (network error, auth issue, or API unavailable)

**Solution**:
1. Check Status Dashboard URL in config
2. Verify Status Dashboard is running and `/v2/components` endpoint is accessible
3. Check authentication secret if configured
4. Review logs for specific error messages

### Issue: "Component not found in cache" (repeated)

**Cause**: Component name or attributes in config don't match Status Dashboard

**Solution**:
1. Check component name spelling in config
2. Verify attributes match exactly (or are subset of) Status Dashboard
3. Check Status Dashboard API response: `curl https://status-dashboard/v2/components`
4. Ensure component is registered in Status Dashboard

### Issue: "Incident creation failed [404]"

**Cause**: Component ID doesn't exist in Status Dashboard

**Solution**:
1. Verify component exists: `curl https://status-dashboard/v2/components/{id}`
2. Check cache is up-to-date
3. Manually trigger cache refresh by restarting reporter

### Issue: "Incident creation failed [400]"

**Cause**: Invalid incident data (impact out of range, missing required fields, invalid date format)

**Solution**:
1. Check health metric returns valid impact (0-3)
2. Verify timestamp is valid Unix epoch seconds
3. Review incident payload in error logs
4. Validate against OpenAPI schema in `/openapi.yaml`

---

## Next Steps

After implementing the migration:

1. **Update Documentation**: Update project docs in `doc/` to reflect V2 usage
2. **Add Monitoring**: Set up alerts for component cache failures or incident creation errors
3. **Performance Tuning**: Monitor HTTP timeout usage; adjust if needed
4. **Decommission V1**: After validation period, remove V1 endpoint usage (if not needed elsewhere)

---

## Reference

- **Feature Spec**: `specs/003-sd-api-v2-migration/spec.md`
- **Research**: `specs/003-sd-api-v2-migration/research.md`
- **Data Model**: `specs/003-sd-api-v2-migration/data-model.md`
- **API Contracts**: `specs/003-sd-api-v2-migration/contracts/`
- **OpenAPI Schema**: `/openapi.yaml`
- **Reference Implementation**: `sd_api_v2_migration` branch
