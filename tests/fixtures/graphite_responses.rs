// Mock Graphite response data for testing
//
// Provides JSON response fixtures for various Graphite API scenarios

use serde_json::json;

/// Valid Graphite response with multiple datapoints
pub fn valid_response_with_data() -> serde_json::Value {
    json!([
        {
            "target": "stats.test-service.production.errors",
            "datapoints": [
                [3.0, 1640000000],
                [2.5, 1640000060],
                [4.0, 1640000120],
                [1.8, 1640000180]
            ]
        }
    ])
}

/// Response with single datapoint
pub fn single_datapoint_response() -> serde_json::Value {
    json!([
        {
            "target": "stats.api.latency",
            "datapoints": [
                [450.0, 1640000000]
            ]
        }
    ])
}

/// Response with empty datapoints array
pub fn empty_datapoints_response() -> serde_json::Value {
    json!([
        {
            "target": "stats.missing.metric",
            "datapoints": []
        }
    ])
}

/// Response with null values in datapoints
pub fn null_values_response() -> serde_json::Value {
    json!([
        {
            "target": "stats.metric.with.nulls",
            "datapoints": [
                [10.0, 1640000000],
                [null, 1640000060],
                [15.0, 1640000120],
                [null, 1640000180],
                [20.0, 1640000240]
            ]
        }
    ])
}

/// Response with all null values
pub fn all_null_response() -> serde_json::Value {
    json!([
        {
            "target": "stats.all.nulls",
            "datapoints": [
                [null, 1640000000],
                [null, 1640000060],
                [null, 1640000120]
            ]
        }
    ])
}

/// Response with multiple metrics
pub fn multi_metric_response() -> serde_json::Value {
    json!([
        {
            "target": "stats.cpu",
            "datapoints": [
                [75.0, 1640000000],
                [78.5, 1640000060]
            ]
        },
        {
            "target": "stats.memory",
            "datapoints": [
                [85.0, 1640000000],
                [88.2, 1640000060]
            ]
        },
        {
            "target": "stats.disk",
            "datapoints": [
                [60.0, 1640000000],
                [62.5, 1640000060]
            ]
        }
    ])
}

/// Response for metric flag evaluation - value below threshold
pub fn below_threshold_response(value: f64) -> serde_json::Value {
    json!([
        {
            "target": "test.metric",
            "datapoints": [
                [value, 1640000000]
            ]
        }
    ])
}

/// Response for metric flag evaluation - value above threshold
pub fn above_threshold_response(value: f64) -> serde_json::Value {
    json!([
        {
            "target": "test.metric",
            "datapoints": [
                [value, 1640000000]
            ]
        }
    ])
}

/// Response for metric flag evaluation - value equal to threshold
pub fn equal_threshold_response(value: f64) -> serde_json::Value {
    json!([
        {
            "target": "test.metric",
            "datapoints": [
                [value, 1640000000]
            ]
        }
    ])
}

/// Response with boundary values near threshold
pub fn boundary_values_response(threshold: f64) -> serde_json::Value {
    json!([
        {
            "target": "test.metric",
            "datapoints": [
                [threshold - 0.001, 1640000000],
                [threshold, 1640000060],
                [threshold + 0.001, 1640000120]
            ]
        }
    ])
}

/// Response with negative values
pub fn negative_values_response() -> serde_json::Value {
    json!([
        {
            "target": "test.negative",
            "datapoints": [
                [-10.5, 1640000000],
                [-5.0, 1640000060],
                [-2.3, 1640000120],
                [0.0, 1640000180]
            ]
        }
    ])
}

/// Response with zero values
pub fn zero_values_response() -> serde_json::Value {
    json!([
        {
            "target": "test.zeros",
            "datapoints": [
                [0.0, 1640000000],
                [0.0, 1640000060],
                [0.0, 1640000120]
            ]
        }
    ])
}

/// Response with very large values
pub fn large_values_response() -> serde_json::Value {
    json!([
        {
            "target": "test.large",
            "datapoints": [
                [999999.99, 1640000000],
                [1000000.0, 1640000060],
                [1500000.5, 1640000120]
            ]
        }
    ])
}

/// Response with decimal precision values
pub fn precise_values_response() -> serde_json::Value {
    json!([
        {
            "target": "test.precise",
            "datapoints": [
                [1.234567, 1640000000],
                [2.345678, 1640000060],
                [3.456789, 1640000120]
            ]
        }
    ])
}

/// Response for /metrics/find endpoint - root level
pub fn find_root_response() -> serde_json::Value {
    json!([
        {
            "text": "flag",
            "id": "flag",
            "leaf": 0,
            "expandable": 1,
            "allowChildren": 1
        },
        {
            "text": "health",
            "id": "health",
            "leaf": 0,
            "expandable": 1,
            "allowChildren": 1
        }
    ])
}

/// Response for /metrics/find endpoint - service level
pub fn find_services_response() -> serde_json::Value {
    json!([
        {
            "text": "api",
            "id": "flag.api",
            "leaf": 0,
            "expandable": 1,
            "allowChildren": 1
        },
        {
            "text": "database",
            "id": "flag.database",
            "leaf": 0,
            "expandable": 1,
            "allowChildren": 1
        }
    ])
}

/// Response for /metrics/find endpoint - environment level
pub fn find_environments_response() -> serde_json::Value {
    json!([
        {
            "text": "production",
            "id": "flag.api.production",
            "leaf": 0,
            "expandable": 1,
            "allowChildren": 1
        },
        {
            "text": "staging",
            "id": "flag.api.staging",
            "leaf": 0,
            "expandable": 1,
            "allowChildren": 1
        }
    ])
}

/// Response for /metrics/find endpoint - metric level (leaf nodes)
pub fn find_metrics_response() -> serde_json::Value {
    json!([
        {
            "text": "error_rate",
            "id": "flag.api.production.error_rate",
            "leaf": 1,
            "expandable": 0,
            "allowChildren": 0
        },
        {
            "text": "response_time",
            "id": "flag.api.production.response_time",
            "leaf": 1,
            "expandable": 0,
            "allowChildren": 0
        }
    ])
}

/// Malformed JSON response
pub fn malformed_json() -> &'static str {
    r#"{"target": "test", "datapoints": [invalid json here}"#
}

/// HTTP 404 error response body
pub fn not_found_error() -> &'static str {
    r#"{"error": "Metric not found"}"#
}

/// HTTP 500 error response body
pub fn server_error() -> &'static str {
    r#"{"error": "Internal server error"}"#
}

/// Response with mixed valid and null datapoints for aggregation testing
pub fn mixed_datapoints_response() -> serde_json::Value {
    json!([
        {
            "target": "stats.metric1",
            "datapoints": [
                [10.0, 1640000000],
                [20.0, 1640000060]
            ]
        },
        {
            "target": "stats.metric2",
            "datapoints": [
                [null, 1640000000],
                [30.0, 1640000060]
            ]
        },
        {
            "target": "stats.metric3",
            "datapoints": [
                [40.0, 1640000000],
                [null, 1640000060]
            ]
        }
    ])
}

/// Response for health calculation with all metrics passing thresholds
pub fn all_passing_health_response() -> serde_json::Value {
    json!([
        {
            "target": "stats.api.cpu",
            "datapoints": [[60.0, 1640000000]]
        },
        {
            "target": "stats.api.memory",
            "datapoints": [[70.0, 1640000000]]
        },
        {
            "target": "stats.api.errors",
            "datapoints": [[2.0, 1640000000]]
        }
    ])
}

/// Response for health calculation with some metrics failing
pub fn partial_failing_health_response() -> serde_json::Value {
    json!([
        {
            "target": "stats.api.cpu",
            "datapoints": [[95.0, 1640000000]]  // Failing
        },
        {
            "target": "stats.api.memory",
            "datapoints": [[70.0, 1640000000]]  // Passing
        },
        {
            "target": "stats.api.errors",
            "datapoints": [[2.0, 1640000000]]   // Passing
        }
    ])
}

/// Response for health calculation with all metrics failing
pub fn all_failing_health_response() -> serde_json::Value {
    json!([
        {
            "target": "stats.api.cpu",
            "datapoints": [[95.0, 1640000000]]
        },
        {
            "target": "stats.api.memory",
            "datapoints": [[95.0, 1640000000]]
        },
        {
            "target": "stats.api.errors",
            "datapoints": [[100.0, 1640000000]]
        }
    ])
}
