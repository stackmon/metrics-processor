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

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use std::collections::HashMap;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use anyhow::Result;
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

#[derive(Deserialize, Serialize, Debug)]
pub struct ComponentStatus {
    pub name: String,
    pub impact: u8,
    pub attributes: Vec<ComponentAttribute>,
}

/// Structure to run GET /v2/components (API v2)
/// Handler returns list of components (ID, Name and attrs)
#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct StatusDashboardComponent {
    #[serde(rename = "ID")]
    pub id: u32,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Attributes")]
    pub attributes: Option<Vec<ComponentAttribute>>,
}

/// Structure to POST /v2/incidents (API v2)
#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct IncidentData {
    pub title: String,
    pub impact: u8,
    pub components: Vec<u32>,
    pub start_date: DateTime<Utc>,
    pub system: bool,
    #[serde(rename = "type")]
    pub incident_type: String,
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
        .timeout(Duration::from_secs(2 as u64))
        .build()
        .unwrap();

    // Receiving and caching components
    let sdb_config = config
        .status_dashboard
        .as_ref()
        .expect("Status dashboard section is missing");

    let components_url = format!("{}/v2/components", sdb_config.url.clone());
    tracing::info!("Fetching components from {}", components_url);

    let mut component_id_cache = match fetch_components(&req_client, &components_url).await {
        Ok(components) => {
            if components.is_empty() {
                tracing::error!(
                    "Component list from status-dashboard is empty. Reporter cannot proceed."
                );
                return;
            }
            build_component_cache(components)
        }
        Err(e) => {
            tracing::error!(
                "Failed to fetch initial component list: {}. Reporter cannot proceed.",
                e
            );
            return;
        }
    };
    tracing::info!(
        "Successfully cached {} components from status-dashboard.",
        component_id_cache.len()
    );

    // Endless loop
    let mut last_cache_update = Utc::now();
    let cache_ttl = chrono::Duration::hours(1);

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
    let status_report_url = format!("{}/v2/incidents", sdb_config.url.clone());
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
        // Checking the time to refresh the cache
        if Utc::now().signed_duration_since(last_cache_update) > cache_ttl {
            tracing::info!("Component cache TTL expired. Refreshing...");
            match fetch_components(&req_client, &components_url).await {
                Ok(components) if !components.is_empty() => {
                    component_id_cache = build_component_cache(components);
                    last_cache_update = Utc::now();
                    tracing::info!(
                        "Successfully refreshed component cache. New size: {}",
                        component_id_cache.len()
                    );
                }
                _ => tracing::warn!("Failed to refresh component cache or list is empty."),
            };
        }

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
                    // Query env/service for time [-2min..-1min]
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
                                            tracing::info!("Bad status found: {}", last.1);
                                            let component = components
                                                .get(&env.name)
                                                .unwrap()
                                                .get(component.0)
                                                .unwrap();
                                            tracing::info!("Component to report: {:?}", component);

                                            // Searching component's ID in the cache
                                            let mut search_attrs = component.attributes.clone();
                                            search_attrs.sort();
                                            let cache_key = (component.name.clone(), search_attrs);

                                            if let Some(component_id) =
                                                component_id_cache.get(&cache_key)
                                            {
                                                tracing::info!(
                                                    "Found component ID {} in cache.",
                                                    component_id
                                                );

                                                // IncidentData's body building
                                                let body = IncidentData {
                                                    title: format!(
                                                        "Automatic incident for {}",
                                                        component.name
                                                    ),
                                                    impact: last.1,
                                                    components: vec![*component_id],
                                                    start_date: Utc::now(),
                                                    system: true,
                                                    incident_type: "incident".to_string(),
                                                };

                                                let res = req_client
                                                    .post(&status_report_url)
                                                    .headers(headers.clone())
                                                    .json(&body)
                                                    .send()
                                                    .await;
                                                match res {
                                                    Ok(rsp) => {
                                                        if rsp.status().is_client_error() {
                                                            tracing::error!(
                                                                "Error: [{}] {:?}",
                                                                rsp.status(),
                                                                rsp.text().await
                                                            );
                                                        }
                                                    }

                                                    Err(e) => {
                                                        tracing::error!(
                                                        "Error during posting component status: {}",
                                                        e
                                                    );
                                                    }
                                                }
                                            } else {
                                                tracing::error!(
                                                    "Component with name '{}' and attributes {:?} not found in status-dashboard cache.",
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

/// HTTP request and deserialization of the component list
async fn fetch_components(
    req_client: &reqwest::Client,
    components_url: &str,
) -> Result<Vec<StatusDashboardComponent>, anyhow::Error> {
    let response = req_client.get(components_url).send().await.map_err(|e| {
        tracing::error!("Request to fetch components failed: {}", e);
        anyhow::Error::new(e)
    })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_else(|_| "N/A".to_string());
        let err_msg = format!(
            "Failed to fetch components. Status: {}, Body: {:?}",
            status, body
        );
        tracing::error!("{}", err_msg);
        return Err(anyhow::anyhow!(err_msg));
    }

    response
        .json::<Vec<StatusDashboardComponent>>()
        .await
        .map_err(|e| {
            tracing::error!("Failed to parse components from status-dashboard: {}", e);
            anyhow::Error::new(e)
        })
}

/// Creating a cache from a vector of components
fn build_component_cache(
    components: Vec<StatusDashboardComponent>,
) -> HashMap<(String, Vec<ComponentAttribute>), u32> {
    components
        .into_iter()
        .map(|c| {
            let mut attrs = c.attributes.unwrap_or_default();
            attrs.sort();
            ((c.name, attrs), c.id)
        })
        .collect()
}
