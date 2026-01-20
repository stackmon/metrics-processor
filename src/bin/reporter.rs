//! cloudmon-metrics-reporter - status-dashboard reporter.
//!
//! Post component status to the CloudMon status-dashboard API.
//!
#![doc(html_no_source)]
use cloudmon_metrics::{
    api::v1::ServiceHealthResponse,
    config::{Config, StatusDashboardConfig},
    types::{Component, ComponentAttribute, IncidentData, StatusDashboardComponent},
};

use reqwest::{
    header::{HeaderMap, AUTHORIZATION},
    ClientBuilder,
};

use tokio::signal;
use tokio::time::{sleep, Duration};

use anyhow::{Context, Result};
use chrono::{TimeZone, Utc};
use serde_json::json;

use std::collections::HashMap;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use hmac::{Hmac, Mac};
use jwt::SignWithKey;
use sha2::Sha256;
use std::collections::BTreeMap;

/// Component ID cache type alias for clarity
type ComponentIdCache = HashMap<(String, Vec<ComponentAttribute>), u32>;

/// Context for the reporter containing shared state and configuration.
struct ReporterContext<'a> {
    req_client: &'a reqwest::Client,
    config: &'a Config,
    sdb_config: &'a StatusDashboardConfig,
    components_url: &'a str,
    incidents_url: &'a str,
    headers: &'a HeaderMap,
}

#[tokio::main]
async fn main() {
    // Enable logging.
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting cloudmon-metrics-reporter");

    // Parse config.
    let config = match Config::new("config.yaml") {
        Ok(cfg) => cfg,
        Err(e) => {
            tracing::error!("Failed to load configuration: {}", e);
            return;
        }
    };

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
        result = metric_watcher(&config) => {
            if let Err(e) = result {
                tracing::error!("Metric watcher failed: {}", e);
            }
        },
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
/// * `retry_count` - Number of retry attempts (0 means no retries);
/// * `retry_delay` - Delay between retries in seconds.
async fn update_component_cache(
    req_client: &reqwest::Client,
    components_url: &str,
    retry_count: u32,
    retry_delay: u64,
) -> Result<ComponentIdCache> {
    tracing::info!("Updating component cache...");

    let fetch_result = if retry_count > 0 {
        fetch_components_with_retry(req_client, components_url, retry_count, retry_delay).await
    } else {
        fetch_components(req_client, components_url).await.ok()
    };

    match fetch_result {
        Some(components) if !components.is_empty() => {
            let cache = build_component_id_cache(components);
            tracing::info!(
                "Successfully updated component cache. New size: {}",
                cache.len()
            );
            Ok(cache)
        }
        Some(_) => {
            anyhow::bail!("Component list from status-dashboard is empty.")
        }
        None => anyhow::bail!("Failed to fetch component list from status-dashboard."),
    }
}

/// Fetches components with a retry mechanism.
async fn fetch_components_with_retry(
    req_client: &reqwest::Client,
    components_url: &str,
    max_attempts: u32,
    retry_delay: u64,
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
                tracing::error!(
                    "Failed to fetch components (attempt {}/{}): {}",
                    attempts,
                    max_attempts,
                    e
                );
                if attempts >= max_attempts {
                    tracing::error!(
                        "Could not fetch components after {} attempts. Giving up.",
                        max_attempts
                    );
                    return None;
                }
                tracing::info!("Retrying in {} seconds...", retry_delay);
                sleep(Duration::from_secs(retry_delay)).await;
            }
        }
    }
}

/// Builds a cache mapping (ComponentName, Attributes) -> ComponentID.
fn build_component_id_cache(components: Vec<StatusDashboardComponent>) -> ComponentIdCache {
    components
        .into_iter()
        .map(|c| {
            let mut attrs = c.attributes;
            attrs.sort();
            ((c.name, attrs), c.id)
        })
        .collect()
}

/// Builds a component lookup table from config.
fn build_components_from_config(config: &Config) -> HashMap<String, HashMap<String, Component>> {
    let mut components_from_config: HashMap<String, HashMap<String, Component>> = HashMap::new();

    for env in config.environments.iter() {
        let comp_env_entry = components_from_config
            .entry(env.name.clone())
            .or_default();

        let env_attrs: Vec<ComponentAttribute> = env
            .attributes
            .as_ref()
            .map(|attrs| {
                attrs
                    .iter()
                    .map(|(key, val)| ComponentAttribute {
                        name: key.to_string(),
                        value: val.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        for (service_name, health_def) in config.health_metrics.iter() {
            if let Some(ref name) = health_def.component_name {
                comp_env_entry.insert(
                    service_name.clone(),
                    Component {
                        name: name.clone(),
                        attributes: env_attrs.clone(),
                    },
                );
            } else {
                tracing::warn!("No component_name is given for {}", health_def.service);
            }
        }
    }

    components_from_config
}

/// Creates authorisation headers with JWT token if secret is provided.
fn create_auth_headers(secret: Option<&String>) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();

    if let Some(secret) = secret {
        let key: Hmac<Sha256> = Hmac::new_from_slice(secret.as_bytes())
            .context("Failed to create HMAC key from secret")?;
        let mut claims = BTreeMap::new();
        claims.insert("stackmon", "dummy");
        let token_str = claims
            .sign_with_key(&key)
            .context("Failed to sign JWT token")?;
        let bearer = format!("Bearer {}", token_str);
        headers.insert(
            AUTHORIZATION,
            bearer
                .parse()
                .context("Failed to parse authorization header")?,
        );
    }

    Ok(headers)
}

/// Finds component ID in the cache by name and required attributes.
fn find_component_id(
    cache: &ComponentIdCache,
    component_name: &str,
    required_attrs: &[ComponentAttribute],
) -> Option<u32> {
    cache
        .iter()
        .find(|((name, attrs), _id)| {
            name == component_name && required_attrs.iter().all(|r| attrs.contains(r))
        })
        .map(|(_, id)| *id)
}

/// Queries the health API for a specific environment and service.
async fn query_health_api(
    req_client: &reqwest::Client,
    port: u16,
    env_name: &str,
    service_name: &str,
    query_from: &str,
    query_to: &str,
) -> Result<ServiceHealthResponse> {
    let response = req_client
        .get(format!("http://localhost:{}/api/v1/health", port))
        .query(&[
            ("environment", env_name),
            ("service", service_name),
            ("from", query_from),
            ("to", query_to),
        ])
        .send()
        .await
        .context("Failed to send health API request")?;

    if response.status().is_client_error() {
        let error_text = response.text().await.unwrap_or_default();
        anyhow::bail!("Health API client error: {}", error_text);
    }

    response
        .json::<ServiceHealthResponse>()
        .await
        .context("Failed to parse health API response")
}

/// Reports an incident to the Status Dashboard API.
async fn report_incident(
    req_client: &reqwest::Client,
    incidents_url: &str,
    headers: &HeaderMap,
    incident: &IncidentData,
) -> Result<()> {
    tracing::debug!("IncidentData body: {:?}", incident);

    let response = req_client
        .post(incidents_url)
        .headers(headers.clone())
        .json(incident)
        .send()
        .await
        .context("Failed to send incident report request")?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        anyhow::bail!("Error reporting incident: [{}] {}", status, error_text);
    }

    Ok(())
}

/// Processes a single health metric for a component.
async fn process_health_metric(
    ctx: &ReporterContext<'_>,
    env_name: &str,
    service_name: &str,
    component: &Component,
    component_id_cache: &mut ComponentIdCache,
) -> Result<()> {
    let mut data = query_health_api(
        ctx.req_client,
        ctx.config.server.port,
        env_name,
        service_name,
        &ctx.sdb_config.query_from,
        &ctx.sdb_config.query_to,
    )
    .await?;

    let Some(last) = data.metrics.pop() else {
        return Ok(()); // No metrics available
    };

    if last.value == 0 {
        return Ok(()); // Status OK, nothing to report
    }

    let shifted_date = Utc
        .timestamp_opt(last.ts as i64, 0)
        .single()
        .map(|ts| ts - chrono::Duration::seconds(1))
        .unwrap_or_else(|| Utc::now() - chrono::Duration::seconds(1));

    // Get metric names for logging
    let metric_names = ctx
        .config
        .health_metrics
        .get(service_name)
        .map(|h| h.metrics.clone())
        .unwrap_or_default();

    // Combined JSON log
    let log_obj = json!({
        "timestamp": shifted_date.to_rfc3339(),
        "status": last.value,
        "service": service_name,
        "environment": env_name,
        "configured_metrics": metric_names,
        "triggered_metrics": last.triggered,
        "metric_value": last.metric_value,
        "component": {
            "name": component.name,
            "attributes": component.attributes,
        }
    });
    tracing::info!("{}", log_obj.to_string());

    // Search for component ID in the cache using name and attributes
    let mut sorted_attrs = component.attributes.clone();
    sorted_attrs.sort();

    // First attempt to find Component
    let mut component_id = find_component_id(component_id_cache, &component.name, &sorted_attrs);

    // If component not found, refresh cache and try again
    if component_id.is_none() {
        tracing::info!(
            "Component '{}' with attributes {:?} not found in cache. Attempting to refresh.",
            component.name,
            component.attributes
        );

        match update_component_cache(ctx.req_client, ctx.components_url, 0, 0).await {
            Ok(new_cache) => {
                *component_id_cache = new_cache;
                component_id =
                    find_component_id(component_id_cache, &component.name, &sorted_attrs);
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to refresh component cache, using old one. Error: {}",
                    e
                );
            }
        }
    }

    let Some(id) = component_id else {
        tracing::error!(
            "Component with name '{}' and attributes {:?} still not found in status-dashboard cache after refresh.",
            component.name,
            component.attributes
        );
        return Ok(());
    };

    tracing::info!("Found component ID {} in cache.", id);

    // Build IncidentData body for API v2
    let incident = IncidentData {
        title: ctx.sdb_config.incident_title.clone(),
        description: ctx.sdb_config.incident_description.clone(),
        impact: last.value,
        components: vec![id],
        start_date: shifted_date,
        system: true,
        incident_type: "incident".to_string(),
    };

    match report_incident(ctx.req_client, ctx.incidents_url, ctx.headers, &incident).await {
        Ok(()) => {
            tracing::info!(
                "Successfully reported incident for component '{}' with attributes {:?}.",
                component.name,
                component.attributes
            );
        }
        Err(e) => {
            tracing::error!("Error reporting incident: {}", e);
        }
    }

    Ok(())
}

async fn metric_watcher(config: &Config) -> Result<()> {
    tracing::info!("Starting metric reporter thread");

    let sdb_config = config
        .status_dashboard
        .as_ref()
        .context("Status dashboard section is missing")?;

    // Init reqwest client with configurable timeout
    let req_client = ClientBuilder::new()
        .timeout(Duration::from_secs(sdb_config.timeout))
        .build()
        .context("Failed to build HTTP client")?;

    // Build component lookup table from config
    let components_from_config = build_components_from_config(config);

    // Fetch components from Status Dashboard and build a cache to resolve component name to ID
    let components_url = format!("{}/v2/components", sdb_config.url);
    let mut component_id_cache = update_component_cache(
        &req_client,
        &components_url,
        sdb_config.retry_count,
        sdb_config.retry_delay,
    )
    .await
    .context("Initial component cache load failed. Reporter cannot proceed.")?;

    // Prepare for incident reporting
    let incidents_url = format!("{}/v2/incidents", sdb_config.url);
    let headers = create_auth_headers(sdb_config.secret.as_ref())?;

    // Create context for passing to process_health_metric
    let ctx = ReporterContext {
        req_client: &req_client,
        config,
        sdb_config,
        components_url: &components_url,
        incidents_url: &incidents_url,
        headers: &headers,
    };

    loop {
        // For every env from config
        for env in config.environments.iter() {
            tracing::trace!("env {:?}", env);

            let Some(env_components) = components_from_config.get(&env.name) else {
                continue;
            };

            // For every component (health_metric service)
            for (service_name, _component_def) in config.health_metrics.iter() {
                tracing::trace!("Component {:?}", service_name);

                let Some(component) = env_components.get(service_name) else {
                    continue;
                };

                if let Err(e) = process_health_metric(
                    &ctx,
                    &env.name,
                    service_name,
                    component,
                    &mut component_id_cache,
                )
                .await
                {
                    tracing::error!(
                        "Error processing health metric for {}/{}: {}",
                        env.name,
                        service_name,
                        e
                    );
                }
            }
        }

        // Sleep for configurable interval
        sleep(Duration::from_secs(sdb_config.poll_interval)).await;
    }
}
