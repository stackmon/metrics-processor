use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::net::SocketAddr;
use tokio::signal;

#[derive(Deserialize, Debug)]
struct HealthQuery {
    environment: String,
    service: String,
    #[serde(default)]
    _from: String,
    #[serde(default)]
    _to: String,
}

/// Structure of the response which waits reporter.rs
#[derive(Serialize, Debug)]
struct ServiceHealthPoint {
    ts: u32,
    value: u8,
    #[serde(default)]
    triggered: Vec<String>,
    #[serde(default)]
    metric_value: Option<f64>,
}

#[derive(Serialize, Debug)]
struct ServiceHealthResponse {
    name: String,
    service_category: String,
    environment: String,
    metrics: Vec<ServiceHealthPoint>,
}

/// Simulated metric generator that autonomously produces metric data
#[derive(Clone)]
struct MetricGenerator {}

impl MetricGenerator {
    fn new() -> Self {
        MetricGenerator {}
    }

    /// Generate metrics based on time to simulate autonomous failures.
    fn generate_metrics(&self, environment: &str, service: &str) -> (u8, Vec<String>, Option<f64>) {
        match (environment, service) {
            ("production_eu-de", "as") => {
                // AS: api_down (weight 2)
                // Always failing with 100% failure rate
                (2, vec!["as.api_down".to_string()], Some(100.0))
            }
            ("production_eu-de", "deh") => {
                // DEH: api_slow (weight 1)
                // Always failing with response time above threshold
                (1, vec!["deh.api_slow".to_string()], Some(1500.0))
            }
            ("production_eu-de", "css") => {
                // CSS: api_down (weight 2)
                // Always failing with 100% failure rate
                (2, vec!["css.api_down".to_string()], Some(100.0))
            }
            _ => {
                // Unknown service - return OK
                (0, vec![], Some(0.0))
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let health_config = load_health_metrics("conf.d/health_metrics.yaml")
        .expect("Failed to load health_metrics.yaml");

    let metric_generator = MetricGenerator::new();

    let app = Router::new()
        .route("/api/v1/health", get(health_handler))
        .with_state((metric_generator, health_config));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3005));
    println!("Mock convertor listening on {}", addr);
    println!("Autonomously simulating component failures...");

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn health_handler(
    State((metric_generator, health_config)): State<(MetricGenerator, Value)>,
    Query(params): Query<HealthQuery>,
) -> (StatusCode, Json<ServiceHealthResponse>) {
    println!(
        "Request: environment={}, service={}",
        params.environment, params.service
    );

    // Get service configuration from health_metrics
    let service_config = health_config
        .get("health_metrics")
        .and_then(|hm| hm.get(&params.service));

    // Generate autonomous metric data based on time
    let (status_weight, triggered_metrics, raw_metric_value) =
        metric_generator.generate_metrics(&params.environment, &params.service);

    let service_category = if let Some(config) = service_config {
        config
            .get("category")
            .and_then(|c| c.as_str())
            .unwrap_or("unknown")
            .to_string()
    } else {
        "unknown".to_string()
    };

    let metric_time = Utc::now().timestamp() as u32;

    let response = ServiceHealthResponse {
        name: params.service.clone(),
        service_category,
        environment: params.environment.clone(),
        metrics: vec![ServiceHealthPoint {
            ts: metric_time,
            value: status_weight,
            triggered: triggered_metrics.clone(),
            metric_value: raw_metric_value,
        }],
    };

    println!(
        "Response: status={}, triggered={:?}, metric_value={:?}",
        status_weight, triggered_metrics, raw_metric_value
    );
    (StatusCode::OK, Json(response))
}

fn load_health_metrics(path: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let config: Value = serde_yaml::from_str(&content)?;
    Ok(config)
}

async fn shutdown_signal() {
    signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C handler");
    println!("Signal received, shutting down mock server.");
}
