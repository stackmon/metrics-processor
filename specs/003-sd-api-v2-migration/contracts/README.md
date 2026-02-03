# API Contracts: Status Dashboard V2

**Feature**: Reporter Migration to Status Dashboard API V2  
**Date**: 2025-01-23

This directory contains API contract specifications for the Status Dashboard V2 endpoints used by the reporter.

## Files

- `components-api.yaml`: GET /v2/components endpoint contract
- `incidents-api.yaml`: POST /v2/incidents endpoint contract
- `request-examples/`: Sample request payloads
- `response-examples/`: Sample response payloads

## Source

All contracts are derived from the project's OpenAPI specification:
- **File**: `/openapi.yaml` (project root)
- **Version**: Status Dashboard API 1.0.0
- **Endpoints Used**:
  - `GET /v2/components` (line 138-151)
  - `POST /v2/incidents` (line 254-270)

## Testing

Contracts can be validated using OpenAPI tooling:

```bash
# Validate against OpenAPI schema
npx @redocly/cli lint openapi.yaml

# Generate mock server for testing
npx @stoplight/prism mock openapi.yaml
```

## Usage in Reporter

### Components Endpoint
```rust
// Fetch all components at startup
let components: Vec<StatusDashboardComponent> = 
    req_client.get(&format!("{}/v2/components", sdb_url))
    .send().await?
    .json().await?;
```

### Incidents Endpoint
```rust
// Create incident
let incident = IncidentData { /* ... */ };
let response: IncidentPostResponse = 
    req_client.post(&format!("{}/v2/incidents", sdb_url))
    .headers(auth_headers)
    .json(&incident)
    .send().await?
    .json().await?;
```
