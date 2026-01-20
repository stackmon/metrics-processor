# Project Structure

This document explains the organization of the metrics-processor codebase, helping you quickly locate components and understand the module relationships.

## Repository Layout

```
metrics-processor/
├── src/                          # Rust source code
│   ├── lib.rs                    # Library entry point - public API exports
│   ├── bin/                      # Binary executables
│   │   ├── convertor.rs          # Convertor binary - metric evaluation server
│   │   └── reporter.rs           # Reporter binary - status dashboard client
│   ├── api.rs                    # HTTP API module root
│   ├── api/
│   │   └── v1.rs                 # API v1 handlers (/v1/health, /v1/maintenances)
│   ├── config.rs                 # Configuration parsing and validation
│   ├── types.rs                  # Core data structures (Config, FlagMetric, HealthMetric)
│   ├── graphite.rs               # Graphite TSDB client implementation
│   └── common.rs                 # Shared utilities and helper functions
├── doc/                          # User documentation (mdbook)
│   ├── book.toml                 # mdbook configuration
│   ├── SUMMARY.md                # Documentation table of contents
│   ├── schemas/                  # Machine-readable schemas for AI tools
│   ├── getting-started/          # Onboarding guides
│   ├── architecture/             # System design documentation
│   ├── api/                      # API reference
│   ├── configuration/            # Configuration reference
│   ├── integration/              # TSDB integration guides
│   ├── modules/                  # Rust module documentation
│   └── guides/                   # Operational guides
├── tests/                        # Integration and documentation tests
│   └── documentation_validation.rs  # Documentation example validation
├── specs/                        # Feature specifications
│   └── 001-project-documentation/   # This feature's design docs
├── Cargo.toml                    # Rust dependencies and project metadata
├── build.rs                      # Build script (generates JSON schemas)
├── openapi-schema.yaml           # OpenAPI 3.0 API specification
└── README.md                     # Project overview
```

## Source Code Organization (`src/`)

### Library Structure (`src/lib.rs`)

The library root exports public modules:
- `api`: HTTP API handlers
- `config`: Configuration parsing
- `types`: Data structures
- `graphite`: TSDB integration
- `common`: Utilities

**Usage**: When importing, use `cloudmon_metrics::api`, `cloudmon_metrics::config`, etc.

### Binaries (`src/bin/`)

Two independent executable binaries:

#### 1. **convertor.rs** - Metric Evaluation Server
- **Purpose**: Evaluate flag metrics, compute health metrics, expose HTTP API
- **Entry point**: `fn main()` initializes tokio runtime, loads config, starts axum server
- **Dependencies**: Requires `api`, `config`, `types`, `graphite` modules
- **Runs on**: Configurable address/port (default: 0.0.0.0:3000)

#### 2. **reporter.rs** - Status Dashboard Client
- **Purpose**: Poll convertor API, send updates to status dashboard
- **Entry point**: `fn main()` initializes tokio runtime, runs polling loop
- **Dependencies**: Requires `config`, `types` modules
- **Runs as**: Background daemon or scheduled job

### Core Modules

#### `api.rs` & `api/v1.rs` - HTTP API Layer
- **Responsibility**: HTTP endpoint handlers using axum framework
- **Endpoints**:
  - `GET /v1/health`: Query health metrics for services
  - `GET /v1/maintenances`: Query maintenance status
- **Key types**: `HealthQuery`, `HealthResponse`, `MaintenancesResponse`
- **Authentication**: JWT token validation for status dashboard

**When to edit**: Adding/modifying API endpoints, changing request/response formats

#### `config.rs` - Configuration Management
- **Responsibility**: Parse YAML config, validate structure, provide defaults
- **Key struct**: `Config` - root configuration struct
- **Features**:
  - Loads from YAML file
  - Supports `conf.d/` directory for modular configs
  - Environment variable overrides (prefix: `MP_`)
- **Key types**: `Config`, `Datasource`, `ServerConf`, `StatusDashboardConfig`

**When to edit**: Adding new configuration fields, changing validation logic

#### `types.rs` - Domain Data Structures
- **Responsibility**: Define core domain entities and their relationships
- **Key structs**:
  - `FlagMetricDef`: Flag metric definition from config
  - `FlagMetric`: Runtime flag metric with evaluation state
  - `ServiceHealthDef`: Health metric definition from config
  - `HealthMetric`: Runtime health metric with computed status
  - `BinaryMetricRawDef`: Metric template definition
  - `EnvironmentDef`: Environment configuration
  - `Expression`: Boolean expression with weight

**When to edit**: Adding new domain concepts, changing data models

#### `graphite.rs` - Graphite TSDB Integration
- **Responsibility**: Query Graphite API, parse responses, handle errors
- **Key struct**: `GraphiteClient` - HTTP client for Graphite
- **Key functions**:
  - `query()`: Execute Graphite query, return parsed results
  - `format_query()`: Build Graphite query URL with parameters
- **Dependencies**: Uses `reqwest` for HTTP, `serde_json` for parsing

**When to edit**: Changing Graphite query logic, adding new TSDB backends

#### `common.rs` - Shared Utilities
- **Responsibility**: Helper functions used across modules
- **Contents**:
  - String utilities
  - Time/date helpers
  - Logging utilities
  - Error handling helpers

**When to edit**: Adding reusable utilities needed by multiple modules

## Module Dependencies

```
lib.rs (root)
├── api.rs
│   ├── api/v1.rs (depends on: types, config, graphite)
│   └── types.rs
├── config.rs (depends on: types)
├── types.rs (no dependencies)
├── graphite.rs (depends on: types)
└── common.rs (no dependencies)

bin/convertor.rs (depends on: api, config, types)
bin/reporter.rs (depends on: config, types)
```

**Dependency Rules**:
- `types.rs` has no dependencies (pure data structures)
- `common.rs` has no dependencies (pure utilities)
- All other modules may depend on `types` and `common`
- Binaries depend on library modules, not vice versa

## Documentation Organization (`doc/`)

### Human-Readable Documentation
- **Getting Started**: Developer onboarding (quickstart, project structure, development workflow)
- **Architecture**: System design, diagrams, data flow
- **API Reference**: Endpoint documentation, examples
- **Configuration**: Field reference, examples, troubleshooting
- **Integration**: Guides for adding TSDB backends
- **Modules**: Rust module responsibilities and APIs
- **Guides**: Operational guides (troubleshooting, deployment)

### Machine-Readable Schemas (`doc/schemas/`)
- **config-schema.json**: JSON Schema for configuration validation (auto-generated from `Config` struct)
- **patterns.json**: Code conventions and patterns for AI code generation
- **README.md**: Usage guide for IDE and AI tools

## Test Organization (`tests/`)

### Documentation Validation Tests
- **documentation_validation.rs**: Ensures documentation examples remain valid
  - Validates YAML examples parse correctly
  - Validates JSON schemas are well-formed
  - Checks internal links resolve

### Integration Tests (Future)
- **contract/**: Contract tests for API endpoints
- **integration/**: End-to-end integration tests
- **unit/**: Unit tests for specific modules

## Configuration Structure

Configuration files follow this hierarchy:

```
config.yaml (root)
├── datasource:         # TSDB connection
├── server:             # HTTP API binding
├── metric_templates:   # Query templates
├── environments:       # Environment definitions
├── flag_metrics:       # Flag metric definitions
├── health_metrics:     # Health metric definitions
└── status_dashboard:   # Dashboard integration
```

**Location**: Typically `config.yaml` at project root, with optional `conf.d/*.yaml` for modular configs.

## Key Design Patterns

### 1. Configuration-Driven Logic
All metric evaluation logic is defined in YAML config, not code. This enables:
- Adding services without code changes
- Customizing thresholds per environment
- Modifying health expressions without recompilation

### 2. Separation of Concerns
- **convertor**: Stateless metric evaluation, pure function of config + TSDB data
- **reporter**: Stateless notification client, polls convertor API
- **No shared state**: Binaries can run independently on different hosts

### 3. Type Safety
Rust's type system ensures:
- Configuration is validated at startup
- Invalid configs fail fast with clear error messages
- No runtime type errors

### 4. Async I/O
All I/O operations use Tokio async runtime:
- Non-blocking TSDB queries
- Concurrent processing of multiple services
- Efficient resource usage

## Locating Code by Task

| Task | Module to Edit | Notes |
|------|---------------|-------|
| Add new API endpoint | `src/api/v1.rs` | Also update `openapi-schema.yaml` |
| Add config field | `src/config.rs`, `src/types.rs` | Update `build.rs` if schema changes |
| Change metric evaluation | `src/bin/convertor.rs` | Logic is config-driven, rarely needs code changes |
| Add new TSDB backend | Create `src/prometheus.rs` | Follow `graphite.rs` pattern, implement same trait |
| Change health logic | Config file | Boolean expressions in `health_metrics` section |
| Add new data type | `src/types.rs` | Define struct, add serde derives |
| Fix TSDB query | `src/graphite.rs` | Modify `query()` or `format_query()` functions |

## Common File Paths

| Purpose | Path |
|---------|------|
| Add dependency | `Cargo.toml` |
| Configuration schema | `doc/schemas/config-schema.json` (auto-generated) |
| API specification | `openapi-schema.yaml` |
| Example config | `doc/configuration/examples.md` |
| Quickstart guide | `doc/getting-started/quickstart.md` |
| Architecture diagrams | `doc/architecture/diagrams.md` |

## Next Steps

- **New to the codebase?** Start with [Quickstart Guide](quickstart.md)
- **Want to contribute?** Read [Development Workflow](development.md)
- **Need to understand architecture?** See [Architecture Overview](../architecture/overview.md)
- **Adding a feature?** Check [Module Documentation](../modules/overview.md) for responsibilities
