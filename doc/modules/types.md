# Types Module

The types module (`src/types.rs`) defines the core data structures used throughout the metrics-processor application.

## Metric Comparison Types

### CmpType

Enum for metric comparison operations:

```rust
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CmpType {
    Lt,  // Less than
    Gt,  // Greater than
    Eq,  // Equal to
}
```

Used to determine when a metric value should be flagged as "healthy" or "unhealthy".

## Metric Definition Types

### BinaryMetricRawDef

Raw metric template definition (used in `metric_templates`):

```rust
#[derive(Clone, Debug, Deserialize)]
pub struct BinaryMetricRawDef {
    /// Graphite query template (supports $var substitution)
    pub query: String,
    /// Comparison operator
    pub op: CmpType,
    /// Threshold value for comparison
    pub threshold: f32,
}
```

### BinaryMetricDef

Metric definition with optional template reference:

```rust
#[derive(Clone, Debug, Deserialize)]
pub struct BinaryMetricDef {
    pub query: Option<String>,
    pub op: Option<CmpType>,
    pub threshold: Option<f32>,
    pub template: Option<MetricTemplateRef>,
}
```

### MetricTemplateRef

Reference to a named template:

```rust
#[derive(Clone, Debug, Deserialize)]
pub struct MetricTemplateRef {
    /// Template name (key in metric_templates)
    pub name: String,
    /// Optional variable substitutions
    pub vars: Option<HashMap<String, String>>,
}
```

### FlagMetricDef

Configuration definition for a flag metric:

```rust
#[derive(Clone, Debug, Deserialize)]
pub struct FlagMetricDef {
    /// Metric name
    pub name: String,
    /// Service this metric belongs to
    pub service: String,
    /// Template reference
    pub template: Option<MetricTemplateRef>,
    /// Per-environment overrides
    pub environments: Vec<MetricEnvironmentDef>,
}
```

### FlagMetric

Processed/resolved flag metric (runtime):

```rust
#[derive(Clone, Debug, Deserialize)]
pub struct FlagMetric {
    /// Resolved Graphite query
    pub query: String,
    /// Comparison operator
    pub op: CmpType,
    /// Threshold value
    pub threshold: f32,
}
```

### MetricEnvironmentDef

Per-environment metric overrides:

```rust
#[derive(Clone, Debug, Deserialize)]
pub struct MetricEnvironmentDef {
    /// Environment name
    pub name: String,
    /// Optional threshold override
    pub threshold: Option<f32>,
}
```

## Environment Types

### EnvironmentDef

Environment definition:

```rust
#[derive(Clone, Debug, Deserialize)]
pub struct EnvironmentDef {
    /// Environment name (e.g., "production", "staging")
    pub name: String,
    /// Optional attributes for template substitution
    pub attributes: Option<HashMap<String, String>>,
}
```

## Health Metric Types

### ServiceHealthDef

Service health configuration:

```rust
#[derive(Clone, Debug, Deserialize)]
pub struct ServiceHealthDef {
    /// Service identifier
    pub service: String,
    /// Optional display name
    pub component_name: Option<String>,
    /// Category (e.g., "compute", "storage", "network")
    pub category: String,
    /// List of flag metric names to evaluate
    pub metrics: Vec<String>,
    /// Expressions for health calculation
    pub expressions: Vec<MetricExpressionDef>,
}
```

### MetricExpressionDef

Boolean expression for health evaluation:

```rust
#[derive(Clone, Debug, Deserialize)]
pub struct MetricExpressionDef {
    /// Boolean expression (e.g., "metric_a && metric_b")
    pub expression: String,
    /// Weight/severity (higher = more severe)
    pub weight: i32,
}
```

## Data Types

### MetricPoints / MetricData

Time series data structures:

```rust
/// Timestamp -> boolean flag mapping
pub type MetricPoints = BTreeMap<u32, bool>;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MetricData {
    pub target: String,
    #[serde(rename(serialize = "datapoints"))]
    pub points: MetricPoints,
}

/// Service health time series: Vec<(timestamp, health_value)>
pub type ServiceHealthData = Vec<(u32, u8)>;
```

## Error Types

### CloudMonError

Application-specific error enum:

```rust
pub enum CloudMonError {
    ServiceNotSupported,   // Requested service not found
    EnvNotSupported,       // Environment not configured for service
    ExpressionError,       // Boolean expression evaluation failed
    GraphiteError,         // TSDB communication error
}
```

Implements `std::error::Error`, `Display`, and `Debug`.

## Application State

### AppState

Central application state shared across handlers:

```rust
#[derive(Clone)]
pub struct AppState {
    /// Original configuration
    pub config: Config,
    /// Processed metric templates
    pub metric_templates: HashMap<String, BinaryMetricRawDef>,
    /// HTTP client for Graphite queries
    pub req_client: reqwest::Client,
    /// Processed flag metrics: service.metric -> env -> FlagMetric
    pub flag_metrics: HashMap<String, HashMap<String, FlagMetric>>,
    /// Health metric definitions by service name
    pub health_metrics: HashMap<String, ServiceHealthDef>,
    /// Environment definitions
    pub environments: Vec<EnvironmentDef>,
    /// Set of known service names
    pub services: HashSet<String>,
}
```

### AppState Methods

#### `new(config: Config) -> Self`

Creates new state with configured HTTP client:

```rust
impl AppState {
    pub fn new(config: Config) -> Self {
        let timeout = Duration::from_secs(config.datasource.timeout as u64);
        Self {
            config: config,
            metric_templates: HashMap::new(),
            flag_metrics: HashMap::new(),
            req_client: ClientBuilder::new().timeout(timeout).build().unwrap(),
            health_metrics: HashMap::new(),
            environments: Vec::new(),
            services: HashSet::new(),
        }
    }
}
```

#### `process_config(&mut self)`

Processes configuration into runtime structures:

1. **Copies metric templates** from config
2. **Resolves flag metrics** - substitutes `$service` and `$environment` variables in queries
3. **Processes health metrics** - replaces `-` with `_` in expression metric names (for evalexpr compatibility)
4. **Populates services set** for discovery endpoints

```rust
pub fn process_config(&mut self) {
    // Template variable substitution uses $var syntax
    let custom_regex = Regex::new(r"(?mi)\$([^\.]+)").unwrap();
    
    // Process flag_metrics with template resolution
    for metric_def in self.config.flag_metrics.iter() {
        // ... resolves template, substitutes variables
    }
    
    // Process health_metrics, fixing expression syntax
    for (metric_name, health_def) in self.config.health_metrics.iter() {
        // ... replaces "-" with "_" for evalexpr
    }
}
```

## Type Relationships

```
Config
  │
  ├── metric_templates: HashMap<String, BinaryMetricRawDef>
  │
  ├── flag_metrics: Vec<FlagMetricDef>
  │         │
  │         └── template: MetricTemplateRef
  │                         └──► BinaryMetricRawDef
  │
  └── health_metrics: HashMap<String, ServiceHealthDef>
                              │
                              ├── metrics: Vec<String>
                              └── expressions: Vec<MetricExpressionDef>
                      
                      ▼ process_config() ▼

AppState
  │
  ├── flag_metrics: HashMap<String, HashMap<String, FlagMetric>>
  │                         │                    │
  │                   "service.metric"      "environment"
  │
  └── health_metrics: HashMap<String, ServiceHealthDef>
```
