//! cloudmon-metrics-reporter - status-dashboard reporter.
//!
//! Post component status to the CloudMon status-dashboard API.
//!
#![doc(html_no_source)]

extern crate anyhow;

use cloudmon_metrics::{api::v1::ServiceHealthResponse, config::Config};

use reqwest::{
    header::{HeaderMap, AUTHORIZATION},
    ClientBuilder,
};

use tokio::signal;
use tokio::time::{sleep, Duration};

use serde::{Deserialize, Serialize};

use std::collections::{BTreeMap, HashMap};

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use hmac::{Hmac, Mac};
use jwt::SignWithKey;
use sha2::Sha256;

use chrono;

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

/// Component status for V1 API (legacy, will be replaced)
#[derive(Deserialize, Serialize, Debug)]
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

/// Incident data for Status Dashboard API V2 POST /v2/incidents request
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
type ComponentCache = HashMap<(String, Vec<ComponentAttribute>), u32>;

/// Fetch all components from Status Dashboard API V2
async fn fetch_components(
    client: &reqwest::Client,
    base_url: &str,
    headers: &HeaderMap,
) -> anyhow::Result<Vec<StatusDashboardComponent>> {
    let url = format!("{}/v2/components", base_url);
    let response = client
        .get(&url)
        .headers(headers.clone())
        .send()
        .await?;

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
fn build_component_id_cache(components: Vec<StatusDashboardComponent>) -> ComponentCache {
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
fn find_component_id(cache: &ComponentCache, target: &Component) -> Option<u32> {
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
fn build_incident_data(component_id: u32, impact: u8, timestamp: i64) -> IncidentData {
    // Convert timestamp to RFC3339 and subtract 1 second per FR-011
    let start_date = chrono::DateTime::from_timestamp(timestamp - 1, 0)
        .expect("Invalid timestamp")
        .to_rfc3339();

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

/// Create incident via Status Dashboard API V2
async fn create_incident(
    client: &reqwest::Client,
    base_url: &str,
    headers: &HeaderMap,
    incident_data: &IncidentData,
) -> anyhow::Result<()> {
    let url = format!("{}/v2/incidents", base_url);
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

#[tokio::main]
async fn main() {
    //Enable logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting cloudmon-metrics-reporter");

    // Parse config
    let config = Config::new("config.yaml").unwrap();

    // Set up CTRL+C handlers
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    // Execute metric_watcher unless need to stop
    tokio::select! {
        _ = metric_watcher(&config) => {},
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Stopped cloudmon-metrics-reporting");
}

async fn metric_watcher(config: &Config) {
    tracing::info!("Starting metric reporter thread");
    // Init reqwest client
    let req_client: reqwest::Client = ClientBuilder::new()
        .timeout(Duration::from_secs(10 as u64))
        .build()
        .unwrap();
    // Endless loop
    let mut components: HashMap<String, HashMap<String, Component>> = HashMap::new();
    for env in config.environments.iter() {
        let comp_env_entry = components.entry(env.name.clone()).or_insert(HashMap::new());
        let mut env_attrs: Vec<ComponentAttribute> = Vec::new();
        if let Some(ref attrs) = env.attributes {
            for (key, val) in attrs.iter() {
                env_attrs.push(ComponentAttribute {
                    name: key.to_string(),
                    value: val.clone(),
                });
            }
        }

        for component in config.health_metrics.iter() {
            match component.1.component_name {
                Some(ref name) => {
                    comp_env_entry.insert(
                        component.0.clone(),
                        Component {
                            name: name.clone(),
                            attributes: env_attrs.clone(),
                        },
                    );
                }
                None => {
                    tracing::warn!("No component_name is given for {}", component.1.service);
                }
            }
        }
    }
    let sdb_config = config
        .status_dashboard
        .as_ref()
        .expect("Status dashboard section is missing");

    // Build authorization headers
    let mut headers = HeaderMap::new();
    if let Some(ref secret) = sdb_config.secret {
        let key: Hmac<Sha256> = Hmac::new_from_slice(secret.as_bytes()).unwrap();
        let mut claims = BTreeMap::new();
        claims.insert("stackmon", "dummy");
        let token_str = claims.sign_with_key(&key).unwrap();
        let bearer = format!("Bearer {}", token_str);
        headers.insert(AUTHORIZATION, bearer.parse().unwrap());
    }

    // Initialize component ID cache (TODO: Phase 4 - add retry logic)
    let component_cache = match fetch_components(&req_client, &sdb_config.url, &headers).await {
        Ok(components) => {
            tracing::info!("Fetched {} components from Status Dashboard", components.len());
            build_component_id_cache(components)
        }
        Err(e) => {
            tracing::error!("Failed to fetch components at startup: {}", e);
            return;
        }
    };

    loop {
        // For every env from config
        for env in config.environments.iter() {
            tracing::trace!("env {:?}", env);
            // For every component (health_metric service)
            for component in config.health_metrics.iter() {
                tracing::trace!("Component {:?}", component.0);
                // Query metric-convertor for the status
                match req_client
                    .get(format!(
                        "http://localhost:{}/api/v1/health",
                        config.server.port
                    ))
                    // Query env/service for time [-5min..-2min]
                    .query(&[
                        ("environment", env.name.clone()),
                        ("service", component.0.clone()),
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
                            // Try to parse response
                            match rsp.json::<ServiceHealthResponse>().await {
                                Ok(mut data) => {
                                    tracing::debug!("response {:?}", data);
                                    // Peek at last metric in the vector
                                    if let Some(last) = data.metrics.pop() {
                                        // Is metric showing issues?
                                        if last.1 > 0 {
                                            let comp = components
                                                .get(&env.name)
                                                .unwrap()
                                                .get(component.0)
                                                .unwrap();

                                            // Find component ID in cache
                                            match find_component_id(&component_cache, comp) {
                                                Some(component_id) => {
                                                    // Build incident data
                                                    let incident_data = build_incident_data(
                                                        component_id,
                                                        last.1,
                                                        last.0 as i64,
                                                    );

                                                    // Structured logging with diagnostic fields (FR-017)
                                                    tracing::info!(
                                                        timestamp = last.0,
                                                        service = component.0.as_str(),
                                                        environment = env.name.as_str(),
                                                        component_name = comp.name.as_str(),
                                                        component_id = component_id,
                                                        impact = last.1,
                                                        "Creating incident for health issue"
                                                    );

                                                    // Create incident via V2 API
                                                    match create_incident(
                                                        &req_client,
                                                        &sdb_config.url,
                                                        &headers,
                                                        &incident_data,
                                                    ).await {
                                                        Ok(_) => {
                                                            tracing::info!(
                                                                component_id = component_id,
                                                                impact = last.1,
                                                                "Incident created successfully"
                                                            );
                                                        }
                                                        Err(e) => {
                                                            // Error logging with details (FR-015)
                                                            tracing::error!(
                                                                error = %e,
                                                                component_id = component_id,
                                                                service = component.0.as_str(),
                                                                environment = env.name.as_str(),
                                                                "Failed to create incident"
                                                            );
                                                        }
                                                    }
                                                }
                                                None => {
                                                    tracing::warn!(
                                                        component_name = comp.name.as_str(),
                                                        service = component.0.as_str(),
                                                        environment = env.name.as_str(),
                                                        "Component not found in cache, skipping incident creation"
                                                    );
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
                        tracing::error!("Error: {}", e);
                    }
                }
            }
        }
        // Sleep for some time
        sleep(Duration::from_secs(60)).await;
    }
}
