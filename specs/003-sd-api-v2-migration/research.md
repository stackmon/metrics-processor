# Research: SD API V2 Migration

**Date**: 2025-01-22  
**Feature**: Reporter Migration to Status Dashboard API V2  
**Branch**: `003-sd-api-v2-migration`

## Overview

This document consolidates research findings for migrating the cloudmon-metrics-reporter from Status Dashboard API V1 to V2. All decisions are informed by:
- OpenAPI schema at `/openapi.yaml`
- Feature specification requirements (17 FRs)
- Existing V1 implementation in `src/bin/reporter.rs`
- Reference implementation in branch `sd_api_v2_migration`

---

## 1. Component Cache Design

### Decision: HashMap<String, HashMap<String, u64>> (Nested Hash Maps)

**Rationale**: 
Component resolution requires matching both `name` and `attributes` as a composite key. The cache structure is:
```rust
HashMap<
    String,                    // Component name (e.g., "Object Storage Service")
    HashMap<String, u64>      // Attributes hash -> Component ID
>
```

Where the inner `String` key is a deterministic hash of sorted attributes (e.g., "category=Storage,region=EU-DE").

**Why this approach**:
1. **Fast lookup**: O(1) for name, O(1) for attribute hash = O(1) total
2. **Subset matching support**: FR-012 requires matching where configured attributes are a subset of component's attributes. We compute the hash from configured attributes and find matches.
3. **Rust-idiomatic**: Uses standard library `HashMap` with no external dependencies
4. **Memory efficient**: ~10-100 components typical; minimal overhead

**Alternatives considered**:
- **Option A: Vec<Component> with linear search** - O(n) lookup, too slow for 60s monitoring cycles
- **Option B: BTreeMap for sorted iteration** - Unnecessary; lookup order doesn't matter
- **Option C: Custom index struct** - Overengineering for simple cache

**Implementation notes**:
- Attributes sorted lexicographically before hashing to ensure deterministic keys
- Cache refresh on miss (FR-005) rebuilds entire cache from `/v2/components` GET

---

## 2. V2 Incident Payload Construction

### Decision: Static struct with serde serialization

**Rationale**:
V2 incident creation uses a fixed payload structure per OpenAPI schema:

```rust
#[derive(Serialize)]
struct IncidentPost {
    title: String,           // Static: "System incident from monitoring system"
    description: String,     // Static: "System-wide incident affecting one or multiple components. Created automatically."
    impact: u8,              // From health metric (0-3)
    components: Vec<u64>,    // Resolved component IDs
    start_date: String,      // RFC3339, from health timestamp - 1s
    system: bool,            // Always true
    #[serde(rename = "type")]
    incident_type: String,   // Always "incident"
}
```

**Why this approach**:
1. **Type safety**: Compile-time validation via Rust structs + serde derive
2. **Security compliance**: FR-002/FR-017 separation - sensitive data in logs, generic data in API
3. **OpenAPI alignment**: Fields match schema exactly (using `#[serde(rename)]` for "type" keyword)
4. **Maintainability**: Single source of truth for payload structure

**Alternatives considered**:
- **Option A: Manual JSON construction** - Error-prone, no compile-time checks
- **Option B: Dynamic template strings** - Harder to test, type-unsafe
- **Option C: Builder pattern** - Overkill for simple static payload

**Security implementation**:
Per FR-002 clarifications (Session 2026-01-22):
- **API fields**: Generic static messages (title, description)
- **Local logs**: Detailed diagnostic info (service, environment, component attributes, triggered metrics per FR-017)
- **Separation enforced**: Incident struct does NOT include sensitive fields; logging uses separate context variables

---

## 3. Error Handling: Cache Refresh Scenarios

### Decision: Retry with exponential backoff for initial load; single retry for cache miss

**Rationale**:

**Initial cache load (startup)**:
```rust
// FR-006: Retry up to 3 times with 60s delays
for attempt in 1..=3 {
    match fetch_components().await {
        Ok(components) => { build_cache(components); break; }
        Err(e) if attempt < 3 => {
            tracing::warn!("Cache load attempt {}/3 failed: {}", attempt, e);
            sleep(Duration::from_secs(60)).await;
        }
        Err(e) => {
            tracing::error!("Failed to load component cache after 3 attempts");
            return Err(e);  // FR-007: Fail to start
        }
    }
}
```

**Cache miss during runtime**:
```rust
// FR-005: Refresh on miss, retry lookup once
if cache.get(name, attrs).is_none() {
    tracing::info!("Component not found in cache; refreshing");
    refresh_cache().await?;  // Single refresh attempt
    if cache.get(name, attrs).is_none() {
        tracing::warn!("Component {} still not found after refresh", name);
        // FR-015: Continue to next service, don't retry incident creation
        continue;
    }
}
```

**Why this approach**:
1. **Startup reliability**: 3 retries with 60s delays handle temporary API unavailability (SC-004: starts within 3min)
2. **Runtime resilience**: Single cache refresh on miss handles new components added to Status Dashboard (FR-005)
3. **No retry on incident creation failure**: Per FR-015, log error and rely on next monitoring cycle (~60s)
4. **Constitution alignment**: Clear error messages (III. User Experience) and async operations (IV. Performance)

**Alternatives considered**:
- **Option A: Infinite retries** - Blocks startup indefinitely; violates SC-004
- **Option B: Exponential backoff during runtime** - Delays monitoring cycle; FR-015 says rely on next cycle
- **Option C: Circuit breaker pattern** - Overengineering; simple retry sufficient

**Error logging**:
Per Constitution III (Logging Standards):
- Include request IDs via tower-http middleware (already configured)
- Log HTTP status codes and response bodies on errors (SC-006)
- Use structured fields: `component_name`, `attributes`, `http_status`, `response_body`

---

## 4. Testing Strategies: Async HTTP with Mockito

### Decision: mockito 1.0 for HTTP mocking + tokio-test for async assertions

**Rationale**:
Testing async reporter logic requires:
1. **HTTP mocking**: Simulate `/v2/components` and `/v2/incidents` responses
2. **Async runtime**: Execute tokio futures in tests
3. **Deterministic timing**: Control retry delays for fast tests

**Test structure**:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use mockito::{mock, Mock};
    use tokio_test::block_on;

    #[test]
    fn test_cache_load_success() {
        let mut server = mockito::Server::new();
        let m = server.mock("GET", "/v2/components")
            .with_status(200)
            .with_body(r#"[{"id":1,"name":"Service A","attributes":[]}]"#)
            .create();
        
        let cache = block_on(fetch_and_build_cache(&server.url()));
        assert!(cache.get("Service A", &[]).is_some());
        m.assert();
    }

    #[test]
    fn test_cache_refresh_on_miss() {
        // Mock initial load with component A
        // Mock refresh returning component A + B
        // Verify lookup finds B after refresh
    }

    #[test]
    fn test_incident_creation_with_static_description() {
        // Mock POST /v2/incidents
        // Verify payload contains generic description (not service/env details)
        // Verify logs contain diagnostic details (FR-017)
    }
}
```

**Why this approach**:
1. **mockito 1.0**: Already in dev-dependencies; simple HTTP mock setup
2. **tokio-test**: Lightweight async test utilities; no heavyweight framework needed
3. **Constitution alignment**: II. Testing Excellence - integration tests in `#[cfg(test)]` modules

**Alternatives considered**:
- **Option A: wiremock crate** - More features but heavier dependency; mockito sufficient
- **Option B: Real HTTP server in tests** - Flaky, slow, requires network
- **Option C: Trait-based mocking** - Overengineering; HTTP layer is the right boundary

**Test coverage targets**:
Per Constitution II (Unit Test Coverage: 95%):
- Component cache: load, refresh, subset matching (FR-012)
- Incident payload: field values, serde serialization
- Error scenarios: cache failures, HTTP timeouts, malformed responses
- Retry logic: initial load retries, cache refresh

---

## 5. Authorization: HMAC-JWT Token (Unchanged)

### Decision: Reuse existing V1 authorization mechanism

**Rationale**:
FR-008 explicitly states "continue using the existing HMAC-signed JWT authorization mechanism without changes." Current V1 code:
```rust
let key: Hmac<Sha256> = Hmac::new_from_slice(secret.as_bytes())?;
let mut claims = BTreeMap::new();
claims.insert("stackmon", "dummy");
let token_str = claims.sign_with_key(&key)?;
headers.insert(AUTHORIZATION, format!("Bearer {}", token_str).parse()?);
```

**No changes required**: V2 endpoints accept same Authorization header format.

**Alternatives considered**: None - FR-008 is explicit.

---

## 6. Timestamp Handling: start_date Field

### Decision: Use health metric timestamp minus 1 second, formatted as RFC3339

**Rationale**:
FR-011 specifies: "Use the timestamp from the health metric as the start_date, adjusted by -1 second to align with monitoring intervals."

Current V1 implementation gets timestamp from:
```rust
let last = data.metrics.pop();  // (timestamp, impact) tuple
// last.0 is the timestamp
```

V2 implementation:
```rust
use chrono::{DateTime, Utc, Duration};

let timestamp_secs = last.0 as i64;
let dt = DateTime::<Utc>::from_timestamp(timestamp_secs, 0).unwrap();
let start_date = (dt - Duration::seconds(1)).to_rfc3339();
```

**Why this approach**:
1. **RFC3339 compliance**: OpenAPI schema specifies `format: date-time` (RFC3339)
2. **chrono crate**: Already in dependencies (v0.4); standard Rust datetime library
3. **-1 second adjustment**: Aligns with monitoring interval logic per FR-011

**Alternatives considered**:
- **Option A: Manual RFC3339 formatting** - Error-prone; chrono is reliable
- **Option B: Use timestamp as-is** - Violates FR-011 specification

---

## Summary of Research Findings

| Topic            | Decision                                | Key Constraint                       |
|------------------|-----------------------------------------|--------------------------------------|
| Component Cache  | Nested HashMap with attribute hash keys | FR-004, FR-012 (subset matching)     |
| Incident Payload | Static serde struct with generic fields | FR-002, FR-017 (security separation) |
| Error Handling   | 3x retry on startup, 1x refresh on miss | FR-005, FR-006, FR-007, FR-015       |
| Testing          | mockito + tokio-test                    | Constitution II (95% coverage)       |
| Authorization    | Unchanged HMAC-JWT                      | FR-008                               |
| Timestamps       | RFC3339, -1 second adjustment           | FR-011                               |

All decisions traceable to specific functional requirements or Constitution principles. No unknowns remaining - proceed to Phase 1 (Design).
