//! Status Dashboard integration module
//!
//! This module contains all functionality for integrating with the Status Dashboard API,
//! including component management, incident creation, cache operations, and authentication.

use anyhow;
use hmac::{Hmac, Mac};
use jwt::SignWithKey;
use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};
use serde_json;
use sha2::Sha256;
use std::collections::HashMap;

const CLAIM_PREFERRED_USERNAME: &str = "preferred_username";
const CLAIM_GROUP: &str = "groups";

/// Component attribute (key-value pair) for identifying components
#[derive(Clone, Deserialize, Serialize, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct ComponentAttribute {
    pub name: String,
    pub value: String,
}

/// Component definition from configuration
#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct Component {
    pub name: String,
    pub attributes: Vec<ComponentAttribute>,
}

/// Component status for V1 API (legacy, deprecated - use V2 IncidentData instead)
#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct ComponentStatus {
    pub name: String,
    pub impact: u8,
    pub attributes: Vec<ComponentAttribute>,
}

/// Component data from Status Dashboard API V2 GET /v2/components response
#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct StatusDashboardComponent {
    pub id: u32,
    pub name: String,
    #[serde(default)]
    pub attributes: Vec<ComponentAttribute>,
}

/// Incident data for Status Dashboard API V2 POST request
#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct IncidentData {
    pub title: String,
    pub description: String,
    pub impact: u8,
    pub components: Vec<u32>,
    pub start_date: String,
    pub system: bool,
    #[serde(rename = "type")]
    pub incident_type: String,
}

/// Component ID cache: maps (component_name, sorted_attributes) to component_id
pub type ComponentCache = HashMap<(String, Vec<ComponentAttribute>), u32>;

/// Generate HMAC-JWT authorization headers for Status Dashboard API
///
/// Creates a Bearer token using HMAC-SHA256 signing with the provided secret.
/// Returns empty HeaderMap if no secret is provided (for optional auth environments).
///
/// # Arguments
/// * `secret` - Optional HMAC secret for JWT signing
/// * `preferred_username` - Optional preferred_username claim for JWT
/// * `group` - Optional group claim for JWT (will be placed into "groups" array in JWT payload)
///
/// # Returns
/// HeaderMap with Authorization header if secret provided, empty otherwise
pub fn build_auth_headers(
    secret: Option<&str>,
    preferred_username: Option<&str>,
    group: Option<&str>,
) -> HeaderMap {
    let mut headers = HeaderMap::new();
    if let Some(secret) = secret {
        let key: Hmac<Sha256> = Hmac::new_from_slice(secret.as_bytes()).unwrap();

        // Build claims as a JSON Value to support complex types
        let mut claims_map = serde_json::Map::new();

        // Add preferred_username if provided
        if let Some(username) = preferred_username {
            claims_map.insert(
                CLAIM_PREFERRED_USERNAME.to_string(),
                serde_json::Value::String(username.to_string()),
            );
        }

        // Add group as array if provided (Status Dashboard expects "groups" claim name)
        if let Some(group_value) = group {
            let groups_json = vec![serde_json::Value::String(group_value.to_string())];
            claims_map.insert(
                CLAIM_GROUP.to_string(),
                serde_json::Value::Array(groups_json),
            );
        }

        let claims = serde_json::Value::Object(claims_map);
        let token_str = claims.sign_with_key(&key).unwrap();
        let bearer = format!("Bearer {}", token_str);
        headers.insert(reqwest::header::AUTHORIZATION, bearer.parse().unwrap());
    }
    headers
}

/// Fetch all components from Status Dashboard API V2
pub async fn fetch_components(
    client: &reqwest::Client,
    base_url: &str,
    headers: &HeaderMap,
) -> anyhow::Result<Vec<StatusDashboardComponent>> {
    let url = format!("{}/v2/components", base_url);
    let response = client.get(&url).headers(headers.clone()).send().await?;

    if !response.status().is_success() {
        anyhow::bail!(
            "Failed to fetch components: status={}, body={:?}",
            response.status(),
            response.text().await
        );
    }

    let components: Vec<StatusDashboardComponent> = response.json().await?;
    Ok(components)
}

/// Build component ID cache from fetched components
pub fn build_component_id_cache(components: Vec<StatusDashboardComponent>) -> ComponentCache {
    components
        .into_iter()
        .map(|c| {
            let mut attrs = c.attributes;
            attrs.sort(); // Ensure deterministic key
            ((c.name, attrs), c.id)
        })
        .collect()
}

/// Find component ID in cache with subset attribute matching
/// Returns the component ID if found, None otherwise
pub fn find_component_id(cache: &ComponentCache, target: &Component) -> Option<u32> {
    cache
        .iter()
        .filter(|((name, _attrs), _id)| name == &target.name)
        .find(|((_name, cache_attrs), _id)| {
            // Config attrs must be subset of cache attrs
            target.attributes.iter().all(|target_attr| {
                cache_attrs.iter().any(|cache_attr| {
                    cache_attr.name == target_attr.name && cache_attr.value == target_attr.value
                })
            })
        })
        .map(|((_name, _attrs), id)| *id)
}

/// Build incident data structure for V2 API
/// timestamp: metric timestamp in seconds since epoch
pub fn build_incident_data(component_id: u32, impact: u8, timestamp: i64) -> IncidentData {
    // Convert timestamp to RFC3339 and subtract 1 second per FR-011
    let start_date = chrono::DateTime::from_timestamp(timestamp - 1, 0)
        .expect("Invalid timestamp")
        .to_rfc3339();

    IncidentData {
        title: "System incident from monitoring system".to_string(),
        description:
            "System-wide incident affecting one or multiple components. Created automatically."
                .to_string(),
        impact,
        components: vec![component_id],
        start_date,
        system: true,
        incident_type: "incident".to_string(),
    }
}

/// Create incident via Status Dashboard API V2
pub async fn create_incident(
    client: &reqwest::Client,
    base_url: &str,
    headers: &HeaderMap,
    incident_data: &IncidentData,
) -> anyhow::Result<()> {
    let url = format!("{}/v2/events", base_url);
    let response = client
        .post(&url)
        .headers(headers.clone())
        .json(incident_data)
        .send()
        .await?;

    if !response.status().is_success() {
        anyhow::bail!(
            "Failed to create incident: status={}, body={:?}",
            response.status(),
            response.text().await
        );
    }

    Ok(())
}
