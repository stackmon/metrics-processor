//! Graphite communication module
//!
//! Module for communication with Graphite TSDB
//!
use axum::{
    async_trait,
    extract::{FromRequest, Query, State},
    http::{header::CONTENT_TYPE, Request, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Form, Json, RequestExt, Router,
};
use axum_macros::debug_handler;
use chrono::{DateTime, FixedOffset};
use itertools::Itertools;
//use regex::Regex;
//use reqwest::Error;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
// use std::sync::Arc;

use crate::common::{get_metric_flag_state, get_service_health};
use crate::types::{AppState, CloudMonError};

#[derive(Deserialize, Serialize, Debug)]
pub struct GraphiteData {
    /// Target name
    pub target: String,
    /// Array of (value, timestamp) tuples
    pub datapoints: Vec<(Option<f32>, u32)>,
}

#[derive(Debug, Deserialize)]
pub struct MetricsQuery {
    pub query: String,
    pub from: Option<String>,
    pub until: Option<String>,
}

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Metric {
    #[serde(rename(serialize = "allowChildren"))]
    pub allow_children: u8,
    pub expandable: u8,
    pub leaf: u8,
    pub id: String,
    pub text: String,
}

#[derive(Default, Debug, Deserialize)]
pub struct RenderRequest {
    pub target: Option<String>,
    pub from: Option<String>,
    pub until: Option<String>,
    #[serde(rename(deserialize = "maxDataPoints"))]
    pub max_data_points: Option<u16>,
}

#[derive(Default, Debug)]
pub struct JsonOrForm<T>(T);

#[async_trait]
impl<S, B, T> FromRequest<S, B> for JsonOrForm<T>
where
    B: Send + 'static,
    S: Send + Sync,
    Json<T>: FromRequest<(), B>,
    Form<T>: FromRequest<(), B>,
    T: 'static,
{
    type Rejection = Response;

    async fn from_request(req: Request<B>, _state: &S) -> Result<Self, Self::Rejection> {
        let content_type_header = req.headers().get(CONTENT_TYPE);
        let content_type = content_type_header.and_then(|value| value.to_str().ok());

        if let Some(content_type) = content_type {
            if content_type.starts_with("application/json") {
                let Json(payload) = req.extract().await.map_err(IntoResponse::into_response)?;
                return Ok(Self(payload));
            }

            if content_type.starts_with("application/x-www-form-urlencoded") {
                let Form(payload) = req.extract().await.map_err(IntoResponse::into_response)?;
                return Ok(Self(payload));
            }
        }

        Err(StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response())
    }
}

pub fn get_graphite_routes() -> Router<AppState> {
    return Router::new()
        .route("/functions", get(handler_functions))
        .route(
            "/metrics/find",
            get(handler_metrics_find), /*.post(handler_metrics_find)*/
        )
        .route("/render", get(handler_render).post(handler_render))
        .route("/tags/autoComplete/tags", get(handler_tags));
}

/// Handler for graphite list supported functions API
pub async fn handler_functions() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({})))
}

pub fn find_metrics(find_request: MetricsQuery, state: AppState) -> Vec<Metric> {
    tracing::debug!("Processing find query={:?}", find_request);
    tracing::debug!("target={:?}", find_request.query);
    let mut metrics: Vec<Metric> = Vec::new();
    let target_parts: Vec<&str> = find_request.query.split(".").collect();
    if target_parts.len() == 1 && "*".eq(target_parts[0]) {
        // Returning 1st level
        metrics.push(Metric {
            allow_children: 1,
            expandable: 1,
            leaf: 0,
            id: "flag".to_string(),
            text: "flag".to_string(),
        });
        metrics.push(Metric {
            allow_children: 1,
            expandable: 1,
            leaf: 0,
            id: "health".to_string(),
            text: "health".to_string(),
        });
    } else if target_parts.len() == 2 && "*".eq(target_parts[1]) {
        for env in state.environments.iter() {
            metrics.push(Metric {
                allow_children: 1,
                expandable: 1,
                leaf: 0,
                id: env.name.clone(),
                text: env.name.clone(),
            });
        }
    } else {
        // we do not support metrics without clear environment
        if "flag".eq(target_parts[0]) {
            // Returning known flag metrics
            if target_parts.len() == 3 {
                // Return 3rd level - services
                metrics.extend(state.services.iter().map(|x| Metric {
                    allow_children: 1,
                    expandable: 1,
                    leaf: 0,
                    id: x.clone(),
                    text: x.clone(),
                }));
            } else if target_parts.len() == 4 {
                // return service metrics
                if "*".eq(target_parts[3]) {
                    // Return all metrics for service
                    metrics.extend(
                        state
                            .flag_metrics
                            .keys()
                            .filter(|x| x.starts_with(target_parts[2]))
                            .map(|x| Metric {
                                allow_children: 0,
                                expandable: 0,
                                leaf: 1,
                                id: x.clone(),
                                text: x.clone(),
                            }),
                    );
                } else {
                    // exact metric
                    let search = format!("{}.{}", target_parts[2], target_parts[3]);
                    metrics.extend(
                        state
                            .flag_metrics
                            .keys()
                            .filter(|x| *x == &search)
                            .map(|x| Metric {
                                allow_children: 0,
                                expandable: 0,
                                leaf: 1,
                                id: x.clone(),
                                text: x.clone(),
                            }),
                    );
                }
            }
        } else if target_parts.len() == 3 && "health".eq(target_parts[0]) {
            // Returning known health metrics
            metrics.extend(state.health_metrics.keys().map(|x| Metric {
                allow_children: 0,
                expandable: 0,
                leaf: 1,
                id: x.clone(),
                text: x.clone(),
            }));
        }
        tracing::debug!("Elements {:?}", target_parts);
    }
    return metrics;
}

/// Handler for graphite find metrics API
#[debug_handler]
pub async fn handler_metrics_find(
    State(state): State<AppState>,
    Query(query): Query<MetricsQuery>,
) -> impl IntoResponse {
    let metrics: Vec<Metric> = find_metrics(query, state);
    return (
        StatusCode::OK,
        Json(json!(metrics
            .into_iter()
            .sorted_by(|a, b| Ord::cmp(&a.text, &b.text))
            .collect::<Vec<Metric>>())),
    );
}

/// Handler for graphite render API
#[debug_handler]
pub async fn handler_render(
    query: Option<Query<RenderRequest>>,
    State(state): State<AppState>,
    payload: Option<JsonOrForm<RenderRequest>>,
) -> impl IntoResponse {
    let Query(query) = query.unwrap_or_default();
    let target = match payload {
        Some(JsonOrForm(ref x)) => x.target.as_ref().expect("Target is required"),
        None => query.target.as_ref().expect("Target is required"),
    };
    let max_data_points = match payload {
        Some(JsonOrForm(ref x)) => x.max_data_points.expect(" is required"),
        None => query.max_data_points.expect("Query is required"),
    };
    let from: Option<String> = match payload {
        Some(JsonOrForm(ref x)) => x.from.clone(),
        None => query.from.clone(),
    };
    let to: Option<String> = match payload {
        Some(JsonOrForm(ref x)) => x.until.clone(),
        None => query.until.clone(),
    };

    let target_parts: Vec<&str> = target.split(".").collect();
    match target_parts[0] {
        "flag" => {
            tracing::debug!("render flags");
            let mut graphite_targets: HashMap<String, String> = HashMap::new();
            if target_parts.len() == 4 {
                let environment = target_parts[1];
                let metric_name = format!("{}.{}", target_parts[2], target_parts[3]);
                if metric_name.ends_with("*") {
                    let target = &metric_name[0..metric_name.len() - 1];
                    for (metric, metric_map) in state.flag_metrics.iter() {
                        if metric.starts_with(target) {
                            if let Some(m) = metric_map.get(environment) {
                                graphite_targets.insert(metric.clone(), m.query.clone());
                            }
                        }
                    }
                } else if let Some(metric) = state.flag_metrics.get(&metric_name) {
                    match metric.get(environment) {
                        Some(m) => {
                            graphite_targets.insert(metric_name.clone(), m.query.clone());
                        }
                        _ => {}
                    };
                }
                tracing::debug!("Requesting Graphite {:?}", graphite_targets);

                match get_graphite_data(
                    &state.req_client,
                    &state.config.datasource.url.as_str(),
                    &graphite_targets,
                    None,
                    from,
                    None,
                    to,
                    max_data_points,
                )
                .await
                {
                    Ok(mut raw_data) => {
                        for data_element in raw_data.iter_mut() {
                            // target + datapoints
                            tracing::trace!("Processing dataframe {:?}", data_element);
                            match state.flag_metrics.get(&data_element.target) {
                                Some(metric_cfg) => {
                                    // if metric is known to us
                                    tracing::trace!(
                                        "Processing datapoints for metric {:?}",
                                        metric_cfg
                                    );
                                    let metric = metric_cfg.get(environment).unwrap();
                                    // Iterate over all fetched series
                                    for (val, _) in data_element.datapoints.iter_mut() {
                                        *val = if get_metric_flag_state(val, metric) {
                                            Some(1.0)
                                        } else {
                                            Some(0.0)
                                        };
                                    }
                                }
                                None => {
                                    tracing::warn!(
                                        "DB Response contains unknown target: {}",
                                        data_element.target
                                    );
                                }
                            }
                        }

                        return (StatusCode::OK, Json(json!(raw_data)));
                    }
                    Err(_) => {
                        return (
                            StatusCode::OK,
                            Json(json!({"message": "Error reading data from TSDB"})),
                        )
                    }
                };
            }
        }
        "health" => {
            tracing::debug!("render health");
            if target_parts.len() == 3 {
                let from = from.unwrap();
                let to = to.unwrap();
                if let Ok(service_health_data) = get_service_health(
                    &state,
                    target_parts[2],
                    target_parts[1],
                    from.as_str(),
                    to.as_str(),
                    max_data_points,
                )
                .await
                {
                    return (
                        StatusCode::OK,
                        Json(
                            json!([{"target": target_parts[2], "datapoints": service_health_data.iter().map(|x| (Some(x.1 as f32), x.0)).collect::<Vec<(Option<f32>, u32)>>()}]),
                        ),
                    );
                }
            }
        }
        _ => {}
    }
    (
        StatusCode::OK,
        //Json(json!([{"target": "", "datapoints": []}])),
        Json(json!([])),
    )
}

fn alias_graphite_query(query: &str, alias: &str) -> String {
    format!("alias({},'{}')", query, alias)
}

/// Fetch required data from Graphite
pub async fn get_graphite_data(
    client: &reqwest::Client,
    url: &str,
    targets: &HashMap<String, String>,
    from: Option<DateTime<FixedOffset>>,
    from_raw: Option<String>,
    to: Option<DateTime<FixedOffset>>,
    to_raw: Option<String>,
    max_data_points: u16,
) -> Result<Vec<GraphiteData>, CloudMonError> {
    // Prepare vector of query parameters
    let mut query_params: Vec<(_, String)> = [
        ("format", "json".to_string()),
        // ("noNullPoints", "true".to_string()),
        ("maxDataPoints", max_data_points.to_string()),
    ]
    .into();
    if let Some(xfrom) = from {
        query_params.push(("from", xfrom.format("%H:%M_%Y%m%d").to_string()));
    } else if let Some(xfrom) = from_raw {
        query_params.push(("from", xfrom.clone()));
    }
    if let Some(xto) = to {
        query_params.push(("until", xto.format("%H:%M_%Y%m%d").to_string()));
    } else if let Some(xto) = to_raw {
        query_params.push(("until", xto.clone()));
    }
    query_params.extend(
        targets
            .iter()
            .map(|x| ("target", alias_graphite_query(x.1, x.0))),
    );
    tracing::trace!("Query: {:?}", &query_params);
    let res = client
        .get(format!("{}/render", url))
        .query(&query_params)
        .send()
        .await;
    match res {
        Ok(rsp) => {
            if rsp.status().is_client_error() {
                tracing::error!("Error: {:?}", rsp.text().await);
                return Err(CloudMonError::GraphiteError);
            } else {
                tracing::trace!("Status: {}", rsp.status());
                tracing::trace!("Headers:\n{:#?}", rsp.headers());
                match rsp.json().await {
                    Ok(dt) => return Ok(dt),
                    Err(_) => return Err(CloudMonError::GraphiteError),
                }
            }
        }
        Err(_) => return Err(CloudMonError::GraphiteError),
    };
}
///
/// Handler for graphite tags API
#[debug_handler]
pub async fn handler_tags() -> impl IntoResponse {
    (StatusCode::OK, Json(json!([])))
}

#[cfg(test)]
mod test {
    use crate::*;
    use mockito::Matcher;
    use reqwest::ClientBuilder;
    // use std::sync::Arc;
    use chrono::{DateTime, FixedOffset};
    use std::time::Duration;

    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use serde_json::{json, Value};
    use tower::Service; // for `call`
    use tower::ServiceExt; // for `oneshot` and `ready`

    use std::collections::HashMap;

    #[test]
    fn test_alias_graphite_query() {
        assert_eq!(graphite::alias_graphite_query("q", "n"), "alias(q,'n')");
    }

    macro_rules! aw {
        ($e:expr) => {
            tokio_test::block_on($e)
        };
    }

    #[test]
    fn test_get_graphite_data() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock("GET", "/render")
            .expect(1)
            .match_query(Matcher::AllOf(vec![
                Matcher::UrlEncoded("target".into(), "alias(query,'alias')".into()),
                Matcher::UrlEncoded("from".into(), "00:00_20220101".into()),
                Matcher::UrlEncoded("until".into(), "00:00_20220201".into()),
                Matcher::UrlEncoded("maxDataPoints".into(), "15".into()),
            ]))
            .create();
        let timeout = Duration::from_secs(1 as u64);
        let _req_client: reqwest::Client = ClientBuilder::new().timeout(timeout).build().unwrap();

        let mut targets: HashMap<String, String> = HashMap::new();
        targets.insert("alias".to_string(), "query".to_string());
        let from: Option<DateTime<FixedOffset>> =
            DateTime::parse_from_rfc3339("2022-01-01T00:00:00+00:00").ok();
        let to: Option<DateTime<FixedOffset>> =
            DateTime::parse_from_rfc3339("2022-02-01T00:00:00+00:00").ok();
        let max_data_points: u16 = 15;
        let _res = aw!(graphite::get_graphite_data(
            &_req_client,
            format!("{}", server.url()).as_str(),
            &targets,
            from,
            None,
            to,
            None,
            max_data_points,
        ));
        mock.assert();
    }

    #[tokio::test]
    async fn test_get_grafana_find() {
        let f = "
        datasource:
          url: 'https:/a.b'
        server:
          port: 3005
        metric_templates:
          tmpl1:
            query: dummy1($environment.$service.count)
            op: lt
            threshold: 90
          tmpl2:
            query: dummy2($environment.$service.count)
            op: gt
            threshold: 80
        environments:
          - name: env1
        flag_metrics:
          - name: metric-1
            service: srvA
            template:
              name: tmpl1
            environments:
              - name: env1
              - name: env2
                threshold: 1
          - name: metric-2
            service: srvA
            template:
              name: tmpl2
            environments:
              - name: env1
              - name: env2
        health_metrics:
          srvA:
            service: srvA
            category: compute
            metrics:
              - srvA.metric-1
              - srvA.metric-2
            expressions:
              - expression: 'srvA.metric-1 || srvA.metric-2'
                weight: 1
";
        let config = config::Config::from_config_str(f);
        let mut state = types::AppState::new(config);
        state.process_config();

        // let app_state = Arc::new(state);
        let mut app = graphite::get_graphite_routes().with_state(state);

        let request_all = Request::builder()
            .uri("/metrics/find?query=*")
            .body(Body::empty())
            .unwrap();
        let request_l1 = Request::builder()
            .uri("/metrics/find?query=health")
            .body(Body::empty())
            .unwrap();
        let request_l2 = Request::builder()
            .uri("/metrics/find?query=flag.*")
            .body(Body::empty())
            .unwrap();
        let request_flag_l3 = Request::builder()
            .uri("/metrics/find?query=flag.env1.*")
            .body(Body::empty())
            .unwrap();
        let request_flag_l4 = Request::builder()
            .uri("/metrics/find?query=flag.env1.srvA.*")
            .body(Body::empty())
            .unwrap();
        let request_health_l3 = Request::builder()
            .uri("/metrics/find?query=health.env1.*")
            .body(Body::empty())
            .unwrap();

        let response = app.ready().await.unwrap().call(request_all).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            body,
            json!([{"allowChildren": 1, "expandable": 1, "id": "flag", "leaf": 0, "text": "flag"},{"allowChildren": 1, "expandable": 1, "id": "health", "leaf": 0, "text": "health"}])
        );

        let response = app.ready().await.unwrap().call(request_l1).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body, json!([]));

        let response = app.ready().await.unwrap().call(request_l2).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            body,
            json!([{"allowChildren": 1, "expandable": 1, "id": "env1", "leaf": 0, "text": "env1"}])
        );

        let response = app
            .ready()
            .await
            .unwrap()
            .call(request_flag_l3)
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            body,
            json!([{"allowChildren": 1, "expandable": 1, "id": "srvA", "leaf": 0, "text": "srvA"}])
        );

        let response = app
            .ready()
            .await
            .unwrap()
            .call(request_flag_l4)
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            body,
            json!([{"allowChildren": 0, "expandable": 0, "id": "srvA.metric-1", "leaf": 1, "text": "srvA.metric-1"},{"allowChildren": 0, "expandable": 0, "id": "srvA.metric-2", "leaf": 1, "text": "srvA.metric-2"}])
        );

        let response = app
            .ready()
            .await
            .unwrap()
            .call(request_health_l3)
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            body,
            json!([{"allowChildren": 0, "expandable": 0, "id": "srvA", "leaf": 1, "text": "srvA"}])
        );
    }
}
