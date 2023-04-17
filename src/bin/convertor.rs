//! cloudmon-metrics is an application that produces CloudMon metrics based on the configuration
//!
use reqwest::Error;
use tower_http::request_id::{MakeRequestId, RequestId};

use axum::{
    //body::Bytes,
    extract::MatchedPath,
    http::{Request, StatusCode, Uri},
    // response::Response,
    Router,
};
use tokio::signal;
use tower::ServiceBuilder;
use tower_http::ServiceBuilderExt;
use tower_http::{
    trace::{DefaultOnRequest, DefaultOnResponse, TraceLayer},
    LatencyUnit,
};
use tracing::{info_span, Level};

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

// Use Jemalloc only for musl-64 bits platforms
#[cfg(all(target_env = "musl", target_pointer_width = "64"))]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

// A `MakeRequestId` that increments an atomic counter
#[derive(Clone, Default)]
struct MyMakeRequestId {}

impl MakeRequestId for MyMakeRequestId {
    fn make_request_id<B>(&mut self, _request: &http::Request<B>) -> Option<RequestId> {
        let req_id = Uuid::new_v4().simple().to_string();

        Some(RequestId::new(
            http::HeaderValue::from_str(req_id.as_str()).unwrap(),
        ))
    }
}

use cloudmon_metrics::api::v1;
use cloudmon_metrics::config::Config;
use cloudmon_metrics::graphite;
use cloudmon_metrics::types::AppState;

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting cloudmon-metrics-convertor");

    let config = Config::new("config.yaml").unwrap();
    let mut state = AppState::new(config);
    state.process_config();
    let server_addr = state.config.get_socket_addr().clone();

    // build our application with a single route
    let app = Router::new()
        // .route("/", get(|| async { "" }))
        .merge(graphite::get_graphite_routes())
        .nest("/api/v1", v1::get_v1_routes())
        .layer(
            ServiceBuilder::new()
                // Inject x-request-id header into processing
                .set_x_request_id(MyMakeRequestId::default())
                .propagate_x_request_id()
                // `TraceLayer` is provided by tower-http so you have to add that as a dependency.
                // It provides good defaults but is also very customizable.
                //
                // See https://docs.rs/tower-http/0.1.1/tower_http/trace/index.html for more details.
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(|request: &Request<_>| {
                            // Use request.uri() or OriginalUri if you want the real path.
                            let matched_path = request
                                .extensions()
                                .get::<MatchedPath>()
                                .map(MatchedPath::as_str);
                            info_span!(
                                "http_request",
                                method = ?request.method(),
                                matched_path,
                                uri = ?request.uri().path()
                            )
                        })
                        .on_request(DefaultOnRequest::new().level(Level::INFO))
                        .on_response(
                            DefaultOnResponse::new()
                                .level(Level::INFO)
                                .latency_unit(LatencyUnit::Micros),
                        ),
                ),
        )
        .with_state(state);

    // add a fallback service for handling routes to unknown paths
    let app = app.fallback(handler_404);

    tracing::debug!("listening on {}", server_addr);
    axum::Server::bind(&server_addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();

    tracing::info!("Stopped cloudmon-metrics-convertor");
    Ok(())
}

/// Return 404 error
async fn handler_404(uri: Uri) -> (StatusCode, String) {
    tracing::info!("URL not found");
    (StatusCode::NOT_FOUND, format!("No route for {}", uri))
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

#[cfg(test)]
mod test {
    // use super::*;
    // use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
}
