# API Endpoints Reference

This document describes the REST API endpoints provided by the CloudMon Metrics Processor.

## Base URL

```
http://cloudmon.eco.tsi-dev.otc-service.com
```

---

## GET /v1/health

Retrieves platform health metrics for specified services and environments.

### Description

Get platform health metrics. The server supports querying metrics for up to 2 years. Older metrics are compressed to save storage space, which reduces data precision over time:

| Age | Precision |
|-----|-----------|
| 0-10 days | 10 seconds |
| 10-50 days | 1 minute |
| 50 days - 3 years | 10 minutes |
| > 3 years | Metrics are removed |

### Request

**Method:** `GET`

**URL:** `/v1/health`

**Operation ID:** `listHealthMetrics`

**Tags:** `metrics`

### Query Parameters

| Parameter | Type | Required | Description | Example |
|-----------|------|----------|-------------|---------|
| `from` | string (date-time) | Yes | Start point to query metrics (ISO 8601 format) | `2022-07-21T17:32:28Z` |
| `to` | string (date-time) | Yes | End point to query metrics (ISO 8601 format) | `2022-07-21T17:32:28Z` |
| `service` | string | Yes | Service name to filter by. Repeat parameter for multiple services | `ecs` |
| `environment` | string | Yes | Monitoring environment to use as a filter | `eu-de` |
| `max_data_points` | integer | No | Maximum number of datapoints per service (default: 100, max: 1024) | `100` |

### Responses

#### 200 OK

**Content-Type:** `application/json`

Returns metrics matching the query.

**Response Schema: ServiceData**

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Service name |
| `category` | string | Service category |
| `environment` | string | Service environment |
| `metrics` | array | Service metrics per region |
| `metrics[].region` | string | Region name |
| `metrics[].datapoints` | array | Array of datapoint arrays |

**Datapoint Format:**

Each datapoint is an array of exactly 2 elements:
- `[0]` - Unix timestamp
- `[1]` - Value (0-2):
  - `0` - Service running without issues
  - `1` - Service degradation
  - `2` - Service outage

**Example Response:**

```json
{
  "name": "ecs",
  "category": "compute",
  "environment": "eu-de",
  "metrics": [
    {
      "region": "eu-de-01",
      "datapoints": [
        [1450754160, 0],
        [1450754170, 1],
        [1450754180, 2]
      ]
    }
  ]
}
```

#### 404 Not Found

Returned when the requested service or environment is not found.

---

## GET /v1/maintenances

Retrieves planned maintenance windows for services.

### Description

Get list of service maintenances. Returns maintenance windows that overlap with the specified time range.

### Request

**Method:** `GET`

**URL:** `/v1/maintenances`

**Operation ID:** `listMaintenances`

**Tags:** `maintenances`

### Query Parameters

| Parameter | Type | Required | Description | Example |
|-----------|------|----------|-------------|---------|
| `from` | string (date-time) | No | Start point for querying maintenance windows (ISO 8601 format) | `2022-07-21T17:32:28Z` |
| `to` | string (date-time) | No | End point for querying maintenance windows (ISO 8601 format) | `2022-07-21T17:32:28Z` |
| `service` | string | No | Service name filter. Repeat for multiple services. Omit to return all services | `ecs` |

### Responses

#### 200 OK

**Content-Type:** `application/json`

Returns maintenance windows matching the query.

**Response Schema: Array of MaintenanceWindowData**

| Field | Type | Description |
|-------|------|-------------|
| `service` | string | Service name |
| `region` | string | Region name |
| `start` | string (date-time) | Maintenance window start datetime (ISO 8601) |
| `end` | string (date-time) | Maintenance window end datetime (ISO 8601) |
| `reason` | string | Optional reason or description of the planned maintenance |

**Example Response:**

```json
[
  {
    "service": "ecs",
    "region": "eu-de",
    "start": "2022-01-02T12:00:00Z",
    "end": "2022-01-02T13:00:00Z",
    "reason": "Service upgrade"
  }
]
```

---

## Status Codes Summary

| Code | Description |
|------|-------------|
| `200` | Successful request |
| `404` | Resource not found |

---

## Date-Time Format

All date-time parameters use the ISO 8601 format: `YYYY-MM-DDTHH:mm:ssZ`

Example: `2022-07-21T17:32:28Z`

The API also supports relative time expressions for internal usage:
- `-5min` - 5 minutes ago
- `-2min` - 2 minutes ago
