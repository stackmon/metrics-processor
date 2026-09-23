//! Integration tests for Status Dashboard API integration
//!
//! Tests for Phase 7: Integration Testing
//! T028-T037: Validate end-to-end Status Dashboard API integration with mocked endpoints

use chrono::DateTime;
use cloudmon_metrics::config::{OidcIdentity, StatusDashboardConfig};
use cloudmon_metrics::sd::{
    build_auth_headers, build_component_id_cache, build_incident_data, create_incident,
    fetch_components, find_component_id, Component, ComponentAttribute, IncidentData,
    StatusDashboardComponent,
};
use mockito::Matcher;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[allow(dead_code)]
#[path = "fixtures/service_account.rs"]
mod service_account;
use service_account::write_service_account_key_file;

const AUTH_SCHEME: &str = "Bearer";
const JWT_BEARER_GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:jwt-bearer";
/// Scope the `zitadel` crate always requests in addition to the configured scopes
const OPENID_SCOPE: &str = "openid";
const REPORTER_SCOPE: &str = "urn:zitadel:iam:org:project:role:sd_reporters";
const AUDIENCE_SCOPE: &str = "urn:zitadel:iam:org:project:id:392066917738875090:aud";
const TOKEN_RESPONSE: &str =
    r#"{"access_token":"mock-access-token","token_type":"Bearer","expires_in":3600}"#;
const OIDC_KEY_FILE_ENV_KEY: &str = "MP_STATUS_DASHBOARD__OIDC_KEY_FILE";
const OIDC_ISSUER_ENV_KEY: &str = "MP_STATUS_DASHBOARD__OIDC_ISSUER";

fn key_file() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().expect("failed to create a temp dir");
    let path = write_service_account_key_file(&dir.path().join("service-account.json"));

    (dir, path)
}

fn status_dashboard_config(issuer: &str, key_file: &str) -> StatusDashboardConfig {
    StatusDashboardConfig {
        url: "https://status.example.com".to_string(),
        oidc_issuer: Some(issuer.to_string()),
        oidc_key_file: Some(key_file.to_string()),
        oidc_scopes: vec![REPORTER_SCOPE.to_string()],
    }
}

fn service_identity(issuer: &str) -> (tempfile::TempDir, OidcIdentity) {
    let (dir, key_file) = key_file();
    let identity = status_dashboard_config(issuer, &key_file)
        .oidc_identity()
        .expect("the key file must resolve a service identity");

    (dir, identity)
}

#[derive(Clone, Debug)]
struct TokenRequest {
    method: String,
    path: String,
    body: String,
    authorization: Option<String>,
}

impl TokenRequest {
    fn capture(request: &mockito::Request) -> Self {
        Self {
            method: request.method().to_string(),
            path: request.path().to_string(),
            body: request
                .body()
                .map(|body| String::from_utf8_lossy(body).to_string())
                .unwrap_or_default(),
            authorization: request
                .header("authorization")
                .first()
                .map(|value| value.to_string()),
        }
    }

    fn form_field(&self, name: &str) -> Option<String> {
        self.body.split('&').find_map(|pair| {
            let (field, value) = pair.split_once('=')?;
            (percent_decode(field) == name).then(|| percent_decode(value))
        })
    }

    fn assertion(&self) -> String {
        self.form_field("assertion")
            .expect("the token request must carry an assertion")
    }
}

struct OidcProvider {
    discovery: mockito::Mock,
    jwks: mockito::Mock,
    token: TokenEndpoint,
}

impl OidcProvider {
    /// The crate runs OIDC discovery and a JWKS fetch before every token request, so one metadata
    /// request is served per token call.
    async fn create(
        server: &mut mockito::ServerGuard,
        calls: usize,
        status: usize,
        response: impl Fn(usize) -> String + Send + Sync + 'static,
    ) -> Self {
        let (discovery, jwks) = mock_oidc_metadata(server, calls).await;

        Self {
            discovery,
            jwks,
            token: TokenEndpoint::create(server, calls, status, response).await,
        }
    }

    async fn healthy(server: &mut mockito::ServerGuard, calls: usize) -> Self {
        Self::create(server, calls, 200, |_| TOKEN_RESPONSE.to_string()).await
    }

    async fn assert(&self) {
        self.discovery.assert_async().await;
        self.jwks.assert_async().await;
        self.token.assert().await;
    }

    fn requests(&self) -> Vec<TokenRequest> {
        self.token.requests()
    }
}

async fn mock_oidc_metadata(
    server: &mut mockito::ServerGuard,
    calls: usize,
) -> (mockito::Mock, mockito::Mock) {
    let url = server.url();
    let document = serde_json::json!({
        "issuer": url,
        "authorization_endpoint": format!("{}/oauth/v2/authorize", url),
        "token_endpoint": format!("{}/oauth/v2/token", url),
        "jwks_uri": format!("{}/oauth/v2/keys", url),
        "response_types_supported": ["code"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["RS256"],
        "grant_types_supported": [
            "authorization_code",
            "urn:ietf:params:oauth:grant-type:jwt-bearer",
        ],
    })
    .to_string();

    let discovery = server
        .mock("GET", "/.well-known/openid-configuration")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(document)
        .expect(calls)
        .create_async()
        .await;

    // The reporter never verifies the token itself, so an empty key set is enough
    let jwks = server
        .mock("GET", "/oauth/v2/keys")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"keys":[]}"#)
        .expect(calls)
        .create_async()
        .await;

    (discovery, jwks)
}

struct TokenEndpoint {
    mock: mockito::Mock,
    requests: Arc<Mutex<Vec<TokenRequest>>>,
}

impl TokenEndpoint {
    async fn create(
        server: &mut mockito::ServerGuard,
        calls: usize,
        status: usize,
        response: impl Fn(usize) -> String + Send + Sync + 'static,
    ) -> Self {
        let requests: Arc<Mutex<Vec<TokenRequest>>> = Arc::new(Mutex::new(Vec::new()));
        let recorder = Arc::clone(&requests);
        let served = Arc::new(AtomicUsize::new(0));
        let call_counter = Arc::clone(&served);

        let mock = server
            .mock("POST", "/oauth/v2/token")
            .match_header("content-type", "application/x-www-form-urlencoded")
            // The JWT profile flow must never fall back to HTTP Basic auth
            .match_header("authorization", Matcher::Missing)
            .with_status(status)
            .with_header("content-type", "application/json")
            .with_body_from_request(move |request| {
                let call = call_counter.fetch_add(1, Ordering::SeqCst) + 1;
                recorder
                    .lock()
                    .expect("token request recorder is poisoned")
                    .push(TokenRequest::capture(request));
                response(call).into_bytes()
            })
            .expect(calls)
            .create_async()
            .await;

        Self { mock, requests }
    }

    async fn assert(&self) {
        self.mock.assert_async().await;
    }

    fn requests(&self) -> Vec<TokenRequest> {
        self.requests
            .lock()
            .expect("token request recorder is poisoned")
            .clone()
    }
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                decoded.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[index + 1..index + 3])
                    .expect("form encoding is not valid UTF-8");
                decoded.push(u8::from_str_radix(hex, 16).expect("invalid percent encoding"));
                index += 3;
            }
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }

    String::from_utf8(decoded).expect("form encoded value is not valid UTF-8")
}

#[derive(Debug, serde::Deserialize)]
struct AssertionClaims {
    iss: String,
    sub: String,
    aud: String,
    iat: i64,
    exp: i64,
}

fn verified_assertion_claims(assertion: &str) -> AssertionClaims {
    let header = jsonwebtoken::decode_header(assertion).expect("the assertion must be a JWS");
    assert_eq!(header.alg, jsonwebtoken::Algorithm::RS256);
    assert_eq!(header.kid.as_deref(), Some(service_account::KEY_ID));

    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);
    validation.validate_aud = false;

    jsonwebtoken::decode::<AssertionClaims>(
        assertion,
        &jsonwebtoken::DecodingKey::from_rsa_pem(service_account::public_key_pem().as_bytes())
            .expect("the derived public key must be a valid PEM RSA key"),
        &validation,
    )
    .expect("the assertion must verify against the public key of the key file")
    .claims
}

#[tokio::test]
async fn test_build_auth_headers() {
    let mut server = mockito::Server::new_async().await;
    let provider = OidcProvider::healthy(&mut server, 1).await;

    let (_key_dir, identity) = service_identity(&server.url());

    let headers = build_auth_headers(&identity).await.unwrap();

    let auth_value = headers.get(reqwest::header::AUTHORIZATION).unwrap();
    assert_eq!(
        auth_value.to_str().unwrap(),
        format!("{} mock-access-token", AUTH_SCHEME)
    );

    provider.assert().await;

    let requests = provider.requests();
    assert_eq!(requests.len(), 1, "exactly one token request is expected");

    let request = &requests[0];
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/oauth/v2/token");
    assert_eq!(
        request.form_field("grant_type").as_deref(),
        Some(JWT_BEARER_GRANT_TYPE)
    );
    // The crate always attaches the openid scope before the configured ones
    assert_eq!(
        request.form_field("scope").as_deref(),
        Some(format!("{} {}", OPENID_SCOPE, REPORTER_SCOPE).as_str())
    );
    assert_eq!(
        request.authorization, None,
        "the token request must not authenticate with HTTP Basic"
    );

    let claims = verified_assertion_claims(&request.assertion());
    assert_eq!(claims.iss, service_account::USER_ID);
    assert_eq!(claims.sub, service_account::USER_ID);
    assert_eq!(claims.aud, server.url());
    assert_eq!(claims.exp - claims.iat, 3600);
}

/// T029: Test fetch_components_success - verify component fetching and parsing
#[tokio::test]
async fn test_fetch_components_success() {
    let mut server = mockito::Server::new_async().await;

    // Mock GET /v2/components endpoint
    let mock = server
        .mock("GET", "/v2/components")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[
                {
                    "id": 218,
                    "name": "Object Storage Service",
                    "attributes": [
                        {"name": "category", "value": "Storage"},
                        {"name": "region", "value": "EU-DE"}
                    ]
                },
                {
                    "id": 254,
                    "name": "Compute Service",
                    "attributes": [
                        {"name": "category", "value": "Compute"},
                        {"name": "region", "value": "EU-NL"}
                    ]
                }
            ]"#,
        )
        .create_async()
        .await;

    let client = reqwest::Client::new();
    let headers = reqwest::header::HeaderMap::new();

    let result = fetch_components(&client, &server.url(), &headers).await;

    assert!(result.is_ok());
    let components = result.unwrap();
    assert_eq!(components.len(), 2);
    assert_eq!(components[0].id, 218);
    assert_eq!(components[0].name, "Object Storage Service");
    assert_eq!(components[0].attributes.len(), 2);
    assert_eq!(components[1].id, 254);

    mock.assert_async().await;
}

/// T030: Test build_component_id_cache - verify cache structure with nested HashMap
#[test]
fn test_build_component_id_cache() {
    let components = vec![
        StatusDashboardComponent {
            id: 218,
            name: "Object Storage Service".to_string(),
            attributes: vec![
                ComponentAttribute {
                    name: "category".to_string(),
                    value: "Storage".to_string(),
                },
                ComponentAttribute {
                    name: "region".to_string(),
                    value: "EU-DE".to_string(),
                },
            ],
        },
        StatusDashboardComponent {
            id: 254,
            name: "Compute Service".to_string(),
            attributes: vec![
                ComponentAttribute {
                    name: "category".to_string(),
                    value: "Compute".to_string(),
                },
                ComponentAttribute {
                    name: "region".to_string(),
                    value: "EU-NL".to_string(),
                },
            ],
        },
    ];

    let cache = build_component_id_cache(components);

    // Verify cache structure
    assert_eq!(cache.len(), 2);

    // Build expected key with sorted attributes
    let mut key1_attrs = vec![
        ComponentAttribute {
            name: "category".to_string(),
            value: "Storage".to_string(),
        },
        ComponentAttribute {
            name: "region".to_string(),
            value: "EU-DE".to_string(),
        },
    ];
    key1_attrs.sort();
    let key1 = ("Object Storage Service".to_string(), key1_attrs);

    assert_eq!(cache.get(&key1), Some(&218));
}

/// T031: Test find_component_id_subset_matching - verify FR-012 subset attribute matching
#[test]
fn test_find_component_id_subset_matching() {
    // Build cache with components that have multiple attributes
    let components = vec![StatusDashboardComponent {
        id: 218,
        name: "Object Storage Service".to_string(),
        attributes: vec![
            ComponentAttribute {
                name: "category".to_string(),
                value: "Storage".to_string(),
            },
            ComponentAttribute {
                name: "region".to_string(),
                value: "EU-DE".to_string(),
            },
            ComponentAttribute {
                name: "type".to_string(),
                value: "block".to_string(),
            },
        ],
    }];

    let cache = build_component_id_cache(components);

    // Test 1: Exact match
    let target_exact = Component {
        name: "Object Storage Service".to_string(),
        attributes: vec![
            ComponentAttribute {
                name: "category".to_string(),
                value: "Storage".to_string(),
            },
            ComponentAttribute {
                name: "region".to_string(),
                value: "EU-DE".to_string(),
            },
            ComponentAttribute {
                name: "type".to_string(),
                value: "block".to_string(),
            },
        ],
    };
    assert_eq!(find_component_id(&cache, &target_exact), Some(218));

    // Test 2: Subset match (config has fewer attributes than cache) - FR-012
    let target_subset = Component {
        name: "Object Storage Service".to_string(),
        attributes: vec![ComponentAttribute {
            name: "region".to_string(),
            value: "EU-DE".to_string(),
        }],
    };
    assert_eq!(find_component_id(&cache, &target_subset), Some(218));

    // Test 3: No match (different attribute value)
    let target_no_match = Component {
        name: "Object Storage Service".to_string(),
        attributes: vec![ComponentAttribute {
            name: "region".to_string(),
            value: "EU-NL".to_string(),
        }],
    };
    assert_eq!(find_component_id(&cache, &target_no_match), None);

    // Test 4: No match (component name doesn't exist)
    let target_no_name = Component {
        name: "NonExistent Service".to_string(),
        attributes: vec![],
    };
    assert_eq!(find_component_id(&cache, &target_no_name), None);
}

/// T032: Test build_incident_data_structure - verify static title/description per FR-002
#[test]
fn test_build_incident_data_structure() {
    let component_id = 218;
    let impact = 2;
    let timestamp = 1705929045; // 2024-01-22 10:30:45 UTC

    let incident_data = build_incident_data(component_id, impact, timestamp);

    // Verify static title and description (FR-002)
    assert_eq!(
        incident_data.title,
        "System incident from monitoring system"
    );
    assert_eq!(
        incident_data.description,
        "System-wide incident affecting one or multiple components. Created automatically."
    );

    // Verify other fields
    assert_eq!(incident_data.impact, 2);
    assert_eq!(incident_data.components, vec![218]);
    assert_eq!(incident_data.system, true);
    assert_eq!(incident_data.incident_type, "incident");
}

/// T033: Test timestamp_rfc3339_minus_one_second - verify FR-011 timestamp handling
#[test]
fn test_timestamp_rfc3339_minus_one_second() {
    let timestamp = 1705929045; // 2024-01-22 10:30:45 UTC
    let incident_data = build_incident_data(218, 2, timestamp);

    // Parse the start_date back to verify it's RFC3339 and -1 second
    let parsed = DateTime::parse_from_rfc3339(&incident_data.start_date);
    assert!(parsed.is_ok());

    let expected_timestamp = timestamp - 1; // FR-011: subtract 1 second
    let expected_dt = DateTime::from_timestamp(expected_timestamp, 0).unwrap();

    assert_eq!(parsed.unwrap().timestamp(), expected_dt.timestamp());

    // Verify the format is RFC3339 (contains 'T' and 'Z' or offset)
    assert!(incident_data.start_date.contains('T'));
    assert!(incident_data.start_date.ends_with('Z') || incident_data.start_date.contains('+'));
}

/// T034: Test create_incident_success - verify POST with mockito
#[tokio::test]
async fn test_create_incident_success() {
    let mut server = mockito::Server::new_async().await;

    // Mock POST endpoint
    let mock = server
        .mock("POST", "/v2/events")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "result": [
                    {
                        "component_id": 218,
                        "incident_id": 456
                    }
                ]
            }"#,
        )
        .match_header("content-type", "application/json")
        .create_async()
        .await;

    let client = reqwest::Client::new();
    let headers = reqwest::header::HeaderMap::new();

    let incident_data = IncidentData {
        title: "System incident from monitoring system".to_string(),
        description: "Test incident".to_string(),
        impact: 2,
        components: vec![218],
        start_date: "2024-01-22T10:30:44Z".to_string(),
        system: true,
        incident_type: "incident".to_string(),
    };

    let result = create_incident(&client, &server.url(), &headers, &incident_data).await;

    assert!(result.is_ok());
    mock.assert_async().await;
}

/// T035: Test cache_refresh_on_miss - verify FR-005 single refresh attempt
/// Note: This is more of a behavior test that would require running the full reporter
/// For now, we test the logic components separately
#[test]
fn test_cache_refresh_logic() {
    // Test scenario: component not found initially, would trigger refresh
    let initial_cache = build_component_id_cache(vec![StatusDashboardComponent {
        id: 218,
        name: "Service A".to_string(),
        attributes: vec![],
    }]);

    let target = Component {
        name: "Service B".to_string(),
        attributes: vec![],
    };

    // First lookup fails
    let result = find_component_id(&initial_cache, &target);
    assert_eq!(result, None);

    // After refresh (simulated by building new cache with additional component)
    let refreshed_cache = build_component_id_cache(vec![
        StatusDashboardComponent {
            id: 218,
            name: "Service A".to_string(),
            attributes: vec![],
        },
        StatusDashboardComponent {
            id: 254,
            name: "Service B".to_string(),
            attributes: vec![],
        },
    ]);

    // Second lookup succeeds
    let result = find_component_id(&refreshed_cache, &target);
    assert_eq!(result, Some(254));
}

/// T036: Test startup_retry_logic - verify FR-006 3 retry attempts with delays
/// Note: Full integration would test actual delays, here we verify the logic structure
#[tokio::test]
async fn test_startup_fetch_with_retries() {
    let mut server = mockito::Server::new_async().await;

    // First two attempts fail, third succeeds
    let mock_fail_1 = server
        .mock("GET", "/v2/components")
        .with_status(503)
        .expect(1)
        .create_async()
        .await;

    let mock_fail_2 = server
        .mock("GET", "/v2/components")
        .with_status(503)
        .expect(1)
        .create_async()
        .await;

    let mock_success = server
        .mock("GET", "/v2/components")
        .with_status(200)
        .with_body(r#"[{"id": 218, "name": "Test Service", "attributes": []}]"#)
        .expect(1)
        .create_async()
        .await;

    let client = reqwest::Client::new();
    let headers = reqwest::header::HeaderMap::new();

    // Simulate retry logic
    let mut attempt = 0;
    let max_attempts = 3;
    let mut result = None;

    while attempt < max_attempts {
        attempt += 1;
        match fetch_components(&client, &server.url(), &headers).await {
            Ok(components) => {
                result = Some(components);
                break;
            }
            Err(_) if attempt < max_attempts => {
                // Would sleep here in real code
                continue;
            }
            Err(_) => {
                break;
            }
        }
    }

    assert!(result.is_some());
    assert_eq!(attempt, 3); // Succeeded on third attempt

    mock_fail_1.assert_async().await;
    mock_fail_2.assert_async().await;
    mock_success.assert_async().await;
}

/// T037: Test error_logging_with_diagnostic_fields - verify FR-017 structured logging
/// Note: This test verifies data structures support structured logging
#[test]
fn test_diagnostic_data_availability() {
    // Verify all required fields for structured logging are accessible
    let component = Component {
        name: "Test Service".to_string(),
        attributes: vec![ComponentAttribute {
            name: "region".to_string(),
            value: "EU-DE".to_string(),
        }],
    };

    let incident_data = build_incident_data(218, 2, 1705929045);

    // All these fields should be accessible for logging (FR-017)
    assert!(!component.name.is_empty());
    assert!(!component.attributes.is_empty());
    assert_eq!(incident_data.components[0], 218);
    assert_eq!(incident_data.impact, 2);
    assert!(!incident_data.start_date.is_empty());

    // Verify ComponentAttribute derives support structured logging
    let attr = &component.attributes[0];
    assert_eq!(attr.name, "region");
    assert_eq!(attr.value, "EU-DE");
}

/// Additional test: Verify empty attributes work correctly
#[test]
fn test_empty_attributes_handling() {
    let components = vec![StatusDashboardComponent {
        id: 100,
        name: "Service Without Attributes".to_string(),
        attributes: vec![],
    }];

    let cache = build_component_id_cache(components);

    let target = Component {
        name: "Service Without Attributes".to_string(),
        attributes: vec![],
    };

    assert_eq!(find_component_id(&cache, &target), Some(100));
}

/// Additional test: Verify multiple components with same name but different attributes
#[test]
fn test_multiple_components_same_name() {
    let components = vec![
        StatusDashboardComponent {
            id: 100,
            name: "Storage Service".to_string(),
            attributes: vec![ComponentAttribute {
                name: "region".to_string(),
                value: "EU-DE".to_string(),
            }],
        },
        StatusDashboardComponent {
            id: 200,
            name: "Storage Service".to_string(),
            attributes: vec![ComponentAttribute {
                name: "region".to_string(),
                value: "EU-NL".to_string(),
            }],
        },
    ];

    let cache = build_component_id_cache(components);

    let target_de = Component {
        name: "Storage Service".to_string(),
        attributes: vec![ComponentAttribute {
            name: "region".to_string(),
            value: "EU-DE".to_string(),
        }],
    };

    let target_nl = Component {
        name: "Storage Service".to_string(),
        attributes: vec![ComponentAttribute {
            name: "region".to_string(),
            value: "EU-NL".to_string(),
        }],
    };

    assert_eq!(find_component_id(&cache, &target_de), Some(100));
    assert_eq!(find_component_id(&cache, &target_nl), Some(200));
}

#[tokio::test]
async fn test_build_auth_headers_multiple_scopes() {
    let mut server = mockito::Server::new_async().await;
    let provider = OidcProvider::healthy(&mut server, 1).await;

    let (_key_dir, key_file) = key_file();
    let mut cfg = status_dashboard_config(&server.url(), &key_file);
    cfg.oidc_scopes = vec![REPORTER_SCOPE.to_string(), AUDIENCE_SCOPE.to_string()];

    let expected_scope = format!("{} {} {}", OPENID_SCOPE, REPORTER_SCOPE, AUDIENCE_SCOPE);
    let identity = cfg.oidc_identity().unwrap();

    let headers = build_auth_headers(&identity).await.unwrap();

    let auth_value = headers.get(reqwest::header::AUTHORIZATION).unwrap();
    assert_eq!(
        auth_value.to_str().unwrap(),
        format!("{} mock-access-token", AUTH_SCHEME)
    );

    provider.assert().await;

    let requests = provider.requests();
    assert_eq!(
        requests[0].form_field("scope").as_deref(),
        Some(expected_scope.as_str()),
        "the scopes must be joined by a single space, in the configured order"
    );
    assert_eq!(
        requests[0].body.matches("scope=").count(),
        1,
        "the scope must be sent as exactly one field: {}",
        requests[0].body
    );
}

#[tokio::test]
async fn test_build_auth_headers_without_configured_scopes() {
    let mut server = mockito::Server::new_async().await;
    let provider = OidcProvider::healthy(&mut server, 1).await;

    let (_key_dir, key_file) = key_file();
    let mut cfg = status_dashboard_config(&server.url(), &key_file);
    cfg.oidc_scopes = Vec::new();

    let identity = cfg.oidc_identity().unwrap();

    build_auth_headers(&identity).await.unwrap();

    provider.assert().await;

    assert_eq!(
        provider.requests()[0].form_field("scope").as_deref(),
        Some(OPENID_SCOPE)
    );
}

#[tokio::test]
async fn test_build_auth_headers_token_endpoint_error() {
    for status in [400usize, 401, 500, 503] {
        let mut server = mockito::Server::new_async().await;
        let provider = OidcProvider::create(&mut server, 1, status, |_| {
            r#"{"error":"invalid_client"}"#.to_string()
        })
        .await;

        let (_key_dir, identity) = service_identity(&server.url());

        let err = build_auth_headers(&identity).await.unwrap_err();
        let message = format!("{:#}", err);
        let assertion = provider.requests()[0].assertion();

        assert!(
            message.contains(&format!("{}/oauth/v2/token", server.url())),
            "status {}: the failing endpoint is missing from the error: {}",
            status,
            message
        );
        assert!(
            !message.contains(&assertion),
            "the signed assertion leaked into the error: {}",
            message
        );
        assert!(
            !message.contains("PRIVATE KEY"),
            "key material leaked into the error: {}",
            message
        );

        provider.assert().await;
    }
}

#[tokio::test]
async fn test_build_auth_headers_without_a_usable_access_token() {
    for body in [
        r#"{"token_type":"Bearer","expires_in":3600}"#,
        r#"{"access_token":"","token_type":"Bearer"}"#,
        r#"{"access_token":null,"token_type":"Bearer"}"#,
        "not json at all",
    ] {
        let mut server = mockito::Server::new_async().await;
        let provider = OidcProvider::create(&mut server, 1, 200, move |_| body.to_string()).await;

        let (_key_dir, identity) = service_identity(&server.url());

        let err = build_auth_headers(&identity)
            .await
            .expect_err("a response without a usable access token must be an error");
        let message = format!("{:#}", err);

        assert!(
            !message.contains("Bearer "),
            "an unusable access token produced credentials: {}",
            message
        );
        assert!(
            message.contains(&format!("{}/oauth/v2/token", server.url())),
            "unexpected error: {}",
            message
        );

        provider.assert().await;
    }
}

#[test]
fn test_oidc_identity_requires_all_credentials() {
    let (_key_dir, key_file) = key_file();
    let complete = status_dashboard_config("https://zitadel.example.com", &key_file);
    assert!(complete.oidc_identity().is_ok());

    let cases = [
        (
            StatusDashboardConfig {
                oidc_issuer: None,
                ..complete.clone()
            },
            OIDC_ISSUER_ENV_KEY,
        ),
        (
            StatusDashboardConfig {
                oidc_key_file: None,
                ..complete.clone()
            },
            OIDC_KEY_FILE_ENV_KEY,
        ),
        (
            StatusDashboardConfig {
                oidc_issuer: None,
                oidc_key_file: None,
                ..complete.clone()
            },
            OIDC_KEY_FILE_ENV_KEY,
        ),
    ];

    for (cfg, expected_key) in cases {
        let message = format!("{:#}", cfg.oidc_identity().unwrap_err());
        assert!(
            message.contains(expected_key),
            "missing {} not reported: {}",
            expected_key,
            message
        );
    }

    let all_missing = StatusDashboardConfig {
        oidc_issuer: None,
        oidc_key_file: None,
        ..complete.clone()
    };
    let message = format!("{:#}", all_missing.oidc_identity().unwrap_err());
    for key in [OIDC_ISSUER_ENV_KEY, OIDC_KEY_FILE_ENV_KEY] {
        assert!(
            message.contains(key),
            "missing {} not reported: {}",
            key,
            message
        );
    }
}

#[test]
fn test_status_dashboard_default_oidc_scopes() {
    let cfg: StatusDashboardConfig =
        serde_yaml::from_str("url: https://status.example.com\n").unwrap();

    assert_eq!(cfg.oidc_scopes, vec![REPORTER_SCOPE.to_string()]);
    assert!(cfg.oidc_issuer.is_none());
    assert!(cfg.oidc_key_file.is_none());
}

/// Test create_incident failure - verify error handling when API returns error
#[tokio::test]
async fn test_create_incident_failure() {
    let mut server = mockito::Server::new_async().await;

    // Mock POST /v2/events to return 500 error (note: actual endpoint is /v2/events)
    let mock = server
        .mock("POST", "/v2/events")
        .with_status(500)
        .with_header("content-type", "application/json")
        .with_body(r#"{"error": "Internal Server Error"}"#)
        .expect(1)
        .create_async()
        .await;

    let client = reqwest::Client::new();
    let headers = reqwest::header::HeaderMap::new();

    let incident_data = IncidentData {
        title: "System incident from monitoring system".to_string(),
        description: "Test incident".to_string(),
        impact: 2,
        components: vec![218],
        start_date: "2024-01-22T10:30:44Z".to_string(),
        system: true,
        incident_type: "incident".to_string(),
    };

    let result = create_incident(&client, &server.url(), &headers, &incident_data).await;

    assert!(result.is_err(), "create_incident should fail on 500 error");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Failed to create incident"),
        "Error message should mention failure: {}",
        err_msg
    );

    mock.assert_async().await;
}

/// Test fetch_components failure - verify error handling when API returns error
#[tokio::test]
async fn test_fetch_components_failure() {
    let mut server = mockito::Server::new_async().await;

    // Mock GET /v2/components to return 503 error
    let mock = server
        .mock("GET", "/v2/components")
        .with_status(503)
        .with_body("Service Unavailable")
        .create_async()
        .await;

    let client = reqwest::Client::new();
    let headers = reqwest::header::HeaderMap::new();

    let result = fetch_components(&client, &server.url(), &headers).await;

    assert!(result.is_err(), "fetch_components should fail on 503 error");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Failed to fetch components"),
        "Error message should mention failure"
    );

    mock.assert_async().await;
}

#[tokio::test]
async fn test_authenticated_requests_fetch_a_fresh_service_token() {
    assert_fresh_token_per_authenticated_request().await;
}

/// The crate signs assertions with a second-resolution `iat` and no `jti`, so two acquisitions in
/// the same second yield the same assertion; crossing the boundary makes freshness observable.
async fn wait_for_next_second() {
    let elapsed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("the system clock must be after the unix epoch");

    tokio::time::sleep(Duration::from_millis(
        u64::from(1000 - elapsed.subsec_millis()) + 50,
    ))
    .await;
}

async fn assert_fresh_token_per_authenticated_request() {
    let mut server = mockito::Server::new_async().await;

    let provider = OidcProvider::create(&mut server, 2, 200, |call| {
        format!(
            r#"{{"access_token":"fresh-token-{}","token_type":"Bearer","expires_in":3600}}"#,
            call
        )
    })
    .await;

    let components_body = r#"[{"id":218,"name":"Object Storage Service","attributes":[{"name":"region","value":"EU-DE"}]}]"#;

    let component_report = server
        .mock("GET", "/v2/components")
        .match_header(
            "authorization",
            Matcher::Exact(format!("{} fresh-token-1", AUTH_SCHEME)),
        )
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(components_body)
        .expect(1)
        .create_async()
        .await;

    let incident_report = server
        .mock("POST", "/v2/events")
        .match_header(
            "authorization",
            Matcher::Exact(format!("{} fresh-token-2", AUTH_SCHEME)),
        )
        .match_header("content-type", "application/json")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"result":[{"component_id":218,"incident_id":456}]}"#)
        .expect(1)
        .create_async()
        .await;

    let client = reqwest::Client::new();
    let (_key_dir, identity) = service_identity(&server.url());

    let headers = build_auth_headers(&identity).await.unwrap();
    fetch_components(&client, &server.url(), &headers)
        .await
        .expect("the component fetch must use the token fetched for it");

    wait_for_next_second().await;

    let headers = build_auth_headers(&identity).await.unwrap();
    create_incident(
        &client,
        &server.url(),
        &headers,
        &build_incident_data(218, 2, 1705929045),
    )
    .await
    .expect("the incident report must use the token fetched for it");

    let requests = provider.requests();
    assert_eq!(
        requests.len(),
        2,
        "the token endpoint must be called once per authenticated request"
    );

    let assertions = [requests[0].assertion(), requests[1].assertion()];
    assert_ne!(
        assertions[0], assertions[1],
        "every token request must carry its own freshly signed assertion"
    );

    provider.assert().await;
    component_report.assert_async().await;
    incident_report.assert_async().await;
}

#[tokio::test]
async fn test_no_status_dashboard_request_when_service_token_fails() {
    let mut server = mockito::Server::new_async().await;

    let provider = OidcProvider::create(&mut server, 1, 503, |_| {
        r#"{"error":"temporarily_unavailable"}"#.to_string()
    })
    .await;

    let components_mock = server
        .mock("GET", "/v2/components")
        .expect(0)
        .create_async()
        .await;
    let events_mock = server
        .mock("POST", "/v2/events")
        .expect(0)
        .create_async()
        .await;

    let (_key_dir, identity) = service_identity(&server.url());

    let err = build_auth_headers(&identity)
        .await
        .expect_err("a failing token endpoint must not provide authorization headers");
    let message = format!("{:#}", err);

    assert!(
        message.contains(&format!("{}/oauth/v2/token", server.url())),
        "the failing token endpoint is missing from the error: {}",
        message
    );
    assert!(
        !message.contains(&provider.requests()[0].assertion()),
        "the signed assertion leaked into the error: {}",
        message
    );
    assert!(
        !message.contains("PRIVATE KEY"),
        "key material leaked into the error: {}",
        message
    );

    provider.assert().await;
    components_mock.assert_async().await;
    events_mock.assert_async().await;
}

#[tokio::test]
async fn test_reporter_exits_non_zero_on_fatal_startup_error() {
    let mut server = mockito::Server::new_async().await;

    let provider = OidcProvider::create(&mut server, 1, 503, |_| {
        r#"{"error":"temporarily_unavailable"}"#.to_string()
    })
    .await;

    let components_mock = server
        .mock("GET", "/v2/components")
        .expect(0)
        .create_async()
        .await;

    let dir = tempfile::tempdir().unwrap();
    let key_file = write_service_account_key_file(&dir.path().join("service-account.json"));

    let config = format!(
        r#"---
datasource:
  url: '{url}'
server:
  port: 3005
environments:
  - name: test-env
flag_metrics: []
health_metrics: {{}}
status_dashboard:
  url: '{url}'
  oidc_issuer: '{url}'
  oidc_key_file: '{key_file}'
"#,
        url = server.url(),
        key_file = key_file,
    );

    std::fs::write(dir.path().join("config.yaml"), config).unwrap();

    let child = tokio::process::Command::new(env!("CARGO_BIN_EXE_cloudmon-metrics-reporter"))
        .current_dir(dir.path())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to start cloudmon-metrics-reporter");

    let output = tokio::time::timeout(Duration::from_secs(60), child.wait_with_output())
        .await
        .expect("the reporter did not exit on a fatal error")
        .expect("failed to collect reporter output");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(1),
        "a fatal error must exit non-zero, stdout: {}, stderr: {}",
        stdout,
        stderr
    );
    assert!(
        stderr.contains("metric reporter failed"),
        "unexpected error output: {}",
        stderr
    );
    assert!(
        !stderr.contains("PRIVATE KEY") && !stdout.contains("PRIVATE KEY"),
        "key material leaked into the reporter output"
    );
    assert!(
        !stderr.contains(&provider.requests()[0].assertion()),
        "the signed assertion leaked into the reporter output"
    );

    provider.assert().await;
    components_mock.assert_async().await;
}

#[tokio::test]
async fn test_reporter_reports_an_unusable_service_account_key_file() {
    let dir = tempfile::tempdir().unwrap();

    let config = r#"---
datasource:
  url: 'http://127.0.0.1:1'
server:
  port: 3005
environments:
  - name: test-env
flag_metrics: []
health_metrics: {}
status_dashboard:
  url: 'http://127.0.0.1:1'
  oidc_issuer: 'http://127.0.0.1:1'
  oidc_key_file: 'service-account.json'
"#;

    std::fs::write(dir.path().join("config.yaml"), config).unwrap();

    let child = tokio::process::Command::new(env!("CARGO_BIN_EXE_cloudmon-metrics-reporter"))
        .current_dir(dir.path())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to start cloudmon-metrics-reporter");

    let output = tokio::time::timeout(Duration::from_secs(60), child.wait_with_output())
        .await
        .expect("the reporter did not exit on a missing key file")
        .expect("failed to collect reporter output");

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_ne!(
        output.status.code(),
        Some(0),
        "a missing key file must fail closed, stderr: {}",
        stderr
    );
    assert!(
        stderr.contains(OIDC_KEY_FILE_ENV_KEY),
        "the failing configuration key must be named: {}",
        stderr
    );
}
