# Configuration Module

The configuration module (`src/config.rs`) handles loading, parsing, and merging configuration from YAML files and environment variables.

## Key Types

### Config

The main configuration structure:

```rust
#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    /// Datasource connection (Graphite TSDB)
    pub datasource: Datasource,
    /// Server binding configuration
    pub server: ServerConf,
    /// Metric templates for reuse across metrics
    pub metric_templates: Option<HashMap<String, BinaryMetricRawDef>>,
    /// Environment definitions
    pub environments: Vec<EnvironmentDef>,
    /// Flag metric definitions
    pub flag_metrics: Vec<FlagMetricDef>,
    /// Health metric definitions keyed by service name
    pub health_metrics: HashMap<String, ServiceHealthDef>,
    /// Status Dashboard integration config
    pub status_dashboard: Option<StatusDashboardConfig>,
}
```

### Datasource

TSDB connection settings:

```rust
#[derive(Clone, Debug, Deserialize)]
pub struct Datasource {
    /// Graphite URL (e.g., "https://graphite.example.com")
    pub url: String,
    /// Query timeout in seconds (default: 10)
    #[serde(default = "default_timeout")]
    pub timeout: u16,
}
```

### ServerConf

HTTP server binding:

```rust
#[derive(Clone, Debug, Deserialize)]
pub struct ServerConf {
    /// IP address to bind (default: "0.0.0.0")
    #[serde(default = "default_address")]
    pub address: String,
    /// Port to bind (default: 3000)
    #[serde(default = "default_port")]
    pub port: u16,
}
```

### StatusDashboardConfig

Optional status dashboard integration:

```rust
#[derive(Clone, Debug, Deserialize)]
pub struct StatusDashboardConfig {
    /// Status dashboard URL
    pub url: String,
    /// JWT token signature secret
    pub secret: Option<String>,
}
```

## Configuration Loading

### Primary Method: `Config::new()`

```rust
pub fn new(config_file: &str) -> Result<Self, ConfigError>
```

Loading process:
1. **Load main config file** - YAML file at specified path
2. **Merge conf.d parts** - All `*.yaml` files in `{config_dir}/conf.d/`
3. **Merge environment variables** - Variables prefixed with `MP_`

### Environment Variable Merging

Environment variables use `MP_` prefix with `__` as sublevel separator:

| Environment Variable | Config Path |
|---------------------|-------------|
| `MP_DATASOURCE__URL` | `datasource.url` |
| `MP_SERVER__PORT` | `server.port` |
| `MP_STATUS_DASHBOARD__SECRET` | `status_dashboard.secret` |

```rust
Environment::with_prefix("MP")
    .prefix_separator("_")
    .separator("__")
```

### Alternative: `Config::from_config_str()`

For testing, load config from a string:

```rust
#[allow(dead_code)]
pub fn from_config_str(data: &str) -> Self
```

## Configuration File Format

### Example Configuration

```yaml
---
datasource:
  url: 'https://graphite.example.com'
  timeout: 30

server:
  address: '0.0.0.0'
  port: 3005

metric_templates:
  api_latency:
    query: 'summarize($environment.$service.latency, "1h", "avg")'
    op: lt
    threshold: 1000

environments:
  - name: production
    attributes:
      region: eu-de
  - name: staging

flag_metrics:
  - name: api-latency
    service: compute
    template:
      name: api_latency
    environments:
      - name: production
        threshold: 500
      - name: staging

health_metrics:
  compute:
    service: compute
    category: compute
    component_name: "Compute Service"
    metrics:
      - compute.api-latency
      - compute.availability
    expressions:
      - expression: 'compute.api_latency && compute.availability'
        weight: 1
      - expression: 'compute.api_latency || compute.availability'
        weight: 2

status_dashboard:
  url: 'https://status.example.com'
  secret: ${MP_STATUS_DASHBOARD__SECRET}
```

### Modular Configuration (conf.d)

Split configuration into multiple files:

```
config/
├── config.yaml           # Main config
└── conf.d/
    ├── compute.yaml      # Compute service metrics
    ├── storage.yaml      # Storage service metrics
    └── network.yaml      # Network service metrics
```

Each conf.d file can contain partial configuration that gets merged.

## Helper Methods

### `get_socket_addr()`

Returns a `SocketAddr` for server binding:

```rust
pub fn get_socket_addr(&self) -> SocketAddr {
    SocketAddr::from((
        self.server.address.as_str().parse::<IpAddr>().unwrap(),
        self.server.port,
    ))
}
```

## Default Values

| Field | Default |
|-------|---------|
| `server.address` | `"0.0.0.0"` |
| `server.port` | `3000` |
| `datasource.timeout` | `10` |

## Validation

Configuration validation happens during deserialization. Missing required fields or type mismatches will cause `ConfigError` to be returned.

## Dependencies

- `config` crate - Configuration loading and merging
- `serde` - Deserialization
- `glob` - Finding conf.d files
