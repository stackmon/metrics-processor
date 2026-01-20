# Troubleshooting Guide

This guide covers common issues encountered when operating the metrics-processor and provides actionable solutions.

## Configuration Errors

### Invalid YAML Syntax

**Symptom:**
```
thread 'main' panicked at 'called `Result::unwrap()` on an `Err` value: 
while parsing a block mapping, did not find expected key at line 5 column 1'
```

**Cause:** Malformed YAML syntax such as incorrect indentation, missing colons, or unquoted special characters.

**Solution:**
1. Validate your YAML file with a linter:
   ```bash
   yamllint config.yaml
   ```
2. Common fixes:
   - Ensure consistent indentation (use 2 spaces)
   - Quote strings containing special characters: `url: "https://graphite.example.com"`
   - Escape special characters in queries

**Related logs:** Application exits at startup with panic message.

---

### Missing Required Fields

**Symptom:**
```
Error: missing field `datasource` at line 1 column 1
```
or
```
Error: missing field `environments` at line 1 column 1
```

**Cause:** Required configuration sections are not present in `config.yaml`.

**Solution:** Ensure all required sections are present:
```yaml
datasource:
  url: https://graphite.example.com

server:
  address: 0.0.0.0
  port: 3000

environments:
  - name: production

flag_metrics: []

health_metrics: {}
```

**Related logs:** Application fails to start with deserialization error.

---

### Threshold Type Mismatch

**Symptom:**
```
Error: invalid type: string "90", expected f32 at line 12 column 16
```

**Cause:** Threshold values must be numeric (float), not strings.

**Solution:**
```yaml
# Wrong
metric_templates:
  api_success_rate:
    threshold: "90"  # String - incorrect

# Correct
metric_templates:
  api_success_rate:
    threshold: 90    # Numeric - correct
    threshold: 90.5  # Decimal also valid
```

**Related logs:** Configuration parsing error at startup.

---

### Invalid Operator Type

**Symptom:**
```
Error: unknown variant `less_than`, expected one of `lt`, `gt`, `eq`
```

**Cause:** The `op` field in metric templates only accepts `lt`, `gt`, or `eq`.

**Solution:**
```yaml
metric_templates:
  api_down:
    query: "..."
    op: lt        # Valid: lt (less than)
    # op: gt      # Valid: gt (greater than)
    # op: eq      # Valid: eq (equal to)
    threshold: 100
```

**Related logs:** Configuration parsing error at startup.

---

## TSDB Connection Issues

### Connection Timeout

**Symptom:**
```
ERROR cloudmon_metrics::graphite: Error: error sending request for url (https://graphite.example.com/render)
```
or API returns:
```json
{"message": "Error reading data from TSDB"}
```

**Cause:** Graphite server is unreachable, slow, or the timeout is too short.

**Solution:**
1. Verify Graphite connectivity:
   ```bash
   curl -v "https://graphite.example.com/render?target=*&format=json&from=-5min"
   ```
2. Increase timeout in configuration:
   ```yaml
   datasource:
     url: https://graphite.example.com
     timeout: 30  # Increase from default 10 seconds
   ```
3. Check network connectivity and firewall rules.

**Related logs:**
```
ERROR cloudmon_metrics::graphite: Error: reqwest::Error { kind: TimedOut }
```

---

### Invalid URL Format

**Symptom:**
```
thread 'main' panicked at 'called `Result::unwrap()` on an `Err` value: 
builder error: relative URL without a base'
```

**Cause:** Datasource URL is malformed or missing protocol.

**Solution:**
```yaml
# Wrong
datasource:
  url: graphite.example.com  # Missing protocol

# Correct
datasource:
  url: https://graphite.example.com
```

**Related logs:** Application exits at startup or first TSDB query.

---

### Authentication Errors

**Symptom:**
```
ERROR cloudmon_metrics::graphite: Error: StatusCode(401)
```

**Cause:** Graphite server requires authentication but credentials are not provided.

**Solution:**
1. Include authentication in URL (if supported):
   ```yaml
   datasource:
     url: https://user:password@graphite.example.com
   ```
2. Or configure reverse proxy with authentication headers.

**Related logs:**
```
ERROR cloudmon_metrics::graphite: Error: {"error": "Authentication required"}
```

---

## Metric Evaluation Errors

### Template Not Found

**Symptom:**
```
thread 'main' panicked at 'called `Option::unwrap()` on a `None` value'
```
during config processing, or metrics return empty data.

**Cause:** Flag metric references a template that doesn't exist in `metric_templates`.

**Solution:**
```yaml
# Ensure template exists
metric_templates:
  api_down:  # Template name
    query: "..."
    op: eq
    threshold: 100

flag_metrics:
  - name: api_down
    service: compute
    template:
      name: api_down  # Must match template name exactly
    environments:
      - name: production
```

**Related logs:**
```
ERROR cloudmon_metrics::types: Metric processing failed
```

---

### Variable Substitution Failures

**Symptom:** Queries return no data because variables weren't substituted.

**Cause:** Template variables `$environment` or `$service` not properly formatted.

**Solution:**
```yaml
metric_templates:
  api_down:
    # Variables use $name syntax (not ${name})
    query: "stats.counters.api.$environment.*.$service.*.failed.count"
    op: eq
    threshold: 100
```

Verify substitution by enabling trace logging:
```bash
RUST_LOG=trace ./cloudmon-metrics-convertor
```

**Related logs:**
```
TRACE cloudmon_metrics::types: Processing template with vars: {"service": "compute", "environment": "production"}
```

---

### Metric Name Not Found

**Symptom:**
```
WARN cloudmon_metrics::graphite: DB Response contains unknown target: srvA.metric-1
```

**Cause:** Graphite returned a metric target that doesn't match configured flag metrics.

**Solution:**
1. Verify metric naming matches between config and Graphite
2. Check the `alias()` function is working correctly in queries
3. Ensure service and metric names match: `{service}.{metric_name}`

**Related logs:**
```
WARN cloudmon_metrics::common: DB Response contains unknown target: unexpected_metric
```

---

## Health Expression Errors

### Syntax Errors in Expressions

**Symptom:**
```
DEBUG cloudmon_metrics::common: Error during evaluation of "comp1.api_down ||" [context: ...]: ...
```
API returns 500 Internal Server Error.

**Cause:** Health expression has invalid syntax (missing operand, invalid operator).

**Solution:**
```yaml
health_metrics:
  compute:
    expressions:
      # Wrong - missing second operand
      - expression: "comp1.api_down ||"
        weight: 1

      # Correct
      - expression: "comp1.api_down || comp1.api_slow"
        weight: 1
```

Valid operators: `||` (or), `&&` (and), `!` (not)

**Related logs:**
```
DEBUG cloudmon_metrics::common: Error during evaluation of "..." [context: {...}]: Expected token but found end of expression
```

---

### Undefined Metrics in Expression

**Symptom:**
```json
{"message": "Internal Expression evaluation error"}
```

**Cause:** Expression references a metric not listed in the `metrics` array.

**Solution:**
```yaml
health_metrics:
  compute:
    service: compute
    category: compute
    metrics:
      - compute.api_down      # Must list all metrics used
      - compute.api_slow
    expressions:
      - expression: "compute.api_down || compute.api_slow"
        weight: 1
```

**Note:** Hyphens in metric names are converted to underscores in expressions:
```yaml
metrics:
  - srvA.metric-1
expressions:
  - expression: "srvA.metric_1"  # Use underscore in expression
```

**Related logs:**
```
DEBUG cloudmon_metrics::common: Error during evaluation of "..." [context: {...}]: Identifier not found
```

---

## API Issues

### Service Not Supported

**Symptom:**
```json
{"message": "Service not supported"}
```
HTTP Status: 409 Conflict

**Cause:** Requested service doesn't exist in `health_metrics` configuration.

**Solution:**
1. Check available services in your configuration
2. Verify the exact service name in the API request:
   ```bash
   curl "http://localhost:3000/api/v1/health?service=compute&environment=production&from=-1h&to=now"
   ```
3. Add the service to configuration if missing

**Related logs:**
```
DEBUG cloudmon_metrics::api::v1: Processing query HealthQuery { service: "unknown_service", ... }
```

---

### Environment Not Supported

**Symptom:**
```json
{"message": "Environment for service not supported"}
```
HTTP Status: 409 Conflict

**Cause:** The requested environment isn't configured for the service.

**Solution:**
```yaml
environments:
  - name: production  # Must be listed here

flag_metrics:
  - name: api_down
    service: compute
    template:
      name: api_down
    environments:
      - name: production  # Environment must be listed per metric
```

**Related logs:**
```
DEBUG cloudmon_metrics::common: Can not find metric api_down for env staging
```

---

### Invalid Query Parameters

**Symptom:**
```
HTTP 400 Bad Request
```
or missing data in response.

**Cause:** Required query parameters missing or malformed.

**Solution:** Ensure all required parameters are provided:
```bash
curl "http://localhost:3000/api/v1/health?\
service=compute&\
environment=production&\
from=2024-01-01T00:00:00Z&\
to=2024-01-01T01:00:00Z&\
max_data_points=100"
```

Required parameters:
- `service` - Service name
- `environment` - Environment name  
- `from` - Start time (RFC3339 or relative like `-1h`)
- `to` - End time (RFC3339 or relative like `now`)

**Related logs:**
```
DEBUG cloudmon_metrics::api::v1: Processing query HealthQuery { ... }
```

---

## Reporter Issues

### Status Dashboard Connection Failed

**Symptom:**
```
ERROR cloudmon_metrics: Error during posting component status: error sending request
```

**Cause:** Status dashboard is unreachable or misconfigured.

**Solution:**
1. Verify status dashboard URL:
   ```yaml
   status_dashboard:
     url: https://status.cloudmon.com
     secret: your-jwt-secret
   ```
2. Test connectivity:
   ```bash
   curl -v https://status.cloudmon.com/v1/component_status
   ```

**Related logs:**
```
ERROR cloudmon_metrics: Error: [401] "{"error": "Invalid token"}"
```

---

### Missing Component Name

**Symptom:**
```
WARN cloudmon_metrics: No component_name is given for compute
```

**Cause:** Health metric is missing the `component_name` field needed for status reporting.

**Solution:**
```yaml
health_metrics:
  compute:
    service: compute
    component_name: "Elastic Cloud Server"  # Required for reporter
    category: compute
    metrics: [...]
```

**Related logs:** Warning during reporter startup.

---

## Performance Issues

### Slow Queries

**Symptom:** API requests take more than 10 seconds or timeout.

**Cause:** 
- Graphite queries are too complex
- Too many datapoints requested
- Network latency

**Solution:**
1. Limit datapoints in requests:
   ```bash
   curl "...&max_data_points=100"  # Instead of default
   ```
2. Use consolidation in Graphite queries:
   ```yaml
   query: "consolidateBy(avg($environment.$service.*.count), 'average')"
   ```
3. Increase timeout if needed:
   ```yaml
   datasource:
     timeout: 30
   ```
4. Consider caching at reverse proxy level

**Related logs:**
```
TRACE cloudmon_metrics::graphite: Query: [("format", "json"), ("maxDataPoints", "1000"), ...]
```

---

### High Memory Usage

**Symptom:** Container OOM killed or excessive memory consumption.

**Cause:** 
- Too many datapoints being processed
- Many concurrent requests
- Memory leak (rare)

**Solution:**
1. Limit `max_data_points` in configuration and requests
2. Set memory limits in container:
   ```yaml
   resources:
     limits:
       memory: 256Mi
   ```
3. Monitor with:
   ```bash
   docker stats metrics-processor
   ```

**Related logs:** Check system logs for OOM events.

---

## Debugging Tips

### Enable Debug Logging

```bash
# Info level (default)
RUST_LOG=info ./cloudmon-metrics-convertor

# Debug level (more detail)
RUST_LOG=debug ./cloudmon-metrics-convertor

# Trace level (very verbose)
RUST_LOG=trace ./cloudmon-metrics-convertor

# Module-specific
RUST_LOG=cloudmon_metrics::graphite=trace ./cloudmon-metrics-convertor
```

### Verify Configuration Loading

```bash
RUST_LOG=debug ./cloudmon-metrics-convertor 2>&1 | grep -i config
```

### Test Graphite Connectivity

```bash
# Direct Graphite query
curl "https://graphite.example.com/render?target=*&format=json&from=-5min&maxDataPoints=10"

# Via metrics-processor
curl "http://localhost:3000/metrics/find?query=*"
```

### Check Health Endpoint

```bash
curl -s "http://localhost:3000/api/v1/health?\
service=compute&\
environment=production&\
from=-1h&\
to=now&\
max_data_points=10" | jq .
```

---

## Getting Help

If you encounter issues not covered in this guide:

1. Check the logs with `RUST_LOG=debug` or `RUST_LOG=trace`
2. Verify your configuration against the schema in `doc/configuration/schema.md`
3. Open an issue with:
   - Configuration (sanitized)
   - Error messages
   - Log output
   - Steps to reproduce
