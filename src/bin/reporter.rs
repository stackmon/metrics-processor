//! cloudmon-metrics-reporter - status-dashboard reporter.
//!
//! Post component status to the CloudMon status-dashboard API.
//!
#![doc(html_no_source)]

extern crate anyhow;

use cloudmon_metrics::sd::{
    build_auth_headers, build_component_id_cache, build_incident_data, create_incident,
    fetch_components, find_component_id, Component, ComponentAttribute,
};
use cloudmon_metrics::{api::v1::ServiceHealthResponse, config::Config};

use reqwest::ClientBuilder;

use tokio::signal;
use tokio::time::{sleep, Duration};

use serde::{Deserialize, Serialize};

use std::collections::HashMap;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const CLIENT_TIMEOUT_SECS: u64 = 2;

/// Component status for V1 API (legacy, will be replaced)
#[derive(Deserialize, Serialize, Debug)]
pub struct ComponentStatus {
    pub name: String,
    pub impact: u8,
    pub attributes: Vec<ComponentAttribute>,
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

    tracing::info!("starting cloudmon-metrics-reporter");

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

    tracing::info!("stopped cloudmon-metrics-reporter");
}

async fn metric_watcher(config: &Config) {
    tracing::info!("starting metric reporter thread");
    // Init reqwest client
    let req_client: reqwest::Client = ClientBuilder::new()
        .timeout(Duration::from_secs(CLIENT_TIMEOUT_SECS))
        .build()
        .unwrap();
    // Endless loop
    let mut components: HashMap<String, HashMap<String, Component>> = HashMap::new();
    for env in config.environments.iter() {
        let comp_env_entry = components.entry(env.name.clone()).or_default();
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

    // Build authorization headers using status_dashboard module (T021, T022, T023 - US3)
    // VERIFIED: Existing HMAC-JWT mechanism works unchanged with V2 endpoints
    let headers = build_auth_headers(
        sdb_config.secret.as_deref(),
        sdb_config.jwt_preferred_username.as_deref(),
        sdb_config.jwt_group.as_deref(),
    );

    // Initialize component ID cache at startup with retry logic (T024, T025, T026, T027)
    // Per FR-006: 3 retry attempts with 60-second delays
    // Per FR-007: Fail to start if all attempts fail
    let mut component_cache = None;
    let max_attempts = 3;

    for attempt in 1..=max_attempts {
        tracing::info!(
            attempt = attempt,
            max_attempts = max_attempts,
            "attempting to fetch components from Status Dashboard"
        );

        match fetch_components(&req_client, &sdb_config.url, &headers).await {
            Ok(components) => {
                tracing::info!(
                    attempt = attempt,
                    component_count = components.len(),
                    "successfully fetched components from Status Dashboard"
                );
                component_cache = Some(build_component_id_cache(components));
                break;
            }
            Err(e) => {
                // T027: Warning logging for each failed attempt with attempt number
                if attempt < max_attempts {
                    tracing::warn!(
                        error = %e,
                        attempt = attempt,
                        max_attempts = max_attempts,
                        retry_delay_seconds = 60,
                        "failed to fetch components, will retry after delay"
                    );
                    // T025: 60-second delay between retry attempts
                    sleep(Duration::from_secs(60)).await;
                } else {
                    // T026: Final failure after all attempts
                    tracing::error!(
                        error = %e,
                        attempt = attempt,
                        max_attempts = max_attempts,
                        "failed to fetch components after all retry attempts, reporter cannot start"
                    );
                }
            }
        }
    }

    // T026: Error return from metric_watcher if cache load fails per FR-007
    let mut component_cache = match component_cache {
        Some(cache) => cache,
        None => {
            tracing::error!("component cache initialization failed, exiting metric_watcher");
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
                // Query metric-convertor for the status (includes metric states and matched expression)
                match req_client
                    .get(format!(
                        "http://localhost:{}/api/v1/health",
                        config.server.port
                    ))
                    // Query env/service for time [query_from...query_to]
                    .query(&[
                        ("environment", env.name.clone()),
                        ("service", component.0.clone()),
                        ("from", config.health_query.query_from.clone()),
                        ("to", config.health_query.query_to.clone()),
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
                                        // Is metric showing issues? (weight > 0 means degraded or outage)
                                        let impact = last.weight;
                                        if impact > 0 {
                                            let comp = components
                                                .get(&env.name)
                                                .unwrap()
                                                .get(component.0)
                                                .unwrap();

                                            // T017: Find component ID in cache (cache miss detection)
                                            let mut component_id =
                                                find_component_id(&component_cache, comp);

                                            // T018: If component not found, refresh cache once per FR-005
                                            if component_id.is_none() {
                                                tracing::info!(
                                                    component_name = comp.name.as_str(),
                                                    service = component.0.as_str(),
                                                    environment = env.name.as_str(),
                                                    "component not found in cache, attempting cache refresh"
                                                );

                                                match fetch_components(
                                                    &req_client,
                                                    &sdb_config.url,
                                                    &headers,
                                                )
                                                .await
                                                {
                                                    Ok(components) => {
                                                        tracing::info!(
                                                            component_count = components.len(),
                                                            "cache refreshed"
                                                        );
                                                        component_cache =
                                                            build_component_id_cache(components);
                                                        // Retry lookup after refresh
                                                        component_id = find_component_id(
                                                            &component_cache,
                                                            comp,
                                                        );
                                                    }
                                                    Err(e) => {
                                                        tracing::warn!(
                                                            error = %e,
                                                            component_name = comp.name.as_str(),
                                                            "failed to refresh component cache"
                                                        );
                                                    }
                                                }
                                            }

                                            // Process component if found
                                            match component_id {
                                                Some(id) => {
                                                    // Build incident data with impact for Status Dashboard
                                                    let incident_data = build_incident_data(
                                                        id,
                                                        impact,
                                                        last.timestamp as i64,
                                                    );

                                                    // Format triggered metric details for logging
                                                    let triggered_metrics: Vec<String> = last
                                                        .triggered_metric_details
                                                        .iter()
                                                        .map(|m| {
                                                            format!(
                                                                "{}(query={}, op={}, threshold={})",
                                                                m.name, m.query, m.op, m.threshold
                                                            )
                                                        })
                                                        .collect();

                                                    // Include full decision context: query parameters, metric details, matched expression
                                                    tracing::info!(
                                                        environment = env.name.as_str(),
                                                        service = component.0.as_str(),
                                                        component_name = comp.name.as_str(),
                                                        component_id = id,
                                                        query_from = config.health_query.query_from.as_str(),
                                                        query_to = config.health_query.query_to.as_str(),
                                                        metric_timestamp = last.timestamp,
                                                        impact = impact,
                                                        triggered_metrics = ?triggered_metrics,
                                                        matched_expression = last.matched_expression.as_deref().unwrap_or("none"),
                                                        "creating incident: health metric indicates service degradation"
                                                    );

                                                    // Create incident via V2 API
                                                    match create_incident(
                                                        &req_client,
                                                        &sdb_config.url,
                                                        &headers,
                                                        &incident_data,
                                                    )
                                                    .await
                                                    {
                                                        Ok(_) => {
                                                            tracing::info!(
                                                                component_id = id,
                                                                impact = impact,
                                                                "incident created successfully"
                                                            );
                                                        }
                                                        Err(e) => {
                                                            // Error logging with details (FR-015)
                                                            tracing::error!(
                                                                error = %e,
                                                                component_id = id,
                                                                service = component.0.as_str(),
                                                                environment = env.name.as_str(),
                                                                "failed to create incident"
                                                            );
                                                        }
                                                    }
                                                }
                                                None => {
                                                    // T019, T020: Warning logging and continue to next service
                                                    tracing::warn!(
                                                        component_name = comp.name.as_str(),
                                                        service = component.0.as_str(),
                                                        environment = env.name.as_str(),
                                                        "component not found in cache even after refresh, skipping incident creation"
                                                    );
                                                    // Continue to next service (no retry on incident creation)
                                                    continue;
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
