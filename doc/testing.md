# Testing Guide

## Overview

This document describes the comprehensive test suite for the metrics-processor project, including test execution, coverage measurement, and regression protection.

## Test Organization

### Test Structure

```
tests/
├── fixtures/                   # Shared test fixtures and utilities
│   ├── mod.rs                 # Module declaration
│   ├── configs.rs             # YAML configuration fixtures
│   ├── graphite_responses.rs # Mock Graphite response data
│   └── helpers.rs             # Test helper functions and assertions
├── documentation_validation.rs # Documentation validation tests
└── (future integration tests)

src/
├── common.rs                  # + #[cfg(test)] mod tests { ... }
├── types.rs                   # + #[cfg(test)] mod tests { ... }
├── config.rs                  # + #[cfg(test)] mod tests { ... }
├── graphite.rs                # + #[cfg(test)] mod tests { ... }
└── api/v1.rs                  # + #[cfg(test)] mod tests { ... }
```

### Test Categories

1. **Unit Tests**: Located inline with source code using `#[cfg(test)]` modules
2. **Integration Tests**: Located in `tests/` directory for cross-module scenarios
3. **Fixtures**: Shared test data and utilities in `tests/fixtures/`

## Running Tests

### Run All Tests

```bash
cargo test
```

### Run Specific Test Module

```bash
# Run only common module tests
cargo test common::tests

# Run only config tests
cargo test config::tests

# Run only integration tests
cargo test --test integration_*
```

### Run Tests with Output

```bash
cargo test -- --nocapture
```

### Run Tests in Parallel (default)

```bash
cargo test -- --test-threads=4
```

### Run Tests Sequentially

```bash
cargo test -- --test-threads=1
```

## Test Coverage

### Measuring Coverage

This project uses `cargo-tarpaulin` for code coverage measurement.

#### Install cargo-tarpaulin

```bash
cargo install cargo-tarpaulin
```

#### Generate Coverage Report

```bash
# Generate XML report (for CI/CD)
cargo tarpaulin --out Xml --output-dir ./coverage

# Generate HTML report (for local viewing)
cargo tarpaulin --out Html --output-dir ./coverage

# Generate both
cargo tarpaulin --out Xml --out Html --output-dir ./coverage
```

#### Coverage Thresholds

The project enforces a **95% coverage threshold** for core business functions:

```bash
cargo tarpaulin --fail-under 95
```

### CI/CD Integration

Coverage is automatically measured in CI via GitHub Actions (`.github/workflows/coverage.yml`):

- Runs on every push and pull request
- Generates XML report for Codecov
- Generates HTML report as artifact
- Fails build if coverage drops below 95%

## Test Execution Time

### Target: Under 2 Minutes

The test suite is designed to execute quickly to enable rapid development feedback.

**Current Performance** (as of implementation):
- Unit tests: ~0.05 seconds
- Integration tests: (to be measured)
- Total: Well under 2-minute target

### Measuring Test Time

```bash
time cargo test
```

## Regression Protection

### Validation Approach

The test suite is designed to catch breaking changes immediately:

1. **Comprehensive Coverage**: All core business logic is tested
2. **Clear Assertions**: Tests use descriptive error messages
3. **Edge Cases**: Boundary conditions, null values, negative numbers
4. **Operator Testing**: All comparison operators (Lt, Gt, Eq) validated

### Manual Regression Validation

To verify the test suite catches breaking changes:

1. **Backup the source file**:
   ```bash
   cp src/common.rs src/common.rs.backup
   ```

2. **Introduce a breaking change** (e.g., swap Lt and Gt operators):
   ```bash
   # Edit src/common.rs and swap the operators
   CmpType::Lt => x > metric.threshold,  # Wrong!
   CmpType::Gt => x < metric.threshold,  # Wrong!
   ```

3. **Run tests** (should fail):
   ```bash
   cargo test common::tests
   ```

4. **Verify failures** with clear error messages

5. **Restore original**:
   ```bash
   mv src/common.rs.backup src/common.rs
   ```

### Expected Behavior

When breaking changes are introduced:
- ✓ Tests fail immediately
- ✓ Error messages clearly indicate the problem
- ✓ Multiple tests catch the same logical error (redundancy)
- ✓ Zero false positives

## Test Coverage by Feature

### Phase 1: Core Metric Flag Evaluation (US1)
- **Tests**: 11 unit tests
- **Coverage**: Lt, Gt, Eq operators
- **Edge Cases**: None values, boundaries, negative numbers, zero threshold
- **Status**: ✅ Complete (100% coverage)

### Phase 2: Service Health Aggregation (US2)
- **Tests**: (to be implemented)
- **Coverage**: Expression evaluation, weight calculation
- **Status**: 🚧 Pending

### Phase 3: Configuration Processing (US4)
- **Tests**: (to be implemented)
- **Coverage**: Template variables, validation, defaults
- **Status**: 🚧 Pending

### Phase 4: API Endpoints (US3)
- **Tests**: (to be implemented)
- **Coverage**: REST API handlers, error responses
- **Status**: 🚧 Pending

### Phase 5: Graphite Integration (US5)
- **Tests**: (to be implemented)
- **Coverage**: HTTP mocking, response parsing
- **Status**: 🚧 Pending

## Best Practices

### Writing Tests

1. **Use Descriptive Names**: Test names should clearly indicate what they test
   ```rust
   #[test]
   fn test_lt_operator_below_threshold() { ... }
   ```

2. **Use Custom Assertions**: Leverage helpers for better error messages
   ```rust
   use crate::fixtures::helpers::assert_metric_flag;
   assert_metric_flag(result, expected, "Lt operator with negative values");
   ```

3. **Test Edge Cases**: Always include boundary conditions
   - None/null values
   - Zero values
   - Negative values
   - Boundary values (threshold ± 0.001)

4. **Isolate Tests**: Each test should be independent
   - Use `mockito::Server::new()` for per-test isolation
   - Avoid shared mutable state
   - Clean up resources in test teardown

### Test Data Management

1. **Use Fixtures**: Centralize test data in `tests/fixtures/`
2. **Reuse Helpers**: Use helper functions for common setup
3. **Mock External Services**: Never call real external APIs in tests

## Troubleshooting

### Tests Failing Unexpectedly

1. **Check for state pollution**: Ensure tests are isolated
2. **Rebuild from scratch**: `cargo clean && cargo test`
3. **Check for race conditions**: Run with `--test-threads=1`

### Coverage Too Low

1. **Identify uncovered code**: `cargo tarpaulin --out Html`
2. **Add tests for uncovered branches**
3. **Focus on core business logic first**

### Tests Running Too Slow

1. **Profile test execution**: `cargo test -- --nocapture`
2. **Check for unnecessary sleeps or timeouts**
3. **Use mocks instead of real HTTP calls**

## Contributing

When adding new features:

1. **Write tests first** (TDD approach)
2. **Ensure tests fail** before implementation
3. **Implement feature** until tests pass
4. **Verify coverage** meets threshold
5. **Update this documentation** if adding new test categories

## References

- [Rust Testing Documentation](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [cargo-tarpaulin](https://github.com/xd009642/tarpaulin)
- [mockito](https://docs.rs/mockito/)
