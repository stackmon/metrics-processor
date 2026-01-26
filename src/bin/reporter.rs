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

use serde::{Deserialize, Serialize};

use std::collections::{BTreeMap, HashMap};

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use hmac::{Hmac, Mac};
use jwt::SignWithKey;
use sha2::Sha256;

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
type ComponentCache = HashMap<String, HashMap<String, u32>>;

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
        .timeout(Duration::from_secs(5 as u64))
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
    let status_report_url = format!("{}/v1/component_status", sdb_config.url.clone(),);
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
                                            let body = ComponentStatus {
                                                name: component.name.clone(),
                                                impact: last.1,
                                                attributes: component.attributes.clone(),
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
