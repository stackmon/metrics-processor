# Implementation Plan: Comprehensive Functional Test Suite

**Feature Branch**: `002-functional-test-suite`  
**Created**: 2025-01-24  
**Status**: Ready for Implementation

---

## 1. Overview

This plan implements a comprehensive functional test suite achieving 95%+ coverage of core business functions. The approach prioritizes **bottom-up testing**: starting with pure unit tests for core logic, then building up to integration tests with mocked HTTP dependencies, and finally full API endpoint tests.

**Key Implementation Strategy:**
- Use existing `mockito` dependency for HTTP mocking (already in dev-dependencies)
- Organize tests by module with shared fixtures per test category
- Leverage Rust's built-in parallel test execution with proper isolation
- Add `cargo-tarpaulin` for coverage measurement in CI

**Current Test Baseline:**
- `config.rs`: 3 tests (config parsing, env merge, conf.d merge)
- `types.rs`: 1 test (AppState processing)
- `graphite.rs`: 3 tests (query building, HTTP mocking, find metrics)
- `common.rs`: 0 tests ❌ (core business logic - **highest priority**)
- `api/v1.rs`: 0 tests ❌ (API handlers)

---

## 2. Design Decisions

### 2.1 Test Framework Architecture

**Decision**: Use Rust's built-in test framework with inline module tests + integration tests in `tests/`

**Rationale**:
- Inline `#[cfg(test)]` modules keep tests close to implementation
- Integration tests in `tests/` directory for cross-module scenarios
- No additional test framework dependencies needed
- Follows existing codebase patterns (see `config.rs`, `graphite.rs`)

**Structure**:
```
src/
├── common.rs          # + #[cfg(test)] mod test { ... }
├── types.rs           # + expand existing test module
├── graphite.rs        # + expand existing test module
├── config.rs          # existing tests - add validation tests
├── api/
│   └── v1.rs          # + #[cfg(test)] mod test { ... }
tests/
├── fixtures/          # Shared test fixtures
│   ├── mod.rs
│   ├── configs.rs     # YAML config fixtures
│   ├── graphite_responses.rs  # Mock Graphite data
│   └── helpers.rs     # Common test utilities
├── integration_health.rs      # Health aggregation E2E
├── integration_api.rs         # Full API endpoint tests
└── documentation_validation.rs  # (existing)
```

### 2.2 Mock Server Setup

**Decision**: Use `mockito` (already in Cargo.toml dev-dependencies)

**Rationale**:
- Already integrated and proven working (see `graphite.rs:test_get_graphite_data`)
- Provides request matching, response mocking, and expectation verification
- Lightweight and thread-safe with `mockito::Server::new()` per test
- No need to add `wiremock-rs` or `httptest` - avoid unnecessary dependencies

**Mock Patterns**:
```rust
// Per-test server isolation (already established pattern)
let mut server = mockito::Server::new();
let mock = server
    .mock("GET", "/render")
    .match_query(Matcher::AllOf(vec![...]))
    .with_body(json!([...]).to_string())
    .create();
```

### 2.3 Fixture Organization Strategy

**Decision**: Shared fixtures per module in `tests/fixtures/`

**Rationale**:
- Centralized test data reduces duplication (spec FR-021)
- Easy to maintain consistent test scenarios
- Can be reused across unit and integration tests
- Follows DRY principle while keeping fixtures discoverable

**Fixture Categories**:

| Module | Fixture Type | Contents |
|--------|--------------|----------|
| `configs.rs` | YAML strings | Valid configs, minimal configs, invalid configs, edge cases |
| `graphite_responses.rs` | JSON data | Valid datapoints, empty arrays, null values, errors |
| `helpers.rs` | Test utilities | `create_test_state()`, `mock_graphite_response()`, custom assertions |

### 2.4 Coverage Measurement Approach

**Decision**: Use `cargo-tarpaulin` with CI integration

**Rationale**:
- Rust-native, accurate line coverage (spec clarification)
- Supports HTML and lcov output formats
- Can enforce minimum threshold in CI
- Widely adopted in Rust ecosystem

**CI Configuration** (for `zuul.yaml` or GitHub Actions):
```yaml
coverage:
  script:
    - cargo install cargo-tarpaulin
    - cargo tarpaulin --out Html --out Lcov --fail-under 95
  artifacts:
    - tarpaulin-report.html
```

---

## 3. Architecture

### 3.1 Test Categories

```
┌─────────────────────────────────────────────────────────────────┐
│                    Test Pyramid                                  │
├─────────────────────────────────────────────────────────────────┤
│  ┌───────────────────────────────────────────────────────────┐  │
│  │         Integration Tests (tests/*.rs)                     │  │
│  │   • Full API endpoint tests with mock Graphite            │  │
│  │   • Cross-module health aggregation flows                 │  │
│  └───────────────────────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │         Unit Tests (src/**/mod test)                      │  │
│  │   • get_metric_flag_state - all operators & edge cases   │  │
│  │   • get_service_health - expression evaluation           │  │
│  │   • AppState::process_config - template expansion        │  │
│  │   • Config validation & error cases                      │  │
│  │   • Graphite query building & response parsing           │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

### 3.2 Test Isolation Strategy

For parallel execution safety (spec FR-018, SC-006):

| Isolation Need | Solution |
|----------------|----------|
| Mock server ports | `mockito::Server::new()` auto-assigns unique ports |
| Shared state | Each test creates own `AppState` instance |
| Environment variables | Use `temp_env` crate or test-specific prefixes |
| File system | Use `tempfile` crate (already in dev-deps) |

### 3.3 Custom Assertion Helpers

For business context in failures (spec FR-019, SC-009):

```rust
// tests/fixtures/helpers.rs
pub fn assert_metric_flag(
    value: Option<f32>,
    metric: &FlagMetric,
    expected: bool,
    context: &str,
) {
    let actual = get_metric_flag_state(&value, metric);
    assert_eq!(
        actual, expected,
        "Metric flag evaluation failed for {}: value={:?}, op={:?}, threshold={}, expected={}, got={}",
        context, value, metric.op, metric.threshold, expected, actual
    );
}

pub fn assert_health_score(
    service: &str,
    environment: &str,
    expected_score: u8,
    actual_score: u8,
) {
    assert_eq!(
        actual_score, expected_score,
        "Health score mismatch for service '{}' in '{}': expected {}, got {}",
        service, environment, expected_score, actual_score
    );
}
```

---

## 4. Implementation Phases

### Phase 1: Test Infrastructure Setup
- [ ] **1.1** Create `tests/fixtures/mod.rs` with module structure
- [ ] **1.2** Create `tests/fixtures/configs.rs` with standard test configurations
- [ ] **1.3** Create `tests/fixtures/graphite_responses.rs` with mock response data
- [ ] **1.4** Create `tests/fixtures/helpers.rs` with custom assertions and utilities
- [ ] **1.5** Add `cargo-tarpaulin` configuration to CI pipeline

### Phase 2: Core Function Unit Tests (P1 - Highest Priority)
- [ ] **2.1** `get_metric_flag_state` tests in `src/common.rs`:
  - [ ] 2.1.1 Lt operator: value < threshold returns true
  - [ ] 2.1.2 Lt operator: value >= threshold returns false
  - [ ] 2.1.3 Gt operator: value > threshold returns true
  - [ ] 2.1.4 Gt operator: value <= threshold returns false
  - [ ] 2.1.5 Eq operator: value == threshold returns true
  - [ ] 2.1.6 Eq operator: value != threshold returns false
  - [ ] 2.1.7 None value always returns false
  - [ ] 2.1.8 Boundary conditions (threshold ± 0.001)
  - [ ] 2.1.9 Negative values
  - [ ] 2.1.10 Zero threshold
- [ ] **2.2** `AppState::process_config` tests in `src/types.rs`:
  - [ ] 2.2.1 Template variable substitution ($environment, $service)
  - [ ] 2.2.2 Multiple environments expansion
  - [ ] 2.2.3 Per-environment threshold override
  - [ ] 2.2.4 Dash-to-underscore conversion in expressions
  - [ ] 2.2.5 Service set population
  - [ ] 2.2.6 Health metrics expression copying

### Phase 3: Integration Tests with Mocked Graphite (P1)
- [ ] **3.1** `get_service_health` tests in `src/common.rs`:
  - [ ] 3.1.1 Single metric OR expression evaluates correctly
  - [ ] 3.1.2 Multiple metrics AND expression evaluates correctly
  - [ ] 3.1.3 Weighted expressions return highest matching weight
  - [ ] 3.1.4 All false expressions return weight 0
  - [ ] 3.1.5 Unknown service returns ServiceNotSupported error
  - [ ] 3.1.6 Unknown environment returns EnvNotSupported error
  - [ ] 3.1.7 Multiple datapoints across time series
- [ ] **3.2** Create `tests/integration_health.rs` for end-to-end health flows:
  - [ ] 3.2.1 Full health calculation with mocked Graphite
  - [ ] 3.2.2 Complex weighted expression scenarios
  - [ ] 3.2.3 Edge cases: empty datapoints, partial data

### Phase 4: API Endpoint Tests (P2)
- [ ] **4.1** Add tests to `src/api/v1.rs`:
  - [ ] 4.1.1 `/api/v1/` root endpoint returns name
  - [ ] 4.1.2 `/api/v1/info` returns API info
  - [ ] 4.1.3 `/api/v1/health` with valid service returns 200 + JSON
  - [ ] 4.1.4 `/api/v1/health` with unknown service returns 409
  - [ ] 4.1.5 `/api/v1/health` with missing params returns 400
- [ ] **4.2** Expand Graphite route tests in `src/graphite.rs`:
  - [ ] 4.2.1 `/render` with flag target returns boolean datapoints
  - [ ] 4.2.2 `/render` with health target returns health scores
  - [ ] 4.2.3 `/render` with invalid target returns empty array
  - [ ] 4.2.4 `/metrics/find` all levels (*, flag.*, flag.env.*, etc.)
  - [ ] 4.2.5 `/functions` returns empty object
  - [ ] 4.2.6 `/tags/autoComplete/tags` returns empty array
- [ ] **4.3** Create `tests/integration_api.rs` for full API tests:
  - [ ] 4.3.1 Health endpoint with mocked Graphite responses
  - [ ] 4.3.2 Render endpoint with various targets
  - [ ] 4.3.3 Error response format validation

### Phase 5: Configuration & Error Path Tests (P2-P3)
- [ ] **5.1** Expand config tests in `src/config.rs`:
  - [ ] 5.1.1 Invalid YAML syntax returns parse error
  - [ ] 5.1.2 Missing required fields error
  - [ ] 5.1.3 Default values applied correctly
  - [ ] 5.1.4 `get_socket_addr()` produces valid address
- [ ] **5.2** Graphite client error handling in `src/graphite.rs`:
  - [ ] 5.2.1 HTTP 4xx returns GraphiteError
  - [ ] 5.2.2 HTTP 5xx returns GraphiteError
  - [ ] 5.2.3 Malformed JSON response handling
  - [ ] 5.2.4 Connection timeout handling
  - [ ] 5.2.5 Empty response handling
- [ ] **5.3** Expression evaluation error tests:
  - [ ] 5.3.1 Invalid expression syntax returns ExpressionError
  - [ ] 5.3.2 Missing metric in context handled

### Phase 6: Coverage Validation & CI Integration
- [ ] **6.1** Run `cargo tarpaulin` and verify 95% coverage target
- [ ] **6.2** Identify and fill any coverage gaps
- [ ] **6.3** Add coverage enforcement to CI (`--fail-under 95`)
- [ ] **6.4** Generate HTML coverage report for documentation
- [ ] **6.5** Verify all tests pass in under 2 minutes
- [ ] **6.6** Run intentional breakage tests to verify regression detection

---

## 5. Dependencies

### Task Dependency Graph

```
Phase 1 (Infrastructure)
    │
    ├──► Phase 2 (Unit Tests) ──┐
    │                           │
    └──► Phase 3 (Integration)──┼──► Phase 6 (Coverage)
              │                 │
              └──► Phase 4 (API)┘
                      │
                      └──► Phase 5 (Errors)
```

### Critical Path

1. **Phase 1.1-1.4** (fixtures) → Blocks all test implementation
2. **Phase 2.1** (get_metric_flag_state) → Core logic, highest ROI
3. **Phase 3.1** (get_service_health) → Most complex business function
4. **Phase 6.1** (coverage check) → May reveal additional gaps

### External Dependencies

| Dependency | Version | Purpose | Status |
|------------|---------|---------|--------|
| `mockito` | ~1.0 | HTTP mocking | ✅ Already in Cargo.toml |
| `tempfile` | ~3.5 | Temp file/dir creation | ✅ Already in Cargo.toml |
| `tokio-test` | * | Async test utilities | ✅ Already in Cargo.toml |
| `hyper` | 0.14 | HTTP test client | ✅ Already in Cargo.toml |
| `cargo-tarpaulin` | latest | Coverage tool | ⚠️ Install required |

---

## 6. Risk Mitigation

### Risk 1: Test Flakiness from Parallel Execution

**Risk**: Tests sharing mock servers or state cause intermittent failures.

**Mitigation**:
- Each test creates its own `mockito::Server::new()` (auto-assigns port)
- Each test creates its own `AppState` instance
- Use `tokio::test` for async isolation
- Avoid global mutable state

**Detection**: Run `cargo test -- --test-threads=1` vs default; results should match.

### Risk 2: Coverage Target Not Achievable

**Risk**: 95% coverage is unrealistic for error paths or edge cases.

**Mitigation**:
- Focus coverage on core functions listed in spec (FR-001)
- Accept lower coverage for generated code, error Display impls
- Use `#[cfg(not(tarpaulin_include))]` for intentionally uncovered code
- Document any exclusions

**Fallback**: Negotiate with stakeholders if <95% is justified.

### Risk 3: Mock Graphite Behavior Diverges from Real

**Risk**: Mocked responses don't match real Graphite behavior.

**Mitigation**:
- Use real Graphite API documentation for response formats
- Record real responses as fixtures where possible
- Include malformed/error responses based on real error scenarios
- Integration test with real Graphite in CI (optional, out of scope)

### Risk 4: Test Suite Exceeds 2-Minute Target

**Risk**: Full test suite takes too long, reducing developer adoption.

**Mitigation**:
- Profile test execution: `cargo test -- --nocapture 2>&1 | ts`
- Identify slow tests (usually mock server setup)
- Use `#[ignore]` for optional slow tests
- Consider test parallelization tuning

**Target Breakdown**:
- Unit tests: <30 seconds (no I/O)
- Integration tests: <60 seconds (mock HTTP)
- API tests: <30 seconds (in-process server)

### Risk 5: Test Maintenance Burden

**Risk**: Tests become brittle and hard to maintain over time.

**Mitigation**:
- Shared fixtures reduce duplication
- Custom assertion helpers provide clear failure messages
- Tests focus on behavior, not implementation details
- Documentation in test names and comments

---

## 7. Test Count Targets (SC-002)

| Category | Target | Priority |
|----------|--------|----------|
| Metric flag evaluation | 20+ tests | P1 |
| Health aggregation | 15+ tests | P1 |
| API endpoints | 10+ tests | P2 |
| Configuration | 5+ tests | P2 |
| Error handling | 10+ tests | P3 |
| **Total** | **60+ tests** | |

---

## 8. Success Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Code coverage | ≥95% | `cargo tarpaulin --fail-under 95` |
| Test count | ≥50 | `cargo test -- --list \| wc -l` |
| Execution time | <2 min | `time cargo test` |
| Parallel safety | 0 flaky | Run 10x with `--test-threads=8` |
| Regression detection | 100% | Intentional break tests |

---

## Appendix A: Sample Test Fixtures

### A.1 Minimal Valid Config

```rust
// tests/fixtures/configs.rs
pub const MINIMAL_CONFIG: &str = r#"
datasource:
  url: 'http://localhost:8080'
server:
  port: 3000
environments:
  - name: prod
flag_metrics: []
health_metrics: {}
"#;
```

### A.2 Mock Graphite Response

```rust
// tests/fixtures/graphite_responses.rs
pub fn valid_datapoints(target: &str) -> String {
    serde_json::json!([{
        "target": target,
        "datapoints": [
            [85.0, 1700000000],
            [90.0, 1700000060],
            [95.0, 1700000120]
        ]
    }]).to_string()
}

pub fn empty_datapoints(target: &str) -> String {
    serde_json::json!([{
        "target": target,
        "datapoints": []
    }]).to_string()
}
```

### A.3 Test Helper Functions

```rust
// tests/fixtures/helpers.rs
use cloudmon_metrics::{config::Config, types::AppState};

pub fn create_test_state(config_yaml: &str) -> AppState {
    let config = Config::from_config_str(config_yaml);
    let mut state = AppState::new(config);
    state.process_config();
    state
}

pub fn create_test_state_with_mock_url(config_yaml: &str, mock_url: &str) -> AppState {
    // Replace datasource URL with mock server URL
    let modified = config_yaml.replace("http://localhost:8080", mock_url);
    create_test_state(&modified)
}
```

---

## Appendix B: Commands Reference

```bash
# Run all tests
cargo test

# Run with verbose output
cargo test -- --nocapture

# Run specific test module
cargo test common::test

# Run tests matching pattern
cargo test metric_flag

# Check coverage (install first: cargo install cargo-tarpaulin)
cargo tarpaulin --out Html

# Coverage with threshold enforcement
cargo tarpaulin --fail-under 95

# Run tests sequentially (debugging flakiness)
cargo test -- --test-threads=1

# List all tests
cargo test -- --list
```
