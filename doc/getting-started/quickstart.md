# Quickstart: Developer Onboarding

**Feature**: 001-project-documentation  
**Target Audience**: New developers joining the metrics-processor project  
**Time Estimate**: 30 minutes

## What You'll Learn

By the end of this guide, you will:
- ✅ Understand what metrics-processor does and why it exists
- ✅ Have a working local development environment
- ✅ Successfully build and run both binaries (convertor and reporter)
- ✅ Identify the major components and their responsibilities
- ✅ Know where to find key documentation sections

---

## What is CloudMon Metrics Processor?

**Problem**: Cloud monitoring produces many different metric types (latencies, status codes, rates). Visualizing overall service health from these disparate metrics is challenging.

**Solution**: metrics-processor converts raw time-series metrics into simple semaphore-like health indicators:
- 🟢 **Green (0)**: Service up and running normally
- 🟡 **Yellow (1)**: Service degraded (slow, errors)
- 🔴 **Red (2)**: Service outage

**Architecture** (high-level):

```
┌─────────────┐     ┌────────────────┐     ┌──────────────┐     ┌─────────────────┐
│   Graphite  │────▶│   Convertor    │────▶│   Reporter   │────▶│ Status Dashboard│
│   (TSDB)    │     │  (evaluates)   │     │  (notifies)  │     │   (displays)    │
└─────────────┘     └────────────────┘     └──────────────┘     └─────────────────┘
      Raw metrics         Flag metrics          Health status          Semaphore UI
```

**Two Main Components**:
1. **Convertor**: Evaluates health from raw metrics, exposes HTTP API
2. **Reporter**: Polls convertor, sends updates to status dashboard

---

## Prerequisites

Before starting, ensure you have:

- **Rust**: Version 1.75 or later (check with `rustc --version`)
  - Install: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- **Git**: For cloning the repository
- **Text Editor**: VSCode, IntelliJ, or vim with Rust support
- **Optional**: Docker (for containerized TSDB testing)

**System Requirements**:
- Linux, macOS, or WSL2 on Windows
- 4GB RAM minimum
- 500MB disk space for dependencies

---

## Step 1: Clone and Build (5 minutes)

```bash
# Clone the repository
git clone https://github.com/your-org/metrics-processor.git
cd metrics-processor

# Build all components
cargo build

# Expected output: Compiling cloudmon-metrics v0.2.0
# Should complete in 1-3 minutes depending on hardware
```

**Verify**: You should see two binaries created:
```bash
ls -lh target/debug/cloudmon-metrics-*
# cloudmon-metrics-convertor
# cloudmon-metrics-reporter
```

---

## Step 2: Understand the Project Structure (5 minutes)

### Repository Layout

```
metrics-processor/
├── src/                      # Rust source code
│   ├── lib.rs                # Library root
│   ├── api.rs                # HTTP API module
│   ├── api/
│   │   └── v1.rs             # API v1 handlers
│   ├── config.rs             # Configuration parsing
│   ├── types.rs              # Domain data structures
│   ├── graphite.rs           # Graphite TSDB integration
│   ├── common.rs             # Shared utilities
│   └── bin/
│       ├── convertor.rs      # Convertor binary entry point
│       └── reporter.rs       # Reporter binary entry point
├── doc/                      # Documentation (mdbook)
├── tests/                    # Integration tests
├── Cargo.toml                # Rust dependencies
├── openapi-schema.yaml       # API specification
└── README.md                 # Project overview
```

### Key Files to Know

| File | Purpose | When to Edit |
|------|---------|-------------|
| `src/config.rs` | Configuration parsing & validation | Adding new config fields |
| `src/types.rs` | Core data structures (Config, FlagMetric, HealthMetric) | Changing data models |
| `src/api/v1.rs` | HTTP API handlers (`/v1/health`, `/v1/maintenances`) | Adding/modifying endpoints |
| `src/graphite.rs` | Graphite TSDB client | TSDB query changes |
| `openapi-schema.yaml` | API contract | Documenting API changes |

---

## Step 3: Run Tests (3 minutes)

```bash
# Run all tests (unit + integration)
cargo test

# Expected output: test result: ok. X passed; 0 failed
```

**What's being tested**:
- Unit tests: Configuration parsing, metric evaluation logic
- Integration tests: HTTP endpoint behavior with mocked TSDB

**If tests fail**: Check the output for specific failures. Common issues:
- Missing dependencies: Run `cargo build` first
- Port conflicts: Ensure ports 3005+ are available

---

## Step 4: Run Convertor Locally (5 minutes)

### Create a Minimal Configuration

Create `config.yaml`:

```yaml
datasource:
  url: "http://localhost:8080"  # Mock TSDB (won't actually connect yet)
  type: graphite

server:
  address: "127.0.0.1"
  port: 3005

metric_templates:
  api_slow:
    query: "stats.timers.api.$environment.$service.mean"
    op: "gt"
    threshold: 500

environments:
  - name: "local-dev"

flag_metrics:
  - name: "api_slow"
    service: "test_service"
    template:
      name: "api_slow"
    environments:
      - name: "local-dev"

health_metrics:
  test_service:
    service: "test_service"
    component_name: "Test Service"
    category: "demo"
    metrics:
      - "test_service.api_slow"
    expressions:
      - expression: "test_service.api_slow"
        weight: 1
```

### Start Convertor

```bash
cargo run --bin cloudmon-metrics-convertor -- --config config.yaml

# Expected output:
# INFO cloudmon_metrics: Server starting on 127.0.0.1:3005
```

### Test the API

In another terminal:

```bash
# Query health endpoint
curl "http://localhost:3005/v1/health?from=2024-01-01T00:00:00Z&to=2024-01-01T01:00:00Z&service=test_service&environment=local-dev"

# Expected response (empty data since no real TSDB):
# {"name":"test_service","category":"demo","environment":"local-dev","metrics":[]}
```

**Success!** You've successfully run the convertor binary and made an API call.

---

## Step 5: Explore the Codebase (10 minutes)

### Understanding Flag Metrics

Flag metrics are binary indicators (raised/lowered) based on raw TSDB queries:

```rust
// src/types.rs
pub struct FlagMetric {
    pub name: String,              // "api_slow"
    pub query: String,             // TSDB query template
    pub comparison: Comparison,    // gt, lt, eq
    pub threshold: f64,            // 500.0
}
```

**Flow**:
1. Query TSDB with variable substitution: `$service` → `test_service`
2. Compare result to threshold: `mean_latency > 500ms`
3. Set flag: `true` (raised) or `false` (lowered)

### Understanding Health Metrics

Health metrics combine flag metrics using boolean expressions:

```rust
// src/types.rs
pub struct HealthMetric {
    pub service: String,
    pub expressions: Vec<Expression>,  // Boolean expressions
}

pub struct Expression {
    pub expr: String,    // "api_slow || api_error_rate_high"
    pub weight: u8,      // 0=healthy, 1=degraded, 2=outage
}
```

**Flow**:
1. Evaluate each expression using flag states
2. Take maximum weight of matching expressions
3. Return semaphore value (0, 1, or 2)

---

## Step 6: Common Development Tasks

### Adding a New Metric Template

1. Edit `config.yaml` → `metric_templates` section
2. Add new template with query, op, threshold
3. Reference in `flag_metrics` section
4. Restart convertor: `cargo run --bin cloudmon-metrics-convertor`

### Running with Auto-Reload

```bash
# Install cargo-watch
cargo install cargo-watch

# Auto-rebuild on file changes
cargo watch -x 'run --bin cloudmon-metrics-convertor -- --config config.yaml'
```

### Debugging with Logs

```bash
# Enable debug logging
RUST_LOG=debug cargo run --bin cloudmon-metrics-convertor -- --config config.yaml

# Filter to specific module
RUST_LOG=cloudmon_metrics::api=debug cargo run ...
```

### Running Clippy (Linter)

```bash
cargo clippy
# Fix all warnings before committing (Constitution requirement)
```

---

## Step 7: Key Documentation Sections

Now that you have a working environment, explore these documentation sections:

| Section | What to Learn | Location |
|---------|---------------|----------|
| **Architecture** | System design, data flow | `doc/architecture/` |
| **API Reference** | Endpoint details, authentication | `doc/api/` |
| **Configuration** | All config fields, examples | `doc/configuration/` |
| **Module Docs** | Rust module responsibilities | `doc/modules/` |
| **Troubleshooting** | Common issues, solutions | `doc/guides/troubleshooting.md` |

**Next Steps**:
- Read [Architecture Overview](../../../doc/architecture/overview.md) to understand component interactions
- Review [Configuration Schema](./contracts/config-schema.json) for full config reference
- Check [patterns.json](./contracts/patterns.json) for coding conventions

---

## Quick Reference Commands

```bash
# Build
cargo build                          # Debug build
cargo build --release                # Production build

# Test
cargo test                           # All tests
cargo test --test integration        # Integration tests only

# Run
cargo run --bin cloudmon-metrics-convertor -- --config config.yaml
cargo run --bin cloudmon-metrics-reporter -- --config config.yaml

# Lint
cargo clippy                         # Linter
cargo fmt                            # Auto-format

# Documentation
cargo doc --open                     # Generate and open rustdoc
mdbook build doc/ && mdbook serve doc/  # Build user documentation
```

---

## Common Issues

### "Could not compile `cloudmon-metrics`"

**Cause**: Missing system dependencies or outdated Rust version

**Solution**:
```bash
rustup update
cargo clean
cargo build
```

### "Address already in use" when starting convertor

**Cause**: Port 3005 is occupied

**Solution**:
```bash
# Change port in config.yaml
server:
  port: 3006

# Or kill existing process
lsof -i :3005
kill <PID>
```

### Tests failing with "connection refused"

**Cause**: Integration tests expect mock TSDB, mockito setup issue

**Solution**: Check that `mockito` dependency is present in `Cargo.toml`

---

## Success Criteria

You've successfully completed onboarding if you can:

- ✅ Build the project without errors
- ✅ Run all tests successfully
- ✅ Start convertor binary and query the API
- ✅ Explain the difference between flag metrics and health metrics
- ✅ Identify which module to edit for different types of changes

**Estimated Time**: If you completed this guide in ~30 minutes, you're ready to contribute! 🎉

---

## Getting Help

- **Code Questions**: Check `doc/modules/` for module-specific documentation
- **Configuration Issues**: See `doc/configuration/schema.md` for field reference
- **Architecture Questions**: Read `doc/architecture/overview.md`
- **Bugs**: Check `doc/guides/troubleshooting.md` first, then file an issue

---

## Next Steps

1. Pick a starter issue from the issue tracker (look for "good first issue" label)
2. Read the relevant module documentation
3. Make your changes following the constitution guidelines (`.specify/memory/constitution.md`)
4. Run tests and clippy before committing
5. Submit a PR with clear description

Welcome to the team! 🚀
