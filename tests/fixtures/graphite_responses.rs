// Mock Graphite response data for testing
//
// Provides JSON response fixtures for various Graphite API scenarios

use serde_json::json;

/// Standard CPU metric response for webapp with multiple datapoints
pub fn webapp_cpu_response() -> serde_json::Value {
    json!([
        {
            "target": "webapp.cpu-usage",
            "datapoints": [
                [85.5, 1609459200],
                [90.2, 1609459260],
                [75.0, 1609459320]
            ]
        }
    ])
}

/// Health metrics response for api-service (cpu, memory, error_rate)
pub fn api_service_health_response(
    cpu: f64,
    memory: f64,
    error_rate: f64,
    timestamp: i64,
) -> serde_json::Value {
    json!([
        {"target": "api-service.cpu_usage", "datapoints": [[cpu, timestamp]]},
        {"target": "api-service.memory_usage", "datapoints": [[memory, timestamp]]},
        {"target": "api-service.error_rate", "datapoints": [[error_rate, timestamp]]}
    ])
}

/// Health metrics response with empty datapoints for all metrics
pub fn api_service_empty_response() -> serde_json::Value {
    json!([
        {"target": "api-service.cpu_usage", "datapoints": []},
        {"target": "api-service.memory_usage", "datapoints": []},
        {"target": "api-service.error_rate", "datapoints": []}
    ])
}

/// Health metrics response with partial data (some metrics missing datapoints)
pub fn api_service_partial_response(
    cpu: f64,
    error_rate: f64,
    timestamp: i64,
) -> serde_json::Value {
    json!([
        {"target": "api-service.cpu_usage", "datapoints": [[cpu, timestamp]]},
        {"target": "api-service.memory_usage", "datapoints": []},
        {"target": "api-service.error_rate", "datapoints": [[error_rate, timestamp]]}
    ])
}
