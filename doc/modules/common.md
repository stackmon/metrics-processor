# Common Module

The common module (`src/common.rs`) provides shared utility functions for metric evaluation and service health calculation.

## Functions

### get_metric_flag_state()

Converts a raw metric value to a boolean flag based on the configured threshold and comparison operator.

```rust
pub fn get_metric_flag_state(value: &Option<f32>, metric: &FlagMetric) -> bool
```

**Parameters:**
- `value` - Raw metric value from Graphite (may be `None` for missing data)
- `metric` - Flag metric configuration with operator and threshold

**Logic:**
```rust
return match *value {
    Some(x) => match metric.op {
        CmpType::Lt => x < metric.threshold,   // value < threshold = healthy
        CmpType::Gt => x > metric.threshold,   // value > threshold = healthy  
        CmpType::Eq => x == metric.threshold,  // value == threshold = healthy
    },
    None => false,  // Missing data = unhealthy
};
```

**Usage Example:**
```rust
let metric = FlagMetric {
    query: "service.latency".to_string(),
    op: CmpType::Lt,
    threshold: 1000.0,
};

// Latency of 500ms is healthy (500 < 1000)
assert!(get_metric_flag_state(&Some(500.0), &metric));

// Latency of 1500ms is unhealthy (1500 >= 1000)
assert!(!get_metric_flag_state(&Some(1500.0), &metric));

// Missing data is unhealthy
assert!(!get_metric_flag_state(&None, &metric));
```

### get_service_health()

Calculates aggregated health scores for a service based on multiple flag metrics and boolean expressions.

```rust
pub async fn get_service_health(
    state: &AppState,
    service: &str,
    environment: &str,
    from: &str,
    to: &str,
    max_data_points: u16,
) -> Result<ServiceHealthData, CloudMonError>
```

**Parameters:**
- `state` - Application state containing configurations and HTTP client
- `service` - Service name to evaluate
- `environment` - Environment name
- `from` / `to` - Time range (RFC3339 format or Graphite relative time)
- `max_data_points` - Maximum data points to return

**Returns:**
- `Ok(ServiceHealthData)` - Vector of `(timestamp, health_score)` tuples
- `Err(CloudMonError)` - On service not found, environment not supported, or evaluation error

## Health Calculation Algorithm

### Step 1: Validate Service

```rust
if !state.health_metrics.contains_key(service) {
    return Err(CloudMonError::ServiceNotSupported);
}
```

### Step 2: Build Graphite Query Map

```rust
let mut graphite_targets: HashMap<String, String> = HashMap::new();
for metric_name in metric_names.iter() {
    if let Some(metric) = state.flag_metrics.get(metric_name) {
        match metric.get(environment) {
            Some(m) => {
                graphite_targets.insert(metric_name.clone(), m.query.clone());
            }
            _ => return Err(CloudMonError::EnvNotSupported),
        };
    }
}
```

### Step 3: Fetch Data from Graphite

```rust
let raw_data: Vec<GraphiteData> = graphite::get_graphite_data(
    &state.req_client,
    &state.config.datasource.url,
    &graphite_targets,
    // ... time parameters
).await?;
```

### Step 4: Organize Data by Timestamp

```rust
let mut metrics_map: BTreeMap<u32, HashMap<String, bool>> = BTreeMap::new();

for data_element in raw_data.iter() {
    let metric = metric_cfg.get(environment).unwrap();
    for (val, ts) in data_element.datapoints.iter() {
        metrics_map.entry(*ts).or_insert(HashMap::new()).insert(
            data_element.target.clone(),
            get_metric_flag_state(val, metric),
        );
    }
}
```

### Step 5: Evaluate Health Expressions

Uses the `evalexpr` crate for boolean expression evaluation:

```rust
for (ts, ts_val) in metrics_map.iter() {
    let mut context = HashMapContext::new();
    
    // Build context with all metric flags
    for metric in hm_config.metrics.iter() {
        let xval = ts_val.get(metric).unwrap_or(&false);
        context.set_value(
            metric.replace("-", "_").into(),  // evalexpr doesn't support "-"
            Value::from(*xval)
        ).unwrap();
    }
    
    // Evaluate expressions in order of weight
    let mut expression_res: u8 = 0;
    for expr in hm_config.expressions.iter() {
        if expr.weight as u8 <= expression_res {
            continue;  // Skip lower-weight expressions
        }
        if eval_boolean_with_context(expr.expression.as_str(), &context)? {
            expression_res = expr.weight as u8;
        }
    }
    
    result.push((*ts, expression_res));
}
```

## Expression Evaluation

### Supported Operators

The `evalexpr` crate supports standard boolean operators:
- `&&` - Logical AND
- `||` - Logical OR
- `!` - Logical NOT
- Parentheses for grouping

### Example Expressions

```yaml
expressions:
  # Both metrics must be healthy
  - expression: 'api_latency && availability'
    weight: 1
  
  # Either metric being unhealthy triggers warning
  - expression: '!api_latency || !availability'
    weight: 2
  
  # Complex conditions
  - expression: '(api_latency && availability) || backup_service'
    weight: 1
```

### Metric Name Handling

Metric names with hyphens are automatically converted:
- Config: `service.metric-name`
- Expression context: `service.metric_name`

```rust
context.set_value(
    metric.replace("-", "_").into(),
    Value::from(xval)
)
```

## Data Flow

```
get_service_health()
        │
        ├──► Validate service exists in health_metrics
        │
        ├──► Build target->query map from flag_metrics
        │
        ├──► graphite::get_graphite_data()
        │           │
        │           ▼
        │    Upstream Graphite TSDB
        │
        ├──► Reorganize: [target, [(val, ts)]] → {ts: {target: bool}}
        │
        ├──► For each timestamp:
        │       ├──► Build evalexpr context
        │       ├──► Evaluate expressions by weight
        │       └──► Record highest matching weight
        │
        └──► Return Vec<(timestamp, health_score)>
```

## Dependencies

- `evalexpr` - Boolean expression evaluation
- `chrono` - Date/time parsing
- `crate::graphite` - Graphite data fetching
- `crate::types` - Core types (`AppState`, `FlagMetric`, etc.)
