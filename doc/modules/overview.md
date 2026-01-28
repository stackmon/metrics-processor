# Module Overview

This document provides a high-level overview of the metrics-processor crate modules, their responsibilities, and relationships.

## Module Responsibility Matrix

| Module | Primary Responsibility | Key Types | Dependencies | Used By |
|--------|----------------------|-----------|--------------|---------|
| `lib` | Crate entry point | - | `api`, `common`, `config`, `graphite`, `sd`, `types` | External consumers |
| `api` | HTTP API routing | - | `api::v1` | `main` binary |
| `api::v1` | V1 REST endpoints | `HealthQuery`, `ServiceHealthResponse` | `common`, `types` | `api` |
| `config` | Configuration parsing | `Config`, `Datasource`, `ServerConf` | `types` | `types`, `main` |
| `types` | Core data structures | `AppState`, `FlagMetric`, `ServiceHealthDef` | `config` | All modules |
| `graphite` | Graphite TSDB interface | `GraphiteData`, `Metric`, `RenderRequest` | `common`, `types` | `common`, `api::v1` |
| `common` | Shared utilities | - | `types`, `graphite` | `api::v1`, `graphite` |
| `sd` | Status Dashboard API | `IncidentData`, `ComponentCache`, `StatusDashboardComponent` | `anyhow`, `hmac`, `jwt` | `reporter` binary |

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                    convertor binary                         │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                         config                              │
│              (Config, Datasource, ServerConf)               │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                          types                              │
│    (AppState, FlagMetric, ServiceHealthDef, CloudMonError)  │
└─────────────────────────────────────────────────────────────┘
                              │
          ┌───────────────────┼───────────────────┐
          ▼                   ▼                   ▼
┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
│     api::v1     │  │    graphite     │  │     common      │
│  (REST API v1)  │  │ (TSDB Client)   │  │   (Utilities)   │
└─────────────────┘  └─────────────────┘  └─────────────────┘
          │                   │                   │
          └───────────────────┴───────────────────┘
                              │
                              ▼
                     ┌─────────────────┐
                     │   HTTP Server   │
                     │     (Axum)      │
                     └─────────────────┘

┌─────────────────────────────────────────────────────────────┐
│                     reporter binary                         │
└─────────────────────────────────────────────────────────────┘
                              │
          ┌───────────────────┼───────────────────┐
          ▼                   ▼                   ▼
┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
│      config     │  │       sd        │  │   api::v1       │
│  (Config load)  │  │ (Status Dash)   │  │  (Query health) │
└─────────────────┘  └─────────────────┘  └─────────────────┘
                              │
                              ▼
                     ┌─────────────────┐
                     │ Status Dashboard│
                     │   V2 API        │
                     └─────────────────┘
```

## Module Summaries

### `lib.rs`
Entry point for the crate, re-exports all public modules:
- `api` - HTTP API handlers
- `common` - Shared utilities  
- `config` - Configuration management
- `graphite` - Graphite TSDB communication
- `types` - Core type definitions

### `api` / `api::v1`
Axum-based HTTP handlers for the REST API. Provides health metrics endpoints.

### `config`
YAML configuration loading with environment variable merging. Supports `conf.d` style modular configuration.

### `types`
Core domain types including metric definitions, application state, and error types.

### `graphite`
Graphite TSDB client implementing the render and metrics/find APIs for Grafana compatibility.

### `common`
Shared business logic for metric flag evaluation and service health calculation.

## Data Flow

1. **Startup**: `config::Config::new()` loads YAML + env vars
2. **Initialization**: `types::AppState::new()` builds runtime state with processed metrics
3. **Request Handling**: Axum routes to `api::v1` or `graphite` handlers
4. **Data Retrieval**: Handlers call `common` utilities which query `graphite` module
5. **Response**: Results transformed and returned as JSON
