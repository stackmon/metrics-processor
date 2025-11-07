//! cloudmon-metrics-reporter - status-dashboard reporter.
//!
//! Post component status to the CloudMon status-dashboard API.
//!
#![doc(html_no_source)]
use cloudmon_metrics::{api::v1::ServiceHealthResponse, config::Config};

use reqwest::{
    header::{HeaderMap, AUTHORIZATION},
    ClientBuilder,
};

use tokio::signal;
use tokio::time::{sleep, Duration};

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use std::collections::HashMap;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use hmac::{Hmac, Mac};
use jwt::SignWithKey;
use sha2::Sha256;
use std::collections::BTreeMap;

#[derive(Clone, Deserialize, Serialize, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct ComponentAttribute {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct Component {
    pub name: String,
    pub attributes: Vec<ComponentAttribute>,
}

/// Structure for deserializing components from Status Dashboard API v2 (/v2/components).
#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct StatusDashboardComponent {
    pub id: u32,
    pub name: String,
    #[serde(default)]
    pub attributes: Vec<ComponentAttribute>,
}

/// Structure for serializing incident data for Status Dashboard API v2 (/v2/incidents).
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

#[tokio::main]
async fn main() {
    //Enable logging.
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting cloudmon-metrics-reporter");

    // Parse config.
    let config = Config::new("config.yaml").unwrap();

    // Set up CTRL+C handlers.
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

    // Execute metric_watcher unless need to stop.
    tokio::select! {
        _ = metric_watcher(&config) => {},
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Stopped cloudmon-metrics-reporting");
}

/// Fetches components from the Status Dashboard API.
async fn fetch_components(
    req_client: &reqwest::Client,
    components_url: &str,
) -> Result<Vec<StatusDashboardComponent>> {
    let response = req_client.get(components_url).send().await?;
    response.error_for_status_ref()?;
    let components = response.json::<Vec<StatusDashboardComponent>>().await?;
    Ok(components)
}

/// Fetches components, builds, and returns a new component ID cache.
///
/// # Arguments:
/// * `req_client` - The reqwest client;
/// * `components_url` - The URL to fetch components from;
/// * `with_retry` - If true, it will retry fetching up to 3 times on failure.
async fn update_component_cache(
    req_client: &reqwest::Client,
    components_url: &str,
    with_retry: bool,
) -> Result<HashMap<(String, Vec<ComponentAttribute>), u32>> {
    tracing::info!("Updating component cache...");

    let fetch_future = if with_retry {
        fetch_components_with_retry(req_client, components_url).await
    } else {
        fetch_components(req_client, components_url).await.ok()
    };

    match fetch_future {
        Some(components) if !components.is_empty() => {
            let cache = build_component_id_cache(components);
            tracing::info!(
                "Successfully updated component cache. New size: {}",
                cache.len()
            );
            Ok(cache)
        }
        Some(_) => {
            // Components list is empty
            anyhow::bail!("Component list from status-dashboard is empty.")
        }
        None => anyhow::bail!("Failed to fetch component list from status-dashboard."),
    }
}
/// Fetches components with a retry mechanism.
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

/// Builds a cache mapping (ComponentName, Attributes) -> ComponentID.
fn build_component_id_cache(
    components: Vec<StatusDashboardComponent>,
) -> HashMap<(String, Vec<ComponentAttribute>), u32> {
    components
        .into_iter()
        .map(|c| {
            let mut attrs = c.attributes;
            attrs.sort();
            ((c.name, attrs), c.id)
        })
        .collect()
}

async fn metric_watcher(config: &Config) {
    tracing::info!("Starting metric reporter thread");
    // Init reqwest client.
    let req_client: reqwest::Client = ClientBuilder::new()
        .timeout(Duration::from_secs(10 as u64))
        .build()
        .unwrap();

    // This is the logic to build a component lookup table from config.
    let mut components_from_config: HashMap<String, HashMap<String, Component>> = HashMap::new();
    for env in config.environments.iter() {
        let comp_env_entry = components_from_config
            .entry(env.name.clone())
            .or_insert(HashMap::new());
        let mut env_attrs: Vec<ComponentAttribute> = Vec::new();
        if let Some(ref attrs) = env.attributes {
            for (key, val) in attrs.iter() {
                env_attrs.push(ComponentAttribute {
                    name: key.to_string(),
                    value: val.clone(),
                });
            }
        }

        for component_def in config.health_metrics.iter() {
            match component_def.1.component_name {
                Some(ref name) => {
                    comp_env_entry.insert(
                        component_def.0.clone(),
                        Component {
                            name: name.clone(),
                            attributes: env_attrs.clone(),
                        },
                    );
                }
                None => {
                    tracing::warn!("No component_name is given for {}", component_def.1.service);
                }
            }
        }
    }

    let sdb_config = config
        .status_dashboard
        .as_ref()
        .expect("Status dashboard section is missing");

    // Fetch components from Status Dashboard and build a cache to resolve component name to ID.
    let components_url = format!("{}/v2/components", sdb_config.url.clone());
    let mut component_id_cache =
        match update_component_cache(&req_client, &components_url, true).await {
            Ok(cache) => cache,
            Err(e) => {
                tracing::error!(
                    "Initial component cache load failed: {}. Reporter cannot proceed.",
                    e
                );
                return;
            }
        };

    // Prepare for incident reporting
    let incidents_url = format!("{}/v2/incidents", sdb_config.url.clone());
    let mut headers = HeaderMap::new();
    if let Some(ref secret) = sdb_config.secret {
        let key: Hmac<Sha256> = Hmac::new_from_slice(secret.as_bytes()).unwrap();
        let mut claims = BTreeMap::new();
        claims.insert("stackmon", "dummy");
        let token_str = claims.sign_with_key(&key).unwrap();
        let bearer = format!("Bearer {}", token_str);
        headers.insert(AUTHORIZATION, bearer.parse().unwrap());
    }
    loop {
        // For every env from config.
        for env in config.environments.iter() {
            tracing::trace!("env {:?}", env);
            // For every component (health_metric service).
            for component_def in config.health_metrics.iter() {
                tracing::trace!("Component {:?}", component_def.0);
                // Query metric-convertor for the status
                match req_client
                    .get(format!(
                        "http://localhost:{}/api/v1/health",
                        config.server.port
                    ))
                    // Query env/service for time [-2min..-1min]
                    .query(&[
                        ("environment", env.name.clone()),
                        ("service", component_def.0.clone()),
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
                            // Try to parse response.
                            match rsp.json::<ServiceHealthResponse>().await {
                                Ok(mut data) => {
                                    tracing::debug!("response {:?}", data);
                                    // Peek at last metric in the vector.
                                    if let Some(last) = data.metrics.pop() {
                                        // Is metric showing issues?
                                        if last.1 > 0 {
                                            // 0 means OK
                                            tracing::info!("Bad status found: {}", last.1);
                                            let component = components_from_config
                                                .get(&env.name)
                                                .unwrap()
                                                .get(component_def.0)
                                                .unwrap();
                                            tracing::info!("Component to report: {:?}", component);

                                            // Search for component ID in the cache using name and attributes.
                                            let mut search_attrs = component.attributes.clone();
                                            search_attrs.sort();
                                            let cache_key = (component.name.clone(), search_attrs);

                                            let mut component_id =
                                                component_id_cache.get(&cache_key);

                                            // If component not found, refresh cache and try again.
                                            if component_id.is_none() {
                                                tracing::info!(
                                                    "Component '{}' with attributes {:?} not found in cache. Attempting to refresh.",
                                                    component.name, component.attributes
                                                );
                                                match update_component_cache(
                                                    &req_client,
                                                    &components_url,
                                                    false,
                                                )
                                                .await
                                                {
                                                    Ok(new_cache) => {
                                                        component_id_cache = new_cache;
                                                        component_id =
                                                            component_id_cache.get(&cache_key);
                                                    }
                                                    Err(e) => {
                                                        tracing::warn!("Failed to refresh component cache, using old one. Error: {}", e);
                                                    }
                                                }
                                            }

                                            if let Some(id) = component_id {
                                                tracing::info!(
                                                    "Found component ID {} in cache.",
                                                    id
                                                );

                                                // Build IncidentData body for API v2
                                                let body = IncidentData {
                                                    title: "System incident from monitoring system"
                                                        .to_string(),
                                                    description: "System-wide incident affecting multiple components. Created automatically."
                                                        .to_string(),
                                                    impact: last.1,
                                                    components: vec![*id],
                                                    start_date: Utc::now(),
                                                    system: true,
                                                    incident_type: "incident".to_string(),
                                                };
                                                let res = req_client
                                                    .post(&incidents_url)
                                                    .headers(headers.clone())
                                                    .json(&body)
                                                    .send()
                                                    .await;
                                                match res {
                                                    Ok(rsp) => {
                                                        if !rsp.status().is_success() {
                                                            tracing::error!(
                                                                "Error reporting incident: [{}] {:?}",
                                                                rsp.status(),
                                                                rsp.text().await
                                                            );
                                                        } else {
                                                            tracing::info!(
                                                                "Successfully reported incident for component '{}' with attributes {:?}.",
                                                                component.name,
                                                                component.attributes
                                                            );
                                                        }
                                                    }
                                                    Err(e) => {
                                                        tracing::error!(
                                                            "Error during sending post request for incident: {}",
                                                            e
                                                        );
                                                    }
                                                }
                                            } else {
                                                tracing::error!(
                                                    "Component with name '{}' and attributes {:?} still not found in status-dashboard cache after refresh.",
                                                    component.name, component.attributes
                                                );
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
