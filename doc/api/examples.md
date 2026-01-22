# API Examples

This document provides practical examples for using the CloudMon Metrics Processor API.

## Health Metrics Examples

### Basic Health Query

Query health metrics for a single service in a specific environment:

```bash
curl -X GET "http://localhost:3000/v1/health?from=2024-01-01T00:00:00Z&to=2024-01-01T12:00:00Z&service=ecs&environment=eu-de"
```

**Response:**

```json
{
  "name": "ecs",
  "category": "compute",
  "environment": "eu-de",
  "metrics": [
    {
      "region": "eu-de-01",
      "datapoints": [
        [1704067200, 0],
        [1704067210, 0],
        [1704067220, 1],
        [1704067230, 0]
      ]
    }
  ]
}
```

### Query Multiple Services

Query metrics for multiple services by repeating the `service` parameter:

```bash
curl -X GET "http://localhost:3000/v1/health?\
from=2024-01-01T00:00:00Z&\
to=2024-01-01T12:00:00Z&\
service=ecs&\
service=evs&\
service=vpc&\
environment=eu-de"
```

### Limit Data Points

Limit the number of datapoints returned to optimize response size:

```bash
curl -X GET "http://localhost:3000/v1/health?\
from=2024-01-01T00:00:00Z&\
to=2024-01-07T00:00:00Z&\
service=ecs&\
environment=eu-de&\
max_data_points=50"
```

### Query Recent Metrics (Relative Time)

For internal usage, relative time expressions can be used:

```bash
curl -X GET "http://localhost:3000/api/v1/health?\
from=-5min&\
to=-2min&\
service=ecs&\
environment=eu-de"
```

---

## Maintenance Windows Examples

### List All Maintenances

Query all maintenance windows without filters:

```bash
curl -X GET "http://localhost:3000/v1/maintenances"
```

**Response:**

```json
[
  {
    "service": "ecs",
    "region": "eu-de",
    "start": "2024-01-15T02:00:00Z",
    "end": "2024-01-15T04:00:00Z",
    "reason": "Scheduled maintenance"
  },
  {
    "service": "evs",
    "region": "eu-de",
    "start": "2024-01-16T03:00:00Z",
    "end": "2024-01-16T05:00:00Z",
    "reason": "Storage upgrade"
  }
]
```

### Filter by Time Range

Query maintenances within a specific time window:

```bash
curl -X GET "http://localhost:3000/v1/maintenances?\
from=2024-01-01T00:00:00Z&\
to=2024-01-31T23:59:59Z"
```

### Filter by Service

Query maintenances for specific services:

```bash
curl -X GET "http://localhost:3000/v1/maintenances?\
service=ecs&\
service=evs&\
from=2024-01-01T00:00:00Z&\
to=2024-01-31T23:59:59Z"
```

---

## Error Handling Examples

### Missing Required Parameters

**Request:**

```bash
curl -X GET "http://localhost:3000/v1/health?service=ecs"
```

**Response (400 Bad Request):**

```json
{
  "error": "Missing required parameters: from, to, environment"
}
```

### Service Not Found

**Request:**

```bash
curl -X GET "http://localhost:3000/v1/health?\
from=2024-01-01T00:00:00Z&\
to=2024-01-01T12:00:00Z&\
service=nonexistent&\
environment=eu-de"
```

**Response (404 Not Found):**

```json
{
  "error": "Service 'nonexistent' not found in environment 'eu-de'"
}
```

### Invalid Date Format

**Request:**

```bash
curl -X GET "http://localhost:3000/v1/health?\
from=invalid-date&\
to=2024-01-01T12:00:00Z&\
service=ecs&\
environment=eu-de"
```

**Response (400 Bad Request):**

```json
{
  "error": "Invalid date format for parameter 'from'. Expected ISO 8601 format."
}
```

---

## Common Use Cases

### Dashboard Integration

Fetch current status for dashboard display:

```bash
#!/bin/bash

# Variables
BASE_URL="http://localhost:3000"
ENVIRONMENT="eu-de"
SERVICES=("ecs" "evs" "vpc" "elb" "rds")

# Build service params
SERVICE_PARAMS=""
for svc in "${SERVICES[@]}"; do
    SERVICE_PARAMS="${SERVICE_PARAMS}&service=${svc}"
done

# Calculate time range (last hour)
TO=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
FROM=$(date -u -d '1 hour ago' +"%Y-%m-%dT%H:%M:%SZ")

# Make request
curl -s "${BASE_URL}/v1/health?from=${FROM}&to=${TO}&environment=${ENVIRONMENT}${SERVICE_PARAMS}&max_data_points=100"
```

### Health Check Script

Check if any service is experiencing issues:

```bash
#!/bin/bash

BASE_URL="http://localhost:3000"
TO=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
FROM=$(date -u -d '5 minutes ago' +"%Y-%m-%dT%H:%M:%SZ")

response=$(curl -s "${BASE_URL}/v1/health?\
from=${FROM}&\
to=${TO}&\
service=ecs&\
environment=eu-de&\
max_data_points=10")

# Check for degradation (value > 0)
if echo "$response" | grep -q '"1"' || echo "$response" | grep -q '"2"'; then
    echo "WARNING: Service degradation detected!"
    exit 1
else
    echo "OK: All services healthy"
    exit 0
fi
```

### Maintenance Notification Script

Check for upcoming maintenances:

```bash
#!/bin/bash

BASE_URL="http://localhost:3000"

# Check next 24 hours
FROM=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
TO=$(date -u -d '24 hours' +"%Y-%m-%dT%H:%M:%SZ")

maintenances=$(curl -s "${BASE_URL}/v1/maintenances?from=${FROM}&to=${TO}")

# Check if any maintenances exist
count=$(echo "$maintenances" | jq length)

if [ "$count" -gt 0 ]; then
    echo "Upcoming maintenances in the next 24 hours:"
    echo "$maintenances" | jq -r '.[] | "- \(.service) in \(.region): \(.start) to \(.end) - \(.reason)"'
else
    echo "No scheduled maintenances in the next 24 hours."
fi
```

---

## Using with jq

### Extract Latest Status

```bash
curl -s "http://localhost:3000/v1/health?\
from=2024-01-01T00:00:00Z&\
to=2024-01-01T12:00:00Z&\
service=ecs&\
environment=eu-de" | jq '.metrics[0].datapoints[-1]'
```

### List Services with Issues

```bash
curl -s "http://localhost:3000/v1/health?\
from=2024-01-01T00:00:00Z&\
to=2024-01-01T12:00:00Z&\
service=ecs&\
service=evs&\
environment=eu-de" | jq '[.metrics[] | select(.datapoints[-1][1] > 0)]'
```

### Format Maintenance Schedule

```bash
curl -s "http://localhost:3000/v1/maintenances" | \
  jq -r '.[] | "\(.service) | \(.region) | \(.start) - \(.end) | \(.reason)"' | \
  column -t -s '|'
```

---

## HTTP Headers

### Request Headers

```bash
curl -X GET "http://localhost:3000/v1/health?..." \
  -H "Accept: application/json" \
  -H "Content-Type: application/json"
```

### Response Headers

Typical response headers:

```
HTTP/1.1 200 OK
Content-Type: application/json
Content-Length: 256
Date: Mon, 01 Jan 2024 12:00:00 GMT
```

---

## Status Code Reference

| Code | Meaning | Action |
|------|---------|--------|
| 200 | Success | Process the response data |
| 400 | Bad Request | Check request parameters |
| 404 | Not Found | Verify service/environment exists |
| 500 | Server Error | Retry after delay, check logs |
