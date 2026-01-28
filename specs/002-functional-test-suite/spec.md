# Feature Specification: Comprehensive Functional Test Suite

**Feature Branch**: `002-functional-test-suite`  
**Created**: 2025-01-24  
**Status**: Draft  
**Input**: User description: "Create a feature specification for comprehensive functional tests for the metrics-processor project.

User requirements:
- As a new developer or QA, I need to be sure that main business functionality works as expected
- I need functional tests for the whole project
- The user plans to refactor the code base and add new features
- They need confidence that the main functionality won't change during refactoring
- Minimum 95% test coverage for the main business functions is required

The spec should cover:
1. Identifying all main business functions in the codebase
2. Creating functional/integration tests that verify business logic
3. Ensuring 95%+ coverage of core business functionality
4. Tests should serve as regression protection during refactoring"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Core Metric Flag Evaluation Testing (Priority: P1)

As a developer refactoring the metrics processing logic, I need comprehensive tests that verify metric flag evaluation (comparison operators Lt/Gt/Eq) works correctly across all threshold scenarios, so I can confidently refactor without breaking core business logic.

**Why this priority**: This is the foundation of the entire system - converting raw numeric metrics to boolean flags. If this breaks, the entire health monitoring system fails. This function (`get_metric_flag_state`) is called for every metric evaluation and is critical for accurate monitoring.

**Independent Test**: Can be fully tested by providing numeric values and metric configurations with different comparison operators, then verifying the boolean flag output matches expected results. Delivers immediate value by preventing false positives/negatives in health monitoring.

**Acceptance Scenarios**:

1. **Given** a metric value of 85 and a threshold of 90 with Lt operator, **When** evaluating flag state, **Then** system returns true (85 < 90)
2. **Given** a metric value of 95 and a threshold of 90 with Gt operator, **When** evaluating flag state, **Then** system returns true (95 > 90)
3. **Given** a metric value of 90 and a threshold of 90 with Eq operator, **When** evaluating flag state, **Then** system returns true (90 == 90)
4. **Given** a metric value of 85 and a threshold of 90 with Gt operator, **When** evaluating flag state, **Then** system returns false (85 not > 90)
5. **Given** multiple metrics with mixed operators in a service, **When** evaluating all flags, **Then** each flag correctly reflects its comparison result

---

### User Story 2 - Service Health Aggregation Testing (Priority: P1)

As a QA engineer, I need tests that verify service health calculation correctly fetches metrics, evaluates boolean expressions, applies weights, and returns the highest-weighted health status, so that monitoring dashboards show accurate service health.

**Why this priority**: This is the most complex business function (`get_service_health`) that orchestrates the entire health evaluation workflow. It combines multiple metrics using boolean expressions (AND/OR) and weighted scoring. Incorrect health status can lead to missed incidents or false alarms.

**Independent Test**: Can be fully tested by mocking Graphite responses with known metric values, defining weighted expressions, and verifying the returned health status matches expected priority. Delivers value by ensuring monitoring accuracy.

**Acceptance Scenarios**:

1. **Given** a service with 2 metrics (both true) and expression "metric1 OR metric2" with weight 3, **When** calculating health, **Then** system returns impact value 3
2. **Given** a service with 3 weighted expressions (weights: 5, 3, 1) where only weight-3 expression evaluates true, **When** calculating health, **Then** system returns impact value 3 (highest true expression)
3. **Given** a service where all expressions evaluate false, **When** calculating health, **Then** system returns impact value 0
4. **Given** a service with AND expression "metric1 AND metric2" where metric1 is true but metric2 is false, **When** calculating health, **Then** expression evaluates false
5. **Given** a service in an unknown environment, **When** calculating health, **Then** system returns appropriate error without crashing

---

### User Story 3 - API Endpoint Integration Testing (Priority: P2)

As a developer adding new API features, I need integration tests for all REST endpoints (/api/v1/health, /render, /metrics/find) that verify request handling, response formats, error handling, and Graphite integration, so I can ensure the API contract remains stable during refactoring.

**Why this priority**: API endpoints are the external interface used by dashboards and other services. Breaking changes affect all consumers. Currently these endpoints have no automated tests, making refactoring risky.

**Independent Test**: Can be fully tested by starting a test server, sending HTTP requests with various parameters, and validating response structure and status codes. Delivers value by protecting the API contract.

**Acceptance Scenarios**:

1. **Given** a running API server, **When** GET /api/v1/health?service=myservice&environment=production, **Then** response contains valid ServiceHealthResponse JSON with status 200
2. **Given** a running API server, **When** GET /render?target=flag.prod.myservice.metric1, **Then** response contains time-series data with boolean values
3. **Given** a running API server, **When** GET /metrics/find?query=flag.*, **Then** response contains list of matching metrics with expandable flag
4. **Given** a request for non-existent service, **When** querying health endpoint, **Then** response returns 404 or appropriate error with message
5. **Given** invalid query parameters, **When** calling any endpoint, **Then** response returns 400 with clear error description

---

### User Story 4 - Configuration Processing Testing (Priority: P2)

As a new team member, I need tests that verify configuration loading, template variable substitution ($environment, $service), and metric initialization work correctly across all configuration scenarios, so I understand how configuration changes affect system behavior.

**Why this priority**: Configuration processing (`AppState::process_config`) is the initialization step that sets up all metrics and expressions. Errors here prevent the system from starting or cause incorrect metric mappings. This has one test but needs comprehensive coverage.

**Independent Test**: Can be fully tested by providing various YAML configurations with templates and variables, then verifying the resulting AppState contains correctly expanded metric definitions and expression mappings. Delivers documentation value through test examples.

**Acceptance Scenarios**:

1. **Given** a config with template "flag.$environment.$service.cpu" and environments [prod, dev], **When** processing config, **Then** system creates metric mappings for flag.prod.*.cpu and flag.dev.*.cpu
2. **Given** a config with health expression containing dashes "api-gateway", **When** processing config, **Then** system converts to "api_gateway" for expression evaluation
3. **Given** a config file, conf.d directory with overrides, and environment variables with MP_ prefix, **When** loading config, **Then** system merges all sources with correct precedence
4. **Given** a config with invalid YAML syntax, **When** loading config, **Then** system returns clear error message with line number
5. **Given** a config with missing required fields, **When** validating config, **Then** system returns error listing all missing fields

---

### User Story 5 - Graphite Integration Testing (Priority: P3)

As a developer working on TSDB integration, I need tests that verify Graphite client query building, response parsing, and error handling for network failures or malformed data, so I can safely refactor the Graphite client without breaking monitoring.

**Why this priority**: Graphite integration (`get_graphite_data`, `find_metrics`) is essential for data retrieval, but failures here are easier to debug and less critical than core business logic. The client already has some test coverage but needs comprehensive scenarios.

**Independent Test**: Can be fully tested by mocking Graphite HTTP responses with various data formats and error conditions, then verifying correct parsing or error handling. Delivers value by ensuring reliable external integration.

**Acceptance Scenarios**:

1. **Given** a mock Graphite server returning valid JSON with datapoints, **When** querying metrics, **Then** client correctly parses values and timestamps
2. **Given** a mock Graphite server returning empty datapoints array, **When** querying metrics, **Then** client handles gracefully without errors
3. **Given** Graphite server returns HTTP 500 error, **When** querying metrics, **Then** client returns appropriate error with context
4. **Given** Graphite server times out, **When** querying metrics, **Then** client returns timeout error after configured duration
5. **Given** metric discovery query for "flag.prod.*", **When** calling find_metrics, **Then** client returns list of expandable nodes at that level

---

### User Story 6 - Regression Test Suite for Refactoring (Priority: P1)

As a developer refactoring the codebase, I need a comprehensive regression test suite that runs quickly (under 2 minutes) and fails immediately when business logic changes, so I can refactor code structure confidently without changing behavior.

**Why this priority**: This is the primary goal - enabling safe refactoring. The test suite must cover 95%+ of business logic and serve as a safety net. Without this, refactoring is risky and slow.

**Independent Test**: Can be fully tested by running the complete test suite after making intentional breaking changes to business logic, and verifying tests catch the breakage. Delivers immediate refactoring confidence.

**Acceptance Scenarios**:

1. **Given** complete test suite covering all business functions, **When** running tests, **Then** all tests pass in under 2 minutes
2. **Given** a deliberate change to metric comparison logic (swap Lt/Gt), **When** running tests, **Then** core metric tests fail with clear error messages
3. **Given** a deliberate change to health calculation weights, **When** running tests, **Then** health aggregation tests fail
4. **Given** refactored code with same behavior but different structure, **When** running tests, **Then** all tests still pass
5. **Given** test suite in CI/CD pipeline, **When** pull request is created, **Then** tests run automatically and block merge if failing

---

## Clarifications

### Session 2025-01-24

- Q: For Graphite integration testing, which mocking approach should the test suite use? → A: HTTP mock server (e.g., wiremock/mockito) - Realistic integration, tests full HTTP stack
- Q: How should test data fixtures (sample configs, metric values, expected outputs) be organized and managed? → A: Shared fixtures per module - Reusable, DRY, good for consistency
- Q: Should tests run in parallel or sequentially to meet the 2-minute execution goal? → A: Parallel by default (cargo test) - Fast, requires careful isolation
- Q: What approach should be used for test assertions and failure diagnostics to ensure clear error messages? → A: Balanced approach - standard assertions for unit tests, custom assertions with business context for functional/integration tests
- Q: Which code coverage tool should be used to measure and enforce the 95% coverage requirement? → A: cargo-tarpaulin - Rust-native, accurate line coverage

### Edge Cases

- What happens when Graphite returns null or NaN values for metrics? (Should handle gracefully, not crash)
- How does system handle services with zero health expressions configured? (Should return default/error state)
- What happens when boolean expressions contain invalid metric names? (Should return error with metric name)
- How does system handle extremely large time ranges (months of data)? (Should limit or paginate)
- What happens when configuration contains circular variable references? (Should detect and error)
- How does system behave when Graphite is completely unreachable? (Should timeout and return error)
- What happens when multiple environments have overlapping metric names? (Should namespace correctly)
- How does expression evaluation handle division by zero or math errors? (Should catch and return error)
- What happens when services list contains special characters or spaces? (Should sanitize or validate)
- How does system handle partial Graphite responses (some metrics succeed, others fail)? (Should process available data)

## Requirements *(mandatory)*

### Functional Requirements

#### Test Coverage Requirements

- **FR-001**: Test suite MUST achieve minimum 95% code coverage for all core business logic functions (get_metric_flag_state, get_service_health, AppState::process_config, handler_render)
- **FR-002**: Test suite MUST cover all three comparison operators (Lt, Gt, Eq) with boundary conditions and edge cases for metric flag evaluation
- **FR-003**: Test suite MUST verify boolean expression evaluation (AND, OR operators) with all combinations of true/false metric states
- **FR-004**: Test suite MUST validate weighted health scoring with multiple expressions at different priority levels
- **FR-005**: Test suite MUST verify configuration template variable substitution ($environment, $service) for all configured environments and services

#### API Testing Requirements

- **FR-006**: Test suite MUST include integration tests for all REST API endpoints: /api/v1/health, /api/v1/info, /render, /metrics/find, /functions, /tags/autoComplete/tags
- **FR-007**: API tests MUST verify correct HTTP status codes (200, 400, 404, 500) for valid and invalid requests
- **FR-008**: API tests MUST validate response JSON structure matches expected schema for each endpoint
- **FR-009**: API tests MUST verify error messages are clear and actionable when requests fail

#### Graphite Integration Testing Requirements

- **FR-010**: Test suite MUST mock Graphite HTTP responses using an HTTP mock server (e.g., wiremock-rs or httptest) for all query scenarios (valid data, empty data, errors, timeouts)
- **FR-011**: Tests MUST verify correct parsing of Graphite JSON response format with datapoints arrays
- **FR-012**: Tests MUST verify metric discovery (find_metrics) correctly handles hierarchical metric paths with wildcards
- **FR-013**: Tests MUST verify query building produces valid Graphite query syntax with correct time ranges and parameters

#### Configuration Testing Requirements

- **FR-014**: Test suite MUST verify configuration loading from multiple sources (file, conf.d directory, environment variables) with correct precedence
- **FR-015**: Tests MUST verify YAML parsing handles valid and invalid syntax with appropriate error messages
- **FR-016**: Tests MUST verify configuration validation catches missing required fields and returns all errors at once
- **FR-017**: Tests MUST verify metric template expansion creates correct mappings for all environment and service combinations

#### Regression Protection Requirements

- **FR-018**: Test suite MUST execute in under 2 minutes to support rapid development workflow. Tests MUST run in parallel using Rust's default test runner (cargo test) with proper isolation (unique mock server ports, isolated test data) to achieve performance goals.
- **FR-019**: Tests MUST fail immediately with clear error messages when business logic behavior changes. Functional tests, API tests, and complex integration tests MUST use custom assertion helpers that include business context (e.g., service name, metric states, expected behavior). Unit tests MAY use standard Rust assert macros for simplicity.
- **FR-020**: Test suite MUST be runnable in CI/CD pipeline with standard Rust test tools (cargo test)
- **FR-021**: Tests MUST be maintainable with clear naming, documentation, and modular structure. Test fixtures MUST be organized per module with shared fixtures for related tests to ensure consistency and reduce duplication.

#### Error Handling Testing Requirements

- **FR-022**: Test suite MUST verify all error paths return appropriate error types without panicking
- **FR-023**: Tests MUST verify system handles null, NaN, and missing metric values gracefully
- **FR-024**: Tests MUST verify system handles network failures (timeouts, connection refused) with retries or clear errors
- **FR-025**: Tests MUST verify system handles malformed JSON responses from Graphite without crashing

### Key Entities

- **Test Case**: Represents a single automated test with setup, execution, and assertion phases. Contains test name, description, mock data fixtures, expected outcomes, and cleanup logic.

- **Mock Graphite Server**: HTTP mock server (e.g., wiremock-rs or httptest) that simulates Graphite TSDB responses for integration testing. Runs an actual HTTP server on localhost during tests, provides configurable responses for different query patterns, supports both valid data and error scenarios. Tests the complete HTTP client stack including connection handling, timeouts, and error codes.

- **Test Fixture**: Reusable test data including sample configurations, metric values, boolean expressions, and expected health scores. Organized by scenario (happy path, edge cases, errors). Each test module (metric_evaluation_tests, health_aggregation_tests, etc.) maintains its own fixtures submodule with common test data shared across related tests, ensuring consistency while keeping fixtures close to where they're used.

- **Coverage Report**: Generated output showing code coverage percentage for each module and function. Generated using cargo-tarpaulin with support for HTML, lcov, and JSON formats. Used to verify 95% threshold is met and identify untested code paths. Integrated into CI/CD pipeline for automated coverage tracking.

- **Test Configuration**: YAML configuration files specifically designed for testing, including minimal valid config, maximal config with all features, and invalid configs for error testing.

## Success Criteria *(mandatory)*

### Measurable Outcomes

#### Coverage Metrics

- **SC-001**: Test suite achieves minimum 95% code coverage for core business functions (get_metric_flag_state, get_service_health, AppState::process_config, handler_render, get_graphite_data) as measured by cargo-tarpaulin
- **SC-002**: Test suite includes minimum 50 test cases covering all priority areas (20+ for metric evaluation, 15+ for health aggregation, 10+ for API endpoints, 5+ for configuration)
- **SC-003**: All 25 functional requirements (FR-001 through FR-025) have at least one passing test that validates the requirement

#### Quality Metrics

- **SC-004**: Test suite completes full execution in under 2 minutes on standard development hardware
- **SC-005**: Tests detect 100% of intentional breaking changes to business logic (deliberate changes to comparison operators, expression evaluation, weight calculations)
- **SC-006**: Zero false positives - tests only fail when actual business logic changes, not due to test flakiness or timing issues. Tests MUST be designed for parallel execution with proper isolation to prevent race conditions.

#### Refactoring Confidence

- **SC-007**: Developers can refactor code structure (rename functions, split modules, reorganize files) without any test failures as long as behavior is preserved
- **SC-008**: New developers can run test suite immediately after cloning repository with single command (cargo test) and see all tests pass
- **SC-009**: Test failures provide clear error messages identifying which business function broke and what the expected vs actual behavior was. Functional and integration test failures include business context (service names, metric states, scenarios) beyond simple value comparisons.

#### Documentation Value

- **SC-010**: Test cases serve as executable documentation - new team members can understand business logic by reading test scenarios
- **SC-011**: Each test case includes descriptive name and comments explaining the business scenario being tested
- **SC-012**: Test coverage report clearly identifies any untested code paths requiring additional tests. Coverage reports generated using cargo-tarpaulin in HTML and lcov formats for developer and CI integration.

#### CI/CD Integration

- **SC-013**: Test suite runs automatically on every pull request and blocks merge if any test fails
- **SC-014**: Test results are reported in CI/CD pipeline within 3 minutes of commit
- **SC-015**: Coverage reports are generated automatically using cargo-tarpaulin and show trend over time (no coverage decrease allowed)
