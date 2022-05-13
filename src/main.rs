//! cloudmon-metrics is an application that produces CloudMon metrics based on the configuration
//! for Grafana Json Datasource plugin
//!
use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

use axum::{
    extract::Extension, handler::Handler, http::StatusCode, response::IntoResponse, routing::get,
    Json, Router,
};
use reqwest::ClientBuilder;
use reqwest::Error;
use std::time::Duration;
use tokio::signal;
// use tracing::Span;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// Use Jemalloc only for musl-64 bits platforms
#[cfg(all(target_env = "musl", target_pointer_width = "64"))]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[derive(Debug, Deserialize)]
struct Config {
    datasource: Datasource,
    server: ConfigServer,
    metrics: HashMap<String, MetricDef>,
}

#[derive(Debug, Deserialize)]
struct ConfigServer {
    #[serde(default = "default_address")]
    address: String,
    #[serde(default = "default_port")]
    port: u16,
}

fn default_address() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    3000
}

fn default_timeout() -> u16 {
    5
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum DatasourceType {
    Graphite,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum CmpType {
    Lt,
    Gt,
    Eq,
}

#[derive(Debug, Deserialize)]
struct Datasource {
    url: String,
    // #[serde(rename(deserialize = "type"))]
    // ds_type: DatasourceType,
    #[serde(default = "default_timeout")]
    timeout: u16,
}

#[derive(Debug, Deserialize)]
struct MetricDef {
    query: String,
    op: CmpType,
    ref_value: f32,
}

type MetricPoints = BTreeMap<u32, bool>;
#[derive(Debug, Deserialize, Serialize)]
struct MetricData {
    target: String,
    #[serde(rename(serialize = "datapoints"))]
    points: MetricPoints,
}

#[derive(Deserialize, Debug)]
struct GraphiteData {
    target: String,
    datapoints: Vec<(Option<f32>, u32)>,
}

struct AppState {
    config: Config,
    req_client: reqwest::Client,
}

#[derive(Deserialize, Debug)]
struct GrafanaJsonSearchRequest {
    target: String,
}

#[derive(Deserialize, Debug)]
struct GrafanaJsonQueryRequest {
    // #[serde(rename(deserialize = "startTime"))]
    // start_time: u64,
    // interval: String,
    // #[serde(rename(deserialize = "intervalMs"))]
    // interval_ms: u32,
    range: GrafanaJsonQueryRequestRange,
    // #[serde(rename(deserialize = "rangeRaw"))]
    // range_raw: GrafanaJsonQueryRequestRangeRaw,
    targets: Vec<GrafanaTarget>,
    // #[serde(rename(deserialize = "maxDataPoints"))]
    // max_data_points: u16,
}

#[derive(Debug, Deserialize)]
struct GrafanaJsonQueryRequestRange {
    from: String,
    to: String,
    // raw: GrafanaJsonQueryRequestRangeRaw,
}

// #[derive(Debug, Deserialize)]
// struct GrafanaJsonQueryRequestRangeRaw {
//     from: String,
//     to: String,
// }

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum GrafanaJsonTargetType {
    Timeserie,
    Table,
}

#[derive(Deserialize, Debug)]
struct GrafanaTarget {
    target: String,
    // #[serde(rename(deserialize = "type"))]
    // target_type: GrafanaJsonTargetType,
    // #[serde(rename(deserialize = "refId"))]
    // ref_id: String,
}

#[derive(Serialize, Debug)]
struct GrafanaDataTargetResponse {
    target: String,
    datapoints: Vec<(f32, u64)>,
}

fn alias_graphite_query(query: &str, alias: &str) -> String {
    format!("alias({},'{}')", query, alias)
}

/// Fetch required data from Graphite
async fn get_graphite_data(
    client: &reqwest::Client,
    url: &str,
    targets: HashMap<&str, &str>,
    from: Option<DateTime<FixedOffset>>,
    to: Option<DateTime<FixedOffset>>,
) -> Result<Vec<GraphiteData>, Error> {
    // Prepare vector of query parameters
    let mut query_params: Vec<(_, String)> = [
        ("format", "json".to_string()),
        ("noNullPoints", "true".to_string()),
    ]
    .into();
    if let Some(xfrom) = from {
        query_params.push(("from", xfrom.format("%H:%M_%Y%m%d").to_string()));
    }
    if let Some(xto) = to {
        query_params.push(("until", xto.format("%H:%M_%Y%m%d").to_string()));
    }
    query_params.extend(
        targets
            .iter()
            .map(|x| ("target", alias_graphite_query(x.1, x.0))),
    );
    let res = client
        .get(format!("{}/render", url))
        .query(&query_params)
        .send()
        .await?;
    // log::debug!("Status: {}", res.status());
    // log::debug!("Headers:\n{:#?}", res.headers());

    let data: Vec<GraphiteData> = res.json().await?;
    Ok(data)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "cloudmon=debug,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting cloudmon-metrics");

    let f = std::fs::File::open("config.yaml").expect("Could not open file.");
    let config: Config = serde_yaml::from_reader(f).expect("Could not read values.");

    let timeout = Duration::from_secs(config.datasource.timeout as u64);
    let req_client: reqwest::Client = ClientBuilder::new().timeout(timeout).build()?;

    let addr = SocketAddr::from((
        config.server.address.as_str().parse::<IpAddr>().unwrap(),
        config.server.port,
    ));
    let app_state = Arc::new(AppState { config, req_client });

    // build our application with a single route
    let app = Router::new()
        .route("/", get(|| async { "" }))
        .route("/query", get(handler_query).post(handler_query))
        .route("/search", get(handler_search).post(handler_search))
        .route("/annotations", get(|| async { "" }))
        .layer(
            ServiceBuilder::new()
                .layer(Extension(app_state))
                // `TraceLayer` is provided by tower-http so you have to add that as a dependency.
                // It provides good defaults but is also very customizable.
                //
                // See https://docs.rs/tower-http/0.1.1/tower_http/trace/index.html for more details.
                //        .layer(TraceLayer::new_for_http().on_request(
                //            |request: &axum::http::Request<_>, _span: &Span| {
                //                tracing::debug!(
                //                    "started {} {} {:?}",
                //                    request.method(),
                //                    request.uri().path(),
                //                    request
                //                )
                //            },
                //        ));
                .layer(TraceLayer::new_for_http()),
        );

    // add a fallback service for handling routes to unknown paths
    let app = app.fallback(handler_404.into_service());

    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();

    tracing::info!("Stopped cloudmon-metrics");
    Ok(())
}

async fn get_metrics(
    state: &AppState,
    metric_names: Vec<String>,
    from: &str,
    to: &str,
) -> Vec<MetricData> {
    let mut result: Vec<MetricData> = Vec::new();
    let mut graphite_targets: HashMap<&str, &str> = HashMap::new();
    // Construct target=>query map
    for metric in metric_names.iter() {
        match state.config.metrics.get(metric) {
            Some(m) => {
                graphite_targets.insert(metric.as_str(), m.query.as_str());
            }
            _ => {}
        };
    }
    tracing::debug!("Requesting {:?}", graphite_targets);
    let raw_data: Vec<GraphiteData> = get_graphite_data(
        &state.req_client,
        &state.config.datasource.url.as_str(),
        graphite_targets,
        DateTime::parse_from_rfc3339(from).ok(),
        DateTime::parse_from_rfc3339(to).ok(),
    )
    .await
    .unwrap();
    // tracing::debug!("Received following data: {:?}", raw_data);
    for data_element in raw_data.iter() {
        match state.config.metrics.get(&data_element.target) {
            Some(metric) => {
                // log::debug!("Data element {:?}", data_element);
                let points: BTreeMap<u32, bool> = BTreeMap::new();
                let mut md = MetricData {
                    target: data_element.target.clone(),
                    points: points,
                };
                for (val, ts) in data_element.datapoints.iter() {
                    let is_fulfilled = match *val {
                        Some(x) => match metric.op {
                            CmpType::Lt => (x < metric.ref_value),
                            CmpType::Gt => (x > metric.ref_value),
                            CmpType::Eq => (x == metric.ref_value),
                        },
                        None => false,
                    };
                    md.points.insert(*ts, is_fulfilled);
                }
                result.push(md);
            }
            None => {
                tracing::warn!(
                    "DB Response contains unknown target: {}",
                    data_element.target
                );
            }
        }
    }
    // tracing::debug!("Summary data: {:?}", result);

    result
}

/// Handler for the /query endpoint
///
/// It Processes request as described under
/// `<https://grafana.com/grafana/plugins/grafana-simple-json-datasource/>`,
/// queries data from Graphite and returns result.
async fn handler_query(
    Json(payload): Json<GrafanaJsonQueryRequest>,
    Extension(state): Extension<Arc<AppState>>,
) -> impl IntoResponse {
    tracing::debug!("Query with {:?}", payload);
    let mut response: Vec<GrafanaDataTargetResponse> = Vec::new();
    let mut metrics: Vec<String> = Vec::new();
    // Construct list of desired metrics
    for tgt in payload.targets.iter() {
        if "*".eq(&tgt.target) {
            metrics.extend(state.config.metrics.keys().cloned());
        } else {
            metrics.push(tgt.target.clone());
        }
    }
    // Iterate over result and convert it
    for data in get_metrics(
        &state,
        metrics,
        payload.range.from.as_str(),
        payload.range.to.as_str(),
    )
    .await
    .iter()
    {
        let datapoints: Vec<(f32, u64)> = data
            .points
            .iter()
            .map(|x| (if *x.1 { 1.0 } else { 0.0 }, (*x.0) as u64 * 1000))
            .collect();
        let data = GrafanaDataTargetResponse {
            target: data.target.clone(),
            datapoints: datapoints,
        };
        response.push(data);
    }
    // tracing::debug!("Sending {:?} back to requestor", response);
    Json(response)
}

/// Process /search request
async fn handler_search(
    Json(payload): Json<GrafanaJsonSearchRequest>,
    Extension(state): Extension<Arc<AppState>>,
) -> impl IntoResponse {
    tracing::debug!("Searching with {:?}", payload);
    let mut metrics: Vec<String> = vec!["*".to_string()];
    for (k, _) in state.config.metrics.iter() {
        if k.starts_with(payload.target.as_str()) {
            tracing::debug!("Matching {}", k);
            metrics.push(k.clone());
        }
    }
    Json(metrics)
}

/// Return 404 error
async fn handler_404() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "nothing to see here")
}

/// Shutdown handler for the application
async fn shutdown_signal() {
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

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    println!("signal received, starting graceful shutdown");
}
