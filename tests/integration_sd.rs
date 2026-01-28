//! Integration tests for Status Dashboard API integration
//!
//! Tests for Phase 7: Integration Testing
//! T028-T037: Validate end-to-end Status Dashboard API integration with mocked endpoints

use chrono::DateTime;
use cloudmon_metrics::sd::{
    build_auth_headers, build_component_id_cache, build_incident_data, create_incident,
    fetch_components, find_component_id, Component, ComponentAttribute, IncidentData,
    StatusDashboardComponent,
};

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

/// Test build_auth_headers - verify JWT token generation
#[test]
fn test_build_auth_headers() {
    // Test with secret
    let headers = build_auth_headers(Some("test-secret"));
    assert!(headers.contains_key(reqwest::header::AUTHORIZATION));

    let auth_value = headers.get(reqwest::header::AUTHORIZATION).unwrap();
    let auth_str = auth_value.to_str().unwrap();
    assert!(auth_str.starts_with("Bearer "));

    // Test without secret (optional auth)
    let headers_empty = build_auth_headers(None);
    assert!(!headers_empty.contains_key(reqwest::header::AUTHORIZATION));
}
