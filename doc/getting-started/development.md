# Development Workflow

This guide covers the complete development workflow for contributing to the metrics-processor project, from local setup through code submission.

## Prerequisites

Ensure you have the following tools installed:

```bash
# Rust toolchain (stable)
rustup update stable

# Code quality tools
rustup component add clippy rustfmt

# Pre-commit hooks (optional but recommended)
pip install pre-commit
pre-commit install

# Documentation builder (optional)
cargo install mdbook
```

## Building the Project

### Development Build

```bash
# Fast compilation with debug symbols
cargo build

# Build specific binary
cargo build --bin cloudmon-metrics-convertor
cargo build --bin cloudmon-metrics-reporter
```

### Release Build

```bash
# Optimized build for deployment
cargo build --release
```

### Build Script

The project includes a `build.rs` that generates JSON schemas for configuration validation. This runs automatically during `cargo build`.

## Running Locally

### Starting the Convertor Server

```bash
# Default configuration
RUST_LOG=info cargo run --bin cloudmon-metrics-convertor -- --config config.yaml

# With debug logging
RUST_LOG=debug cargo run --bin cloudmon-metrics-convertor -- --config config.yaml

# Specify custom port
RUST_LOG=info cargo run --bin cloudmon-metrics-convertor -- --config config.yaml --port 8080
```

### Starting the Reporter

```bash
RUST_LOG=info cargo run --bin cloudmon-metrics-reporter -- --config config.yaml
```

## Testing

### Running All Tests

```bash
# Run all tests
cargo test

# Run with output displayed
cargo test -- --nocapture

# Run specific test
cargo test validate_yaml_examples

# Run tests in specific module
cargo test config::
```

### Test Categories

#### Unit Tests

Unit tests live within implementation files as `#[cfg(test)]` modules:

```bash
# Run only unit tests (fast)
cargo test --lib
```

#### Integration Tests

Integration tests are in the `tests/` directory:

```bash
# Run integration tests
cargo test --test '*'

# Run specific integration test file
cargo test --test documentation_validation
```

#### Documentation Tests

Rustdoc examples are compiled and run as tests:

```bash
# Run documentation tests
cargo test --doc
```

### Test Coverage Targets

Per the project constitution:
- **Core business logic**: Minimum 95% coverage
- **API endpoints**: Integration test coverage required
- **Configuration parsing**: Validation test coverage required

### Writing Tests

Follow these conventions:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_does_expected_behavior() {
        // Arrange
        let input = setup_test_data();

        // Act
        let result = function_under_test(input);

        // Assert
        assert_eq!(result, expected_value);
    }

    #[tokio::test]
    async fn test_async_function() {
        // For async tests, use tokio::test macro
        let result = async_function().await;
        assert!(result.is_ok());
    }
}
```

### Mocking External Services

Use `mockito` for external service mocking:

```rust
use mockito::{mock, server_url};

#[tokio::test]
async fn test_graphite_query() {
    let _m = mock("GET", "/render")
        .with_status(200)
        .with_body(r#"[{"target": "test", "datapoints": [[1.0, 1234567890]]}]"#)
        .create();

    let client = GraphiteClient::new(&server_url());
    let result = client.query("test.metric").await;
    assert!(result.is_ok());
}
```

## Debugging

### Logging Configuration

The project uses the `tracing` crate with `tracing-subscriber`. Control log verbosity via `RUST_LOG`:

```bash
# Error only
RUST_LOG=error cargo run --bin cloudmon-metrics-convertor

# Info level (recommended for development)
RUST_LOG=info cargo run --bin cloudmon-metrics-convertor

# Debug level (verbose)
RUST_LOG=debug cargo run --bin cloudmon-metrics-convertor

# Trace level (very verbose)
RUST_LOG=trace cargo run --bin cloudmon-metrics-convertor

# Module-specific logging
RUST_LOG=cloudmon_metrics::graphite=debug,info cargo run --bin cloudmon-metrics-convertor
```

### Watch Mode

Use `cargo-watch` for automatic recompilation during development:

```bash
# Install cargo-watch
cargo install cargo-watch

# Rebuild on file changes
cargo watch -x build

# Run tests on file changes
cargo watch -x test

# Run specific command on changes
cargo watch -x 'run --bin cloudmon-metrics-convertor -- --config config.yaml'

# Clear screen between runs
cargo watch -c -x test
```

### IDE Debugging

For VS Code, add to `.vscode/launch.json`:

```json
{
    "version": "0.2.0",
    "configurations": [
        {
            "type": "lldb",
            "request": "launch",
            "name": "Debug Convertor",
            "cargo": {
                "args": ["build", "--bin=cloudmon-metrics-convertor"],
                "filter": {
                    "name": "cloudmon-metrics-convertor",
                    "kind": "bin"
                }
            },
            "args": ["--config", "config.yaml"],
            "cwd": "${workspaceFolder}",
            "env": {
                "RUST_LOG": "debug"
            }
        }
    ]
}
```

### Debug Assertions

Use debug assertions for development-time checks:

```rust
// Only runs in debug builds
debug_assert!(value > 0, "Value must be positive");

// Conditional compilation for debug code
#[cfg(debug_assertions)]
fn expensive_validation(data: &Data) {
    // This function is stripped in release builds
}
```

## Code Quality Tools

### Formatting with rustfmt

```bash
# Check formatting (CI check)
cargo fmt --check

# Apply formatting
cargo fmt

# Format specific file
cargo fmt -- src/config.rs
```

### Linting with Clippy

```bash
# Run clippy lints
cargo clippy

# Run clippy with all targets
cargo clippy --all-targets

# Treat warnings as errors (CI mode)
cargo clippy -- -D warnings

# Allow specific lint temporarily
cargo clippy -- -A clippy::too_many_arguments
```

Common Clippy fixes:

| Lint | Solution |
|------|----------|
| `needless_borrow` | Remove unnecessary `&` |
| `clone_on_copy` | Use value directly instead of `.clone()` |
| `map_unwrap_or` | Use `.map_or()` or `.map_or_else()` |
| `single_match` | Convert to `if let` |

### Documentation with rustdoc

```bash
# Build documentation
cargo doc

# Build and open in browser
cargo doc --open

# Include private items
cargo doc --document-private-items

# Check for broken documentation links
cargo doc --no-deps
```

Write documentation following Rust conventions:

```rust
/// Brief one-line summary.
///
/// More detailed explanation of what this function does,
/// when to use it, and any important caveats.
///
/// # Arguments
///
/// * `config` - The configuration object to validate
///
/// # Returns
///
/// Returns `Ok(())` if valid, or an error describing the issue.
///
/// # Errors
///
/// This function will return an error if:
/// - The configuration is missing required fields
/// - Field values are out of valid ranges
///
/// # Examples
///
/// ```
/// use cloudmon_metrics::config::Config;
///
/// let config = Config::load("config.yaml")?;
/// config.validate()?;
/// ```
pub fn validate(config: &Config) -> Result<(), ConfigError> {
    // implementation
}
```

### Pre-commit Hooks

The project uses pre-commit hooks defined in `.pre-commit-config.yaml`:

```bash
# Install hooks
pre-commit install

# Run all hooks manually
pre-commit run --all-files

# Run specific hook
pre-commit run fmt --all-files
pre-commit run cargo-check --all-files
```

Hooks run automatically on `git commit`:
- `cargo fmt` - Code formatting
- `cargo check` - Compilation check
- YAML validation
- Trailing whitespace removal

## Git Workflow

### Branch Naming

Follow this convention:

```
feature/<issue-id>-<description>   # New features
fix/<issue-id>-<description>       # Bug fixes
docs/<description>                  # Documentation only
refactor/<description>              # Code refactoring
```

Examples:
```bash
git checkout -b feature/42-add-prometheus-backend
git checkout -b fix/123-query-timeout-handling
git checkout -b docs/improve-config-examples
```

### Commit Messages

Write clear, descriptive commit messages:

```
<type>: <short summary>

<detailed description if needed>

Fixes #<issue-number>
```

Types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`

Examples:
```
feat: add Prometheus TSDB backend support

Implements the TSDB trait for Prometheus, enabling metric queries
via PromQL. Supports both instant and range queries.

Fixes #42
```

```
fix: handle timeout errors in Graphite queries

Previously, timeout errors would panic. Now they return a proper
QueryError::Timeout variant that callers can handle gracefully.

Fixes #123
```

### Pull Request Process

1. **Create feature branch** from `main`:
   ```bash
   git checkout main
   git pull origin main
   git checkout -b feature/my-feature
   ```

2. **Make changes** following code quality standards

3. **Run local checks** before pushing:
   ```bash
   cargo fmt --check
   cargo clippy -- -D warnings
   cargo test
   ```

4. **Push and create PR**:
   ```bash
   git push -u origin feature/my-feature
   ```

5. **PR must pass CI gates**:
   - Compilation (`cargo build`)
   - Linting (`cargo clippy`)
   - Tests (`cargo test`)
   - Docker build validation

6. **Address review feedback** and verify Constitution compliance

7. **Merge** after approval from at least one maintainer

## Common Development Tasks

### Adding a New Configuration Field

1. Add field to struct in `src/types.rs`:
   ```rust
   #[derive(Debug, Deserialize, Serialize, JsonSchema)]
   pub struct Config {
       // existing fields...
       #[serde(default)]
       pub new_field: Option<String>,
   }
   ```

2. Update `src/config.rs` if validation is needed

3. Rebuild to regenerate schema:
   ```bash
   cargo build
   ```

4. Update documentation in `doc/configuration/`

5. Add tests for the new field

### Adding a New API Endpoint

1. Add handler in `src/api/v1.rs`:
   ```rust
   pub async fn new_endpoint(
       State(state): State<AppState>,
       Query(params): Query<NewParams>,
   ) -> impl IntoResponse {
       // implementation
   }
   ```

2. Register route in router setup

3. Update `openapi-schema.yaml`

4. Add integration tests

5. Update API documentation in `doc/api/`

### Adding a New TSDB Backend

1. Create new module (e.g., `src/prometheus.rs`)

2. Implement the same interface pattern as `graphite.rs`

3. Add configuration support in `src/config.rs`

4. Add integration tests with mocked responses

5. Document in `doc/integration/`

## Troubleshooting

### Build Errors

**Error: "cannot find crate"**
```bash
# Clean and rebuild
cargo clean
cargo build
```

**Error: "linker not found"**
```bash
# macOS: Install Xcode command line tools
xcode-select --install

# Linux: Install build essentials
sudo apt-get install build-essential
```

**Error: "openssl not found"**
```bash
# macOS
brew install openssl
export OPENSSL_DIR=$(brew --prefix openssl)

# Linux
sudo apt-get install libssl-dev pkg-config
```

### Test Failures

**Tests fail with "connection refused"**
- Ensure mockito server is properly configured
- Check for port conflicts

**Async tests hang**
- Ensure using `#[tokio::test]` macro
- Check for deadlocks in async code

**Documentation tests fail**
```bash
# Run with verbose output to see which example failed
cargo test --doc -- --nocapture
```

### Runtime Issues

**"Configuration parsing failed"**
- Validate YAML syntax: `cargo run --bin cloudmon-metrics-convertor -- --config config.yaml 2>&1`
- Check against JSON schema in `doc/schemas/config-schema.json`

**"TSDB query timeout"**
- Check network connectivity to TSDB
- Verify TSDB URL in configuration
- Increase timeout values if needed

**"High memory usage"**
- Check for large response payloads from TSDB
- Enable debug logging to trace memory allocation
- Consider query result pagination

### Performance Debugging

```bash
# Build with debug symbols for profiling
cargo build --release

# Use perf (Linux)
perf record target/release/cloudmon-metrics-convertor
perf report

# Use Instruments (macOS)
instruments -t "Time Profiler" target/release/cloudmon-metrics-convertor
```

## Development Environment Setup

### Recommended VS Code Extensions

- **rust-analyzer**: Language server for Rust
- **CodeLLDB**: Debugger for Rust
- **Even Better TOML**: TOML syntax highlighting
- **YAML**: YAML syntax highlighting and validation

### Environment Variables

```bash
# Development settings
export RUST_LOG=info
export RUST_BACKTRACE=1  # Full backtraces on panic

# For integration testing
export TEST_GRAPHITE_URL=http://localhost:8080
```

## Next Steps

- **New to the codebase?** Start with [Project Structure](project-structure.md)
- **Ready to contribute?** Check open issues and pick one labeled `good-first-issue`
- **Questions?** Refer to the [Constitution](../../.specify/memory/constitution.md) for code standards
