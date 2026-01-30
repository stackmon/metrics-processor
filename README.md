# Metrics processor

When monitoring cloud it is not unusual to
have a variety of metrics types (latencies,
status codes, rates). Visualizing overall
state of the service based on those metrics
is not an easy task in this case. It is
desired to have something like a semaphore to
visualize overall "health" of the component
(green - up and running, yellow - there are
some issues, red - complete outage).
Depending on the used TSDB there might be no
way to do this at all.

metrics-processor is there to address 2 primary needs:

- convert series of raw metrics of different
  types into single semaphore-like metric
- inform status dashboard once certain
  component status is not healthy.


## Project Structure

- `src/` - Rust source code
- `doc/` - Documentation sources (mdbook)
- `tests/` - Integration and validation tests
- `specs/` - Feature specifications and implementation plans
- `playbooks/` - Operational playbooks

## Documentation

The project uses [mdbook](https://rust-lang.github.io/mdBook/) for documentation. Source files are in `doc/`.

### Building Documentation

```bash
# Install mdbook (if not already installed)
cargo install mdbook mdbook-mermaid

# Build documentation
mdbook build doc/

# Output will be in docs/
open docs/index.html
```

### Serving Documentation Locally

```bash
# Serve with live reload (rebuilds on changes)
mdbook serve doc/

# Open http://localhost:3000 in your browser
```

### Documentation Contents

| Section | Description |
|---------|-------------|
| [Getting Started](doc/getting-started/) | Quickstart guide, project structure, development workflow |
| [Architecture](doc/architecture/) | System overview, diagrams, data flow |
| [API Reference](doc/api/) | REST endpoints, authentication, examples |
| [Configuration](doc/configuration/) | Config schema, examples, validation |
| [Integration](doc/integration/) | TSDB interface, adding new backends |
| [Modules](doc/modules/) | Rust module documentation |
| [Guides](doc/guides/) | Troubleshooting, deployment |

| [Testing](doc/testing.md) | Testing guide, fixtures, coverage |

## Testing

### Running Tests

```bash
# Run all unit tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test module
cargo test common::tests

# Run tests in parallel (default)
cargo test -- --test-threads=4
```

### E2E Integration Tests

End-to-end tests validate the complete pipeline using real Docker containers (go-carbon + carbonapi).

#### Prerequisites
- Docker installed and running
- Ports available: 2003, 8080, 3005, 9999

#### Running E2E Tests

```bash
# Run E2E tests (Docker containers are managed automatically)
cargo test --test integration_e2e_reporter -- --ignored --nocapture
```

The E2E test validates 4 scenarios:
| Scenario | Expected Weight | Description |
|----------|-----------------|-------------|
| healthy | 0 | All metrics within thresholds |
| degraded_slow | 1 | API response time > 1200ms |
| degraded_errors | 1 | Success rate < 65% |
| outage | 2 | 100% API failures |

For details, see [Testing Guide](doc/testing.md).

### Test Coverage

```bash
# Install cargo-tarpaulin (if not already installed)
cargo install cargo-tarpaulin

# Run coverage report
cargo tarpaulin --out Html

# Open coverage report
open tarpaulin-report.html
```

### JSON Schema for Configuration

A JSON Schema for configuration validation is auto-generated during build:

```bash
# Schema is generated to doc/schemas/config-schema.json
cargo build

# Use in your IDE for YAML autocomplete and validation
```
