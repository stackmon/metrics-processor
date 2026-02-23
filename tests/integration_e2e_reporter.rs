//! # E2E Integration Tests for Reporter Log Validation
//!
//! This module contains end-to-end integration tests that validate the complete
//! metrics-processor pipeline using real Docker containers (go-carbon + carbonapi).
//!
//! ## Overview
//!
//! These tests verify that the reporter correctly:
//! - Fetches metrics from Graphite
//! - Evaluates health expressions
//! - Creates incidents with correct severity
//! - Logs all required fields for observability
//!
//! ## Prerequisites
//!
//! ### Docker
//! Docker must be installed and running. The test automatically manages containers.
//!
//! ### Ports
//! The following ports must be available:
//! - `2003` - Carbon plaintext protocol (metrics ingestion)
//! - `8080` - CarbonAPI (Graphite-compatible query API)
//! - `3005` - Convertor API
//! - `9999` - Mock Status Dashboard
//!
//! ## Running the Tests
//!
//! ### Full E2E Test (recommended)
//! ```bash
//! cargo test --test integration_e2e_reporter -- --ignored --nocapture
//! ```
//!
//! ### Unit Tests Only (no Docker required)
//! ```bash
//! cargo test --test integration_e2e_reporter
//! ```
//!
//! ## Test Scenarios
//!
//! | Scenario | Weight | Expression | Triggered Metrics |
//! |----------|--------|------------|-------------------|
//! | healthy | 0 | none | [] |
//! | degraded_slow | 1 | `api_slow \|\| api_success_rate_low` | [api_slow] |
//! | degraded_errors | 1 | `api_slow \|\| api_success_rate_low` | [api_success_rate_low] |
//! | outage | 2 | `api_down` | [api_down, api_success_rate_low] |
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
//! │  Test Code  │────▶│  go-carbon  │────▶│  carbonapi  │
//! │(write data) │     │  (storage)  │     │   (query)   │
//! └─────────────┘     └─────────────┘     └─────────────┘
//!                                                │
//!       ┌─────────────────────────────────────────┘
//!       ▼
//! ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
//! │  Convertor  │────▶│  Reporter   │────▶│ Mock Status │
//! │  (process)  │     │   (alert)   │     │  Dashboard  │
//! └─────────────┘     └─────────────┘     └─────────────┘
//!       │                   │
//!       │                   ▼
//!       │             ┌─────────────┐
//!       └────────────▶│  Log Output │◀── Test validates
//!                     │  (stdout)   │
//!                     └─────────────┘
//! ```
//!
//! ## How It Works
//!
//! 1. **Docker Setup**: Test restarts Docker containers to ensure clean Graphite data
//! 2. **Build Binaries**: Compiles convertor and reporter binaries once
//! 3. **For Each Scenario**:
//!    - Generates scenario-specific config (unique service name for data isolation)
//!    - Starts mock Status Dashboard (Python HTTP server)
//!    - Starts convertor binary
//!    - Writes test metrics to Graphite via TCP to Carbon (port 2003)
//!    - Starts reporter binary and captures stdout
//!    - Validates log output contains expected fields
//!    - Cleans up processes
//!
//! ## Data Isolation
//!
//! Each scenario uses a unique service name (e.g., `rms_healthy`, `rms_outage`) to
//! prevent data from one scenario affecting another. This allows all scenarios to
//! run sequentially without clearing Graphite between tests.
//!
//! ## Troubleshooting
//!
//! ### "Docker containers failed to start"
//! - Ensure Docker is running: `docker ps`
//! - Check port availability: `lsof -i :2003 -i :8080`
//!
//! ### "Convertor not ready"
//! - Increase timeout in `wait_for_convertor()`
//! - Check convertor logs for errors
//!
//! ### "No incident log found"
//! - Verify Graphite received data: `curl 'http://localhost:8080/metrics/find?query=stats.*'`
//! - Check go-carbon scan frequency in `tests/docker/go-carbon.conf`
//!
//! ### "Log validation failed"
//! - Check for ANSI escape codes in output (test strips them)
//! - Verify expected expression matches config

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use regex::Regex;

const GRAPHITE_URL: &str = "http://localhost:8080";
const CARBON_HOST: &str = "localhost";
const CARBON_PORT: u16 = 2003;
const CONVERTOR_PORT: u16 = 3005;
const STATUS_DASHBOARD_PORT: u16 = 9999;

// ============================================================================
// Test Infrastructure
// ============================================================================

/// Check if Graphite/CarbonAPI is available
async fn is_graphite_available() -> bool {
    let client = reqwest::Client::new();
    match client
        .get(format!("{}/render?format=json", GRAPHITE_URL))
        .timeout(Duration::from_secs(5))
        .send()
        .await
    {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

/// Send metric to Carbon (go-carbon) via TCP
fn send_metric(metric_path: &str, value: f64, timestamp: i64) -> bool {
    let metric_line = format!("{} {} {}\n", metric_path, value, timestamp);
    match TcpStream::connect(format!("{}:{}", CARBON_HOST, CARBON_PORT)) {
        Ok(mut stream) => {
            stream.set_write_timeout(Some(Duration::from_secs(5))).ok();
            match stream.write_all(metric_line.as_bytes()) {
                Ok(_) => {
                    println!("  sent: {} = {} @ {}", metric_path, value, timestamp);
                    true
                }
                Err(e) => {
                    eprintln!("  failed to write: {}", e);
                    false
                }
            }
        }
        Err(e) => {
            eprintln!("  failed to connect to carbon: {}", e);
            false
        }
    }
}

/// Test scenario configuration with expected log patterns
///
/// ## Metric Thresholds (from config)
///
/// The test config defines these thresholds for health evaluation:
/// - `api_slow`: response_time > 1200ms (weight=1, degraded)
/// - `api_success_rate_low`: success_rate < 65% (weight=1, degraded)
/// - `api_down`: failed_count == attempted_count (weight=2, outage)
///
/// ## How Metrics Are Calculated
///
/// - `success_rate` = success_count / attempted_count * 100
/// - `response_time` = timer mean value in milliseconds
/// - `api_down` = true when all requests fail (failed_count == attempted_count)
#[derive(Debug, Clone)]
struct TestScenario {
    name: &'static str,
    description: &'static str,
    // Metric values
    failed_count: f64,
    attempted_count: f64,
    response_time_ms: f64,
    success_count: f64,
    // Expected results
    expected_weight: u8,
    // Expected log patterns (what reporter should log)
    expect_incident_log: bool,
}

impl TestScenario {
    /// Get expected expression based on scenario
    fn expected_expression(&self) -> Option<String> {
        let service = format!("rms_{}", self.name);
        match self.name {
            "healthy" => None,
            "degraded_slow" | "degraded_errors" => Some(format!(
                "{}.api_slow || {}.api_success_rate_low",
                service, service
            )),
            "outage" => Some(format!("{}.api_down", service)),
            _ => None,
        }
    }

    /// Get expected triggered metrics based on scenario
    fn expected_triggered_metrics(&self) -> Vec<String> {
        let service = format!("rms_{}", self.name);
        match self.name {
            "healthy" => vec![],
            "degraded_slow" => vec![format!("{}.api_slow", service)],
            "degraded_errors" => vec![format!("{}.api_success_rate_low", service)],
            "outage" => vec![
                format!("{}.api_down", service),
                format!("{}.api_success_rate_low", service),
            ],
            _ => vec![],
        }
    }

    /// Healthy scenario: all metrics within normal thresholds
    /// - response_time: 500ms < 1200ms threshold (OK)
    /// - success_rate: 99/100 = 99% > 65% threshold (OK)
    /// - failed_count: 0 != attempted_count (not down)
    /// Result: no incident (weight=0)
    fn healthy() -> Self {
        TestScenario {
            name: "healthy",
            description: "All metrics healthy - no incident expected",
            failed_count: 0.0,
            attempted_count: 100.0,
            response_time_ms: 500.0,
            success_count: 99.0,
            expected_weight: 0,
            expect_incident_log: false,
        }
    }

    /// Degraded (slow) scenario: response time exceeds threshold
    /// - response_time: 1500ms > 1200ms threshold (TRIGGERS api_slow)
    /// - success_rate: 99/100 = 99% > 65% threshold (OK)
    /// Result: degraded incident (weight=1)
    fn degraded_slow() -> Self {
        TestScenario {
            name: "degraded_slow",
            description: "API slow - degraded incident expected (weight=1)",
            failed_count: 0.0,
            attempted_count: 100.0,
            response_time_ms: 1500.0,
            success_count: 99.0,
            expected_weight: 1,
            expect_incident_log: true,
        }
    }

    /// Degraded (errors) scenario: success rate below threshold
    /// - response_time: 500ms < 1200ms threshold (OK)
    /// - success_rate: 50/100 = 50% < 65% threshold (TRIGGERS api_success_rate_low)
    /// Result: degraded incident (weight=1)
    fn degraded_errors() -> Self {
        TestScenario {
            name: "degraded_errors",
            description: "Low success rate - degraded incident expected (weight=1)",
            failed_count: 0.0,
            attempted_count: 100.0,
            response_time_ms: 500.0,
            success_count: 50.0,
            expected_weight: 1,
            expect_incident_log: true,
        }
    }

    /// Outage scenario: all requests failed
    /// - failed_count: 100 == attempted_count: 100 (TRIGGERS api_down, weight=2)
    /// - success_rate: 0/100 = 0% < 65% threshold (also triggers api_success_rate_low)
    /// Result: outage incident (weight=2, highest severity wins)
    fn outage() -> Self {
        TestScenario {
            name: "outage",
            description: "API down - outage incident expected (weight=2)",
            failed_count: 100.0,
            attempted_count: 100.0,
            response_time_ms: 0.0,
            success_count: 0.0,
            expected_weight: 2,
            expect_incident_log: true,
        }
    }
}

/// Write test data to Graphite for a scenario
/// Uses scenario-specific metric paths to isolate data between scenarios
fn write_scenario_data(scenario: &TestScenario, base_timestamp: i64) {
    println!("\npopulating data for scenario: {}", scenario.name);
    println!("   {}", scenario.description);

    // Use scenario name in path to isolate data between scenarios
    let base = format!(
        "stats.counters.openstack.api.production_eu-de.identity.rms_{}.v3.tokens",
        scenario.name
    );
    let timer_base = format!(
        "stats.timers.openstack.api.production_eu-de.identity.rms_{}.v3.tokens.GET",
        scenario.name
    );

    // Send data at multiple timestamps to ensure coverage across the query window
    // Graphite aggregates at minute boundaries, so we send at 0, 60, 120, 180 seconds back
    for offset in [0, 60, 120, 180] {
        let timestamp = base_timestamp - offset;
        send_metric(
            &format!("{}.failed.count", base),
            scenario.failed_count,
            timestamp,
        );
        send_metric(
            &format!("{}.attempted.count", base),
            scenario.attempted_count,
            timestamp,
        );
        send_metric(
            &format!("{}.mean", timer_base),
            scenario.response_time_ms,
            timestamp,
        );
        send_metric(
            &format!("{}.200.count", base),
            scenario.success_count,
            timestamp,
        );
    }

    // Give Graphite time to process and persist
    // After container restart, Graphite needs more time to be fully ready
    println!("   waiting for graphite to process data...");
    std::thread::sleep(Duration::from_secs(10));
}

// ============================================================================
// Expected Log Entry Patterns
// ============================================================================

/// Expected log entry fields for validation
#[derive(Debug, Clone)]
struct ExpectedLogEntry {
    environment: String,
    service: String,
    component_name: String,
    impact: u8,
    matched_expression: String,
    triggered_metrics_contain: Vec<String>,
}

impl ExpectedLogEntry {
    fn from_scenario(scenario: &TestScenario) -> Option<Self> {
        if !scenario.expect_incident_log {
            return None;
        }

        Some(ExpectedLogEntry {
            environment: "production_eu-de".to_string(),
            service: "config".to_string(),
            component_name: "Config".to_string(),
            impact: scenario.expected_weight,
            matched_expression: scenario
                .expected_expression()
                .unwrap_or_else(|| "none".to_string()),
            triggered_metrics_contain: scenario.expected_triggered_metrics(),
        })
    }
}

/// Validate that a log line contains expected fields
fn validate_log_line(log_line: &str, expected: &ExpectedLogEntry) -> Vec<String> {
    let mut errors = Vec::new();

    // Strip ANSI escape codes (color codes from tracing)
    // ANSI codes are in format \x1b[...m where ... is numbers/semicolons
    let re = Regex::new(r"\x1b\[[0-9;]*m").unwrap();
    let clean_log = re.replace_all(log_line, "").to_string();

    // Check environment field
    let env_pattern = format!("environment=\"{}\"", expected.environment);
    if !clean_log.contains(&env_pattern) {
        errors.push(format!(
            "Missing or wrong environment: expected '{}' in log",
            env_pattern
        ));
    }

    // Check service field
    let service_pattern = format!("service=\"{}\"", expected.service);
    if !clean_log.contains(&service_pattern) {
        errors.push(format!(
            "Missing or wrong service: expected '{}' in log",
            service_pattern
        ));
    }

    // Check component_name field
    let component_pattern = format!("component_name=\"{}\"", expected.component_name);
    if !clean_log.contains(&component_pattern) {
        errors.push(format!(
            "Missing or wrong component_name: expected '{}' in log",
            component_pattern
        ));
    }

    // Check impact field
    let impact_pattern = format!("impact={}", expected.impact);
    if !clean_log.contains(&impact_pattern) {
        errors.push(format!(
            "Missing or wrong impact: expected '{}' in log",
            impact_pattern
        ));
    }

    // Check matched_expression field
    let expr_pattern = format!("matched_expression=\"{}\"", expected.matched_expression);
    if !clean_log.contains(&expr_pattern) {
        errors.push(format!(
            "Missing or wrong matched_expression: expected '{}' in log",
            expr_pattern
        ));
    }

    // Check triggered_metrics contains expected metric names
    for metric in &expected.triggered_metrics_contain {
        if !clean_log.contains(metric) {
            errors.push(format!(
                "triggered_metrics missing '{}' in log line",
                metric
            ));
        }
    }

    // Verify the log message indicates incident creation
    if !clean_log.contains("creating incident") {
        errors.push("Missing 'creating incident' message in log".to_string());
    }

    errors
}

// ============================================================================
// Process Management
// ============================================================================

/// Kill any existing process on a port
fn kill_process_on_port(port: u16) {
    // Try to kill any existing process on the port
    let _ = Command::new("lsof")
        .args(["-ti", &format!(":{}", port)])
        .output()
        .map(|output| {
            if output.status.success() {
                let pids = String::from_utf8_lossy(&output.stdout);
                for pid in pids.trim().lines() {
                    if let Ok(pid_num) = pid.trim().parse::<i32>() {
                        let _ = Command::new("kill").arg(pid_num.to_string()).output();
                    }
                }
            }
        });
    std::thread::sleep(Duration::from_millis(100));
}

/// Start mock Status Dashboard server
fn start_mock_status_dashboard() -> Option<Child> {
    // Clean up any existing process on the port
    kill_process_on_port(STATUS_DASHBOARD_PORT);

    // Use a Python HTTP server that supports IPv4/IPv6 and runs indefinitely
    let mock_server = Command::new("python3")
        .args([
            "-c",
            &format!(
                r#"
import http.server
import json
import socketserver
import socket

class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if '/v2/components' in self.path:
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.end_headers()
            response = json.dumps([
                {{"id": 218, "name": "Config", "attributes": [{{"name": "region", "value": "EU-DE"}}]}}
            ])
            self.wfile.write(response.encode())
        else:
            self.send_response(404)
            self.end_headers()

    def do_POST(self):
        if '/v2/events' in self.path:
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.end_headers()
            response = json.dumps({{"result": [{{"component_id": 218, "incident_id": 1}}]}})
            self.wfile.write(response.encode())
        else:
            self.send_response(404)
            self.end_headers()

    def log_message(self, format, *args):
        pass  # Suppress logging

class DualStackTCPServer(socketserver.TCPServer):
    address_family = socket.AF_INET6
    allow_reuse_address = True
    
    def server_bind(self):
        self.socket.setsockopt(socket.IPPROTO_IPV6, socket.IPV6_V6ONLY, 0)
        super().server_bind()

server = DualStackTCPServer(('::', {}), Handler)
server.serve_forever()
"#,
                STATUS_DASHBOARD_PORT
            ),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();

    match mock_server {
        Ok(child) => {
            // Wait for server to be ready with retry loop
            let start = std::time::Instant::now();
            let timeout = Duration::from_secs(5);
            let mut ready = false;

            while start.elapsed() < timeout {
                match std::net::TcpStream::connect_timeout(
                    &format!("127.0.0.1:{}", STATUS_DASHBOARD_PORT)
                        .parse()
                        .unwrap(),
                    Duration::from_millis(100),
                ) {
                    Ok(_) => {
                        ready = true;
                        break;
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(100)),
                }
            }

            if ready {
                println!(
                    "mock status dashboard started on port {}",
                    STATUS_DASHBOARD_PORT
                );
                Some(child)
            } else {
                eprintln!("mock status dashboard not ready after timeout");
                None
            }
        }
        Err(e) => {
            eprintln!("failed to start mock status dashboard: {}", e);
            None
        }
    }
}

/// Start the convertor process
#[allow(dead_code)]
fn start_convertor(config_path: &str) -> Option<Child> {
    let convertor = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "cloudmon-metrics-convertor",
            "--",
            "-c",
            config_path,
        ])
        .env("RUST_LOG", "info")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    match convertor {
        Ok(child) => {
            // Give convertor time to start
            std::thread::sleep(Duration::from_secs(3));
            println!("convertor started");
            Some(child)
        }
        Err(e) => {
            eprintln!("failed to start convertor: {}", e);
            None
        }
    }
}

/// Start the reporter process and capture its output
#[allow(dead_code)]
fn start_reporter_with_output_capture(
    config_path: &str,
) -> Option<(Child, Arc<Mutex<Vec<String>>>)> {
    let reporter = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "cloudmon-metrics-reporter",
            "--",
            "-c",
            config_path,
        ])
        .env("RUST_LOG", "info")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    match reporter {
        Ok(mut child) => {
            let logs: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
            let logs_clone = logs.clone();

            // Capture stderr (where tracing logs go)
            if let Some(stderr) = child.stderr.take() {
                thread::spawn(move || {
                    let reader = BufReader::new(stderr);
                    for line in reader.lines().map_while(Result::ok) {
                        println!("  [reporter] {}", line);
                        let mut log_vec = logs_clone.lock().unwrap();
                        log_vec.push(line);
                    }
                });
            }

            // Give reporter time to start
            std::thread::sleep(Duration::from_secs(2));
            println!("reporter started with log capture");
            Some((child, logs))
        }
        Err(e) => {
            eprintln!("failed to start reporter: {}", e);
            None
        }
    }
}

/// Check if convertor API is ready
async fn wait_for_convertor(timeout_secs: u64) -> bool {
    let client = reqwest::Client::new();
    let start = std::time::Instant::now();

    while start.elapsed().as_secs() < timeout_secs {
        match client
            .get(format!("http://localhost:{}/api/v1", CONVERTOR_PORT))
            .timeout(Duration::from_secs(2))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                println!("convertor api ready at port {}", CONVERTOR_PORT);
                return true;
            }
            _ => {
                std::thread::sleep(Duration::from_millis(500));
            }
        }
    }

    eprintln!("convertor api not ready after {} seconds", timeout_secs);
    false
}

// ============================================================================
// E2E Tests
// ============================================================================

/// Restart docker containers to clear graphite data
/// This ensures each test run starts with clean state
fn restart_docker_containers() -> bool {
    println!("restarting docker containers to clear graphite data...");

    // Stop containers
    let stop = Command::new("docker")
        .args([
            "compose",
            "-f",
            "tests/docker/docker-compose.yml",
            "down",
            "-v",
        ])
        .output();

    if let Err(e) = stop {
        eprintln!("warning: failed to stop containers: {}", e);
    }

    // Start containers
    let start = Command::new("docker")
        .args([
            "compose",
            "-f",
            "tests/docker/docker-compose.yml",
            "up",
            "-d",
        ])
        .output();

    match start {
        Ok(result) if result.status.success() => {
            println!("docker containers restarted");
            // Wait for services to be ready - graphite needs time to initialize
            println!("waiting for graphite to be ready...");
            std::thread::sleep(Duration::from_secs(15));
            true
        }
        Ok(result) => {
            eprintln!(
                "failed to start containers: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            false
        }
        Err(e) => {
            eprintln!("failed to run docker compose: {}", e);
            false
        }
    }
}

/// Build binaries once before running tests
fn build_binaries() -> bool {
    println!("building binaries...");
    let output = Command::new("cargo")
        .args([
            "build",
            "--bin",
            "cloudmon-metrics-convertor",
            "--bin",
            "cloudmon-metrics-reporter",
        ])
        .output();

    match output {
        Ok(result) => {
            if result.status.success() {
                println!("binaries built successfully");
                true
            } else {
                eprintln!(
                    "failed to build binaries: {}",
                    String::from_utf8_lossy(&result.stderr)
                );
                false
            }
        }
        Err(e) => {
            eprintln!("failed to run cargo build: {}", e);
            false
        }
    }
}

/// Get path to compiled binary
fn get_binary_path(name: &str) -> String {
    format!("./target/debug/{}", name)
}

/// Generate config for a specific scenario
/// Uses scenario-specific service name to isolate data between scenarios
fn generate_config(scenario_name: &str) -> String {
    // Use scenario-specific service name (e.g., "rms_healthy", "rms_outage")
    let service = format!("rms_{}", scenario_name);

    format!(
        r#"
datasource:
  url: '{}'
  timeout: 30

server:
  port: {}
  address: '0.0.0.0'

status_dashboard:
  url: 'http://localhost:{}'
  jwt_secret: 'test-secret-key'

metric_templates:
  api_down:
    query: "asPercent(smartSummarize(sumSeries(stats.counters.openstack.api.$environment.*.$service.*.*.failed.count), '1min', 'average', '1min'), smartSummarize(sumSeries(stats.counters.openstack.api.$environment.*.$service.*.*.attempted.count), '1min', 'average', '1min'))"
    op: "eq"
    threshold: 100

  api_slow:
    query: "smartSummarize(consolidateBy(aggregate(stats.timers.openstack.api.$environment.*.$service.*.*.*.mean, 'average'), 'average'), '3min', 'average')"
    op: "gt"
    threshold: 1200

  api_success_rate_low:
    query: "smartSummarize(asPercent(sumSeries(stats.counters.openstack.api.$environment.*.$service.*.*.{{{{2*,3*,404}}}}.count), sumSeries(stats.counters.openstack.api.$environment.*.$service.*.*.attempted.count)), '3min', 'average')"
    op: "lt"
    threshold: 65

environments:
  - name: production_eu-de
    attributes:
      region: EU-DE

flag_metrics:
  - name: "api_down"
    service: "{}"
    template:
      name: "api_down"
    environments:
      - name: "production_eu-de"

  - name: "api_slow"
    service: "{}"
    template:
      name: "api_slow"
    environments:
      - name: "production_eu-de"

  - name: "api_success_rate_low"
    service: "{}"
    template:
      name: "api_success_rate_low"
    environments:
      - name: "production_eu-de"

health_metrics:
  config:
    service: {}
    component_name: "Config"
    category: management
    metrics:
      - {}.api_slow
      - {}.api_down
      - {}.api_success_rate_low
    expressions:
      - expression: "{}.api_slow || {}.api_success_rate_low"
        weight: 1
      - expression: "{}.api_down"
        weight: 2

health_query:
  query_from: "-5min"
  query_to: "-1min"
"#,
        GRAPHITE_URL,
        CONVERTOR_PORT,
        STATUS_DASHBOARD_PORT,
        service,
        service,
        service,
        service,
        service,
        service,
        service,
        service,
        service,
        service
    )
}

/// Main E2E test that runs all scenarios and validates reporter log output
#[tokio::test]
#[ignore] // Run with: cargo test --test integration_e2e_reporter -- --ignored --nocapture
async fn test_e2e_reporter_log_validation() {
    println!("\ne2e reporter log validation test");
    println!("====================================\n");

    // Restart docker containers to ensure clean graphite data
    // This prevents stale data from previous test runs affecting results
    assert!(
        restart_docker_containers(),
        "failed to restart docker containers"
    );

    // Check if Graphite is available - FAIL if not
    assert!(
        is_graphite_available().await,
        "graphite not available at {}. start with: cd tests/docker && docker compose up -d",
        GRAPHITE_URL
    );
    println!("graphite is available at {}\n", GRAPHITE_URL);

    // Build binaries first
    assert!(build_binaries(), "failed to build binaries");

    // Test each scenario - now with isolated metric paths per scenario
    let scenarios = vec![
        TestScenario::healthy(),
        TestScenario::degraded_slow(),
        TestScenario::degraded_errors(),
        TestScenario::outage(),
    ];

    let mut all_passed = true;
    let mut scenarios_run = 0;
    let config_path = "config.yaml";

    for scenario in scenarios {
        println!("\n============================================================");
        println!("test scenario: {}", scenario.name.to_uppercase());
        println!("   {}", scenario.description);
        println!("============================================================");

        // Generate per-scenario config with unique service name to isolate data
        let config_content = generate_config(&scenario.name);
        std::fs::write(config_path, &config_content).expect("failed to write config file");
        println!("scenario config written to {}", config_path);

        // Start mock Status Dashboard
        let mut mock_sd = start_mock_status_dashboard();
        assert!(
            mock_sd.is_some(),
            "failed to start mock status dashboard for scenario: {}",
            scenario.name
        );

        // Start convertor using pre-built binary (uses config.yaml in current dir)
        let convertor_bin = get_binary_path("cloudmon-metrics-convertor");
        let mut convertor = match Command::new(&convertor_bin)
            .env("RUST_LOG", "info")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                if let Some(ref mut sd) = mock_sd {
                    let _ = sd.kill();
                }
                panic!(
                    "failed to start convertor for scenario {}: {}",
                    scenario.name, e
                );
            }
        };

        // Wait for convertor to be ready
        std::thread::sleep(Duration::from_secs(2));
        if !wait_for_convertor(15).await {
            let _ = convertor.kill();
            if let Some(ref mut sd) = mock_sd {
                let _ = sd.kill();
            }
            panic!(
                "convertor not ready after 15 seconds for scenario: {}",
                scenario.name
            );
        }

        // Write test data to Graphite - use current time as base
        // The function will send data at multiple timestamps (now, now-60, now-120, now-180)
        let timestamp = chrono::Utc::now().timestamp();
        write_scenario_data(&scenario, timestamp);

        // Start reporter using pre-built binary and capture logs (uses config.yaml in current dir)
        let reporter_bin = get_binary_path("cloudmon-metrics-reporter");
        let logs: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let mut reporter = match Command::new(&reporter_bin)
            .env("RUST_LOG", "info")
            .stdout(Stdio::piped()) // Capture stdout, not stderr - reporter logs to stdout
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(mut r) => {
                // Start stdout reader thread immediately
                if let Some(stdout) = r.stdout.take() {
                    let logs_clone = logs.clone();
                    thread::spawn(move || {
                        let reader = BufReader::new(stdout);
                        for line in reader.lines() {
                            match line {
                                Ok(l) => {
                                    println!("  [reporter] {}", l);
                                    logs_clone.lock().unwrap().push(l);
                                }
                                Err(_) => break,
                            }
                        }
                    });
                }
                r
            }
            Err(e) => {
                let _ = convertor.kill();
                if let Some(ref mut sd) = mock_sd {
                    let _ = sd.kill();
                }
                panic!(
                    "failed to start reporter for scenario {}: {}",
                    scenario.name, e
                );
            }
        };

        println!("   reporter started (pid: {:?})", reporter.id());
        scenarios_run += 1;

        // Wait for reporter to process metrics (one iteration)
        println!("   waiting for reporter to process metrics...");
        std::thread::sleep(Duration::from_secs(10));

        // Check if reporter is still running
        match reporter.try_wait() {
            Ok(Some(status)) => println!("   reporter exited early with status: {:?}", status),
            Ok(None) => println!("   reporter is still running"),
            Err(e) => println!("   error checking reporter status: {}", e),
        }

        // Stop reporter
        let _ = reporter.kill();
        let _ = reporter.wait();

        // Give the reader thread time to finish reading
        std::thread::sleep(Duration::from_millis(500));

        // Get captured logs
        let captured_logs = logs.lock().unwrap().clone();

        // Print captured logs for debugging
        println!("   captured {} log lines", captured_logs.len());
        for line in &captured_logs {
            println!("  [reporter] {}", line);
        }

        // Validate log output
        println!("\nvalidating log output for scenario: {}", scenario.name);

        if let Some(expected) = ExpectedLogEntry::from_scenario(&scenario) {
            // Find the incident creation log line
            let incident_log = captured_logs
                .iter()
                .find(|line| line.contains("creating incident"));

            match incident_log {
                Some(log_line) => {
                    println!("   found incident log: {}", log_line);

                    let errors = validate_log_line(log_line, &expected);
                    if errors.is_empty() {
                        println!("   all log fields validated successfully");
                    } else {
                        println!("   log validation errors:");
                        for err in &errors {
                            println!("      - {}", err);
                        }
                        all_passed = false;
                    }

                    // Print expected vs actual comparison
                    println!("\n   expected log fields:");
                    println!("      environment=\"{}\"", expected.environment);
                    println!("      service=\"{}\"", expected.service);
                    println!("      component_name=\"{}\"", expected.component_name);
                    println!("      impact={}", expected.impact);
                    println!(
                        "      matched_expression=\"{}\"",
                        expected.matched_expression
                    );
                    println!(
                        "      triggered_metrics should contain: {:?}",
                        expected.triggered_metrics_contain
                    );
                }
                None => {
                    println!("   expected incident log not found!");
                    println!("   captured logs ({} lines):", captured_logs.len());
                    for (i, line) in captured_logs.iter().enumerate().take(20) {
                        println!("      {}: {}", i, line);
                    }
                    all_passed = false;
                }
            }
        } else {
            // Healthy scenario - should NOT have incident log
            let has_incident = captured_logs
                .iter()
                .any(|line| line.contains("creating incident"));

            if has_incident {
                println!("   unexpected incident log found for healthy scenario!");
                all_passed = false;
            } else {
                println!("   no incident log (expected for healthy scenario)");
            }
        }

        // Cleanup
        let _ = convertor.kill();
        if let Some(ref mut sd) = mock_sd {
            let _ = sd.kill();
        }

        // Brief pause between scenarios
        std::thread::sleep(Duration::from_secs(2));
    }

    // Clean up config file
    let _ = std::fs::remove_file(config_path);

    // Ensure all scenarios were run
    assert_eq!(
        scenarios_run, 4,
        "expected to run 4 scenarios, but only ran {}",
        scenarios_run
    );

    println!("\n============================================================");
    if all_passed {
        println!(
            "all e2e reporter tests passed ({} scenarios)",
            scenarios_run
        );
    } else {
        println!("some e2e reporter tests failed");
    }
    println!("============================================================\n");

    assert!(
        all_passed,
        "e2e reporter tests failed - see output above for details"
    );
}

/// Helper test to verify log line validation logic
#[test]
fn test_log_line_validation() {
    let expected = ExpectedLogEntry {
        environment: "production_eu-de".to_string(),
        service: "config".to_string(),
        component_name: "Config".to_string(),
        impact: 1,
        matched_expression: "rms.api_slow || rms.api_success_rate_low".to_string(),
        triggered_metrics_contain: vec!["rms.api_slow".to_string()],
    };

    // Test with valid log line
    let valid_log = r#"2024-01-22T10:30:45.123456Z  INFO cloudmon_metrics_reporter: environment="production_eu-de" service="config" component_name="Config" component_id=218 query_from="-5min" query_to="-1min" metric_timestamp=1705929045 impact=1 triggered_metrics=["rms.api_slow(query=..., op=gt, threshold=1200)"] matched_expression="rms.api_slow || rms.api_success_rate_low" creating incident: health metric indicates service degradation"#;

    let errors = validate_log_line(valid_log, &expected);
    assert!(errors.is_empty(), "Valid log should pass: {:?}", errors);

    // Test with missing field
    let invalid_log = r#"environment="wrong_env" service="config" impact=1 matched_expression="rms.api_slow || rms.api_success_rate_low" creating incident"#;

    let errors = validate_log_line(invalid_log, &expected);
    assert!(!errors.is_empty(), "Invalid log should have errors");
    assert!(
        errors.iter().any(|e| e.contains("environment")),
        "Should detect wrong environment"
    );
}

/// Test scenario field population
#[test]
fn test_scenario_expected_log_entries() {
    // Healthy scenario should not produce incident log
    let healthy = TestScenario::healthy();
    assert!(!healthy.expect_incident_log);
    assert!(ExpectedLogEntry::from_scenario(&healthy).is_none());

    // Degraded slow should produce incident log
    let degraded = TestScenario::degraded_slow();
    assert!(degraded.expect_incident_log);
    let expected = ExpectedLogEntry::from_scenario(&degraded).unwrap();
    assert_eq!(expected.impact, 1);
    assert_eq!(
        expected.matched_expression,
        "rms_degraded_slow.api_slow || rms_degraded_slow.api_success_rate_low"
    );

    // Outage should produce incident log with weight=2
    let outage = TestScenario::outage();
    assert!(outage.expect_incident_log);
    let expected = ExpectedLogEntry::from_scenario(&outage).unwrap();
    assert_eq!(expected.impact, 2);
    assert_eq!(expected.matched_expression, "rms_outage.api_down");
}
