//! Status Dashboard integration module
//!
//! This module contains all functionality for integrating with the Status Dashboard API,
//! including component management, incident creation, cache operations, and authentication.

use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::config::OidcIdentity;
use crate::oidc::fetch_service_token;

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

/// D6: a token is requested for every authenticated call, never cached in-process, and a failed
/// request must not degrade into an unauthenticated one.
pub async fn build_auth_headers(identity: &OidcIdentity) -> anyhow::Result<HeaderMap> {
    let token = fetch_service_token(identity).await?;
    let mut headers = HeaderMap::new();
    let value = HeaderValue::from_str(&format!("Bearer {}", token))?;
    headers.insert(reqwest::header::AUTHORIZATION, value);
    Ok(headers)
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

pub fn merge_component_attributes(
    environment_attributes: &[ComponentAttribute],
    overrides: Option<&HashMap<String, String>>,
) -> Vec<ComponentAttribute> {
    let mut merged = environment_attributes.to_vec();
    let overrides = match overrides {
        Some(overrides) => overrides,
        None => return merged,
    };

    for (name, value) in overrides {
        let attribute = ComponentAttribute {
            name: name.clone(),
            value: value.clone(),
        };
        match merged.iter().position(|attr| attr.name == attribute.name) {
            Some(index) => merged[index] = attribute,
            None => merged.push(attribute),
        }
    }

    merged.sort();
    merged
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

#[cfg(test)]
mod tests {
    use super::*;

    fn attribute(name: &str, value: &str) -> ComponentAttribute {
        ComponentAttribute {
            name: name.to_string(),
            value: value.to_string(),
        }
    }

    #[test]
    fn merge_keeps_the_environment_attributes_without_overrides() {
        let environment = vec![attribute("region", "EU-DE")];
        let merged = merge_component_attributes(&environment, None);
        assert_eq!(merged, vec![attribute("region", "EU-DE")]);
    }

    #[test]
    fn merge_replaces_an_existing_attribute() {
        let environment = vec![attribute("region", "EU-DE")];
        let overrides = HashMap::from([("region".to_string(), "Global".to_string())]);
        let merged = merge_component_attributes(&environment, Some(&overrides));
        assert_eq!(merged, vec![attribute("region", "Global")]);
    }

    #[test]
    fn merge_adds_an_attribute_missing_from_the_environment() {
        let environment = vec![attribute("region", "EU-DE")];
        let overrides = HashMap::from([("url".to_string(), "https://example.com".to_string())]);
        let merged = merge_component_attributes(&environment, Some(&overrides));
        assert_eq!(
            merged,
            vec![
                attribute("region", "EU-DE"),
                attribute("url", "https://example.com")
            ]
        );
    }
}
