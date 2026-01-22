# Architecture Diagrams

This document contains visual representations of the CloudMon Metrics Processor architecture using Mermaid diagrams.

## System Architecture

### High-Level Component Diagram

```mermaid
graph TB
    subgraph External["External Systems"]
        TSDB["Graphite<br/>(Time-Series Database)"]
        Dashboard["Status Dashboard<br/>(Atlassian Statuspage)"]
        Grafana["Grafana<br/>(Visualization)"]
    end
    
    subgraph MetricsProcessor["CloudMon Metrics Processor"]
        subgraph Convertor["Convertor Binary"]
            API["HTTP API<br/>(Axum)"]
            GraphiteModule["Graphite Module<br/>(TSDB Client)"]
            Common["Common Module<br/>(Evaluation Logic)"]
            Config["Config Module<br/>(YAML Parser)"]
            Types["Types Module<br/>(Domain Models)"]
        end
        
        subgraph Reporter["Reporter Binary"]
            Poller["Metric Poller<br/>(60s interval)"]
            Notifier["Dashboard Notifier<br/>(JWT Auth)"]
        end
    end
    
    TSDB -->|"Raw Metrics"| GraphiteModule
    GraphiteModule --> Common
    Common --> API
    API -->|"Health Data"| Grafana
    API -->|"Health Data"| Poller
    Poller --> Notifier
    Notifier -->|"Status Updates"| Dashboard
    
    Config --> Types
    Types --> Common
    Types --> GraphiteModule
    
    style Convertor fill:#e1f5fe
    style Reporter fill:#fff3e0
    style External fill:#f5f5f5
```

### Component Interactions

```mermaid
graph LR
    subgraph Inputs["Inputs"]
        ConfigFile["config.yaml"]
        EnvVars["Environment Variables"]
        RawMetrics["Raw TSDB Metrics"]
    end
    
    subgraph Processing["Processing"]
        Parse["Parse Config"]
        Template["Apply Templates"]
        Query["Query TSDB"]
        Evaluate["Evaluate Flags"]
        Combine["Combine to Health"]
    end
    
    subgraph Outputs["Outputs"]
        RESTAPI["REST API Response"]
        GrafanaData["Grafana Datapoints"]
        StatusUpdate["Dashboard Update"]
    end
    
    ConfigFile --> Parse
    EnvVars --> Parse
    Parse --> Template
    Template --> Query
    RawMetrics --> Query
    Query --> Evaluate
    Evaluate --> Combine
    Combine --> RESTAPI
    Combine --> GrafanaData
    RESTAPI --> StatusUpdate
```

## Module Dependency Graph

### Library Crate Dependencies

```mermaid
graph TD
    subgraph lib["cloudmon_metrics (lib)"]
        lib_rs["lib.rs<br/>(re-exports)"]
    end
    
    subgraph modules["Core Modules"]
        api["api.rs<br/>(module declaration)"]
        api_v1["api/v1.rs<br/>(REST handlers)"]
        config["config.rs<br/>(YAML + env parsing)"]
        types["types.rs<br/>(domain types)"]
        graphite["graphite.rs<br/>(TSDB client)"]
        common["common.rs<br/>(shared utils)"]
    end
    
    subgraph binaries["Binaries"]
        convertor["bin/convertor.rs"]
        reporter["bin/reporter.rs"]
    end
    
    lib_rs --> api
    lib_rs --> config
    lib_rs --> types
    lib_rs --> graphite
    lib_rs --> common
    
    api --> api_v1
    api_v1 --> common
    api_v1 --> types
    
    graphite --> common
    graphite --> types
    
    common --> types
    common --> graphite
    
    types --> config
    
    convertor --> lib_rs
    reporter --> lib_rs
    reporter --> api_v1
    
    style lib fill:#e8f5e9
    style modules fill:#e3f2fd
    style binaries fill:#fce4ec
```

### External Crate Dependencies

```mermaid
graph TB
    subgraph Application["Application"]
        Convertor["Convertor"]
        Reporter["Reporter"]
    end
    
    subgraph WebFramework["Web Framework"]
        Axum["axum 0.6"]
        Tower["tower-http"]
        Hyper["hyper"]
    end
    
    subgraph AsyncRuntime["Async Runtime"]
        Tokio["tokio"]
        TokioSignal["tokio::signal"]
    end
    
    subgraph HTTP["HTTP Client"]
        Reqwest["reqwest"]
    end
    
    subgraph Serialization["Serialization"]
        Serde["serde"]
        SerdeJson["serde_json"]
        SerdeYaml["config (yaml)"]
    end
    
    subgraph Evaluation["Expression Evaluation"]
        Evalexpr["evalexpr"]
    end
    
    subgraph Auth["Authentication"]
        JWT["jwt"]
        HMAC["hmac + sha2"]
    end
    
    subgraph Utilities["Utilities"]
        Chrono["chrono"]
        Regex["regex"]
        Uuid["uuid"]
        Tracing["tracing"]
    end
    
    Convertor --> Axum
    Convertor --> Tower
    Convertor --> Tokio
    Convertor --> Reqwest
    Convertor --> Serde
    Convertor --> Evalexpr
    Convertor --> Tracing
    
    Reporter --> Tokio
    Reporter --> Reqwest
    Reporter --> Serde
    Reporter --> JWT
    Reporter --> HMAC
    Reporter --> Tracing
    
    Axum --> Hyper
    Axum --> Tower
    Axum --> Tokio
    
    style Application fill:#bbdefb
    style WebFramework fill:#c8e6c9
    style AsyncRuntime fill:#fff9c4
```

## Deployment Architecture

### Standalone Deployment

```mermaid
graph TB
    subgraph Host["Host / Container"]
        subgraph ConvertorProcess["cloudmon-metrics-convertor"]
            Server["HTTP Server<br/>:3000"]
            ConvConfig["config.yaml"]
        end
        
        subgraph ReporterProcess["cloudmon-metrics-reporter"]
            Poller["Polling Loop"]
            RepConfig["config.yaml"]
        end
    end
    
    subgraph External["External Services"]
        Graphite["Graphite TSDB"]
        StatusDB["Status Dashboard"]
    end
    
    Graphite -->|"HTTP GET /render"| Server
    Server -->|"HTTP GET /api/v1/health"| Poller
    Poller -->|"HTTP POST /v1/component_status"| StatusDB
    
    ConvConfig -.->|"reads"| Server
    RepConfig -.->|"reads"| Poller
    
    style Host fill:#e0f2f1
    style ConvertorProcess fill:#b3e5fc
    style ReporterProcess fill:#ffe0b2
```

### Kubernetes Deployment

```mermaid
graph TB
    subgraph K8s["Kubernetes Cluster"]
        subgraph NSMetrics["Namespace: metrics"]
            subgraph DeployConv["Deployment: convertor"]
                Pod1["Pod<br/>convertor"]
                Pod2["Pod<br/>convertor"]
            end
            
            SvcConv["Service: convertor<br/>ClusterIP:3000"]
            
            subgraph DeployRep["Deployment: reporter"]
                Pod3["Pod<br/>reporter"]
            end
            
            CM["ConfigMap<br/>config.yaml"]
            Secret["Secret<br/>JWT credentials"]
        end
    end
    
    subgraph External["External"]
        Graphite["Graphite TSDB"]
        StatusDB["Status Dashboard"]
        Ingress["Ingress Controller"]
    end
    
    Pod1 --> SvcConv
    Pod2 --> SvcConv
    Pod3 -->|"polls"| SvcConv
    
    Graphite --> Pod1
    Graphite --> Pod2
    Pod3 --> StatusDB
    
    CM -.-> Pod1
    CM -.-> Pod2
    CM -.-> Pod3
    Secret -.-> Pod3
    
    Ingress -.->|"optional"| SvcConv
    
    style K8s fill:#f3e5f5
    style NSMetrics fill:#e1bee7
    style DeployConv fill:#b3e5fc
    style DeployRep fill:#ffe0b2
```

### High Availability Setup

```mermaid
graph TB
    subgraph Region1["Region 1"]
        LB1["Load Balancer"]
        subgraph Convertor1["Convertor Cluster"]
            C1a["Convertor A"]
            C1b["Convertor B"]
        end
        R1["Reporter"]
    end
    
    subgraph Region2["Region 2"]
        LB2["Load Balancer"]
        subgraph Convertor2["Convertor Cluster"]
            C2a["Convertor A"]
            C2b["Convertor B"]
        end
        R2["Reporter"]
    end
    
    subgraph Shared["Shared Services"]
        TSDB["Graphite<br/>(Replicated)"]
        Dashboard["Status Dashboard"]
    end
    
    LB1 --> C1a
    LB1 --> C1b
    R1 --> LB1
    
    LB2 --> C2a
    LB2 --> C2b
    R2 --> LB2
    
    TSDB --> C1a
    TSDB --> C1b
    TSDB --> C2a
    TSDB --> C2b
    
    R1 --> Dashboard
    R2 --> Dashboard
    
    style Region1 fill:#e3f2fd
    style Region2 fill:#fff3e0
    style Shared fill:#e8f5e9
```

## Request Flow Diagrams

### Health API Request Flow

```mermaid
sequenceDiagram
    participant Client
    participant API as API Layer<br/>(api/v1.rs)
    participant Common as Common Module<br/>(common.rs)
    participant Graphite as Graphite Module<br/>(graphite.rs)
    participant TSDB as Graphite TSDB
    
    Client->>API: GET /api/v1/health?service=svc&env=prod
    API->>API: Validate query params
    API->>Common: get_service_health(svc, env, from, to)
    Common->>Common: Load health_metric config
    Common->>Graphite: get_graphite_data(queries)
    Graphite->>TSDB: GET /render?target=...
    TSDB-->>Graphite: JSON datapoints
    Graphite-->>Common: Vec<GraphiteData>
    Common->>Common: Evaluate flag metrics
    Common->>Common: Build expression context
    Common->>Common: Evaluate health expressions
    Common-->>API: ServiceHealthData
    API-->>Client: JSON response
```

### Reporter Notification Flow

```mermaid
sequenceDiagram
    participant Timer as Tokio Timer
    participant Reporter as Reporter
    participant Convertor as Convertor API
    participant Dashboard as Status Dashboard
    
    loop Every 60 seconds
        Timer->>Reporter: tick
        Reporter->>Reporter: For each environment
        Reporter->>Reporter: For each service
        Reporter->>Convertor: GET /api/v1/health
        Convertor-->>Reporter: ServiceHealthResponse
        
        alt Health status > 0 (degraded/outage)
            Reporter->>Dashboard: POST /v1/component_status
            Dashboard-->>Reporter: 200 OK
        end
    end
```

## Data Structure Diagrams

### Configuration Hierarchy

```mermaid
graph TD
    subgraph Config["Config Structure"]
        Root["Config"]
        Root --> DS["datasource"]
        Root --> Server["server"]
        Root --> Templates["metric_templates"]
        Root --> Envs["environments"]
        Root --> Flags["flag_metrics"]
        Root --> Health["health_metrics"]
        Root --> Status["status_dashboard"]
        
        DS --> DS_URL["url: String"]
        DS --> DS_Timeout["timeout: u16"]
        
        Server --> Server_Addr["address: String"]
        Server --> Server_Port["port: u16"]
        
        Templates --> Tmpl["Template"]
        Tmpl --> Tmpl_Query["query: String"]
        Tmpl --> Tmpl_Op["op: CmpType"]
        Tmpl --> Tmpl_Thresh["threshold: f32"]
        
        Envs --> Env["EnvironmentDef"]
        Env --> Env_Name["name: String"]
        Env --> Env_Attrs["attributes: HashMap"]
        
        Flags --> Flag["FlagMetricDef"]
        Flag --> Flag_Name["name: String"]
        Flag --> Flag_Service["service: String"]
        Flag --> Flag_Template["template: TemplateRef"]
        Flag --> Flag_Envs["environments: Vec"]
        
        Health --> HM["ServiceHealthDef"]
        HM --> HM_Service["service: String"]
        HM --> HM_Category["category: String"]
        HM --> HM_Metrics["metrics: Vec<String>"]
        HM --> HM_Exprs["expressions: Vec"]
        
        Status --> Status_URL["url: String"]
        Status --> Status_Secret["secret: Option<String>"]
    end
    
    style Config fill:#e8eaf6
```

### AppState Structure

```mermaid
graph LR
    subgraph AppState["AppState (Runtime)"]
        AS_Config["config: Config"]
        AS_Templates["metric_templates:<br/>HashMap<String, BinaryMetricRawDef>"]
        AS_Client["req_client:<br/>reqwest::Client"]
        AS_Flags["flag_metrics:<br/>HashMap<String, HashMap<String, FlagMetric>>"]
        AS_Health["health_metrics:<br/>HashMap<String, ServiceHealthDef>"]
        AS_Envs["environments:<br/>Vec<EnvironmentDef>"]
        AS_Services["services:<br/>HashSet<String>"]
    end
    
    subgraph Lookup["Lookup Pattern"]
        Query["Query: env=prod, service=api"]
        FlagLookup["flag_metrics.get('api.latency')?.get('prod')"]
        HealthLookup["health_metrics.get('api')"]
    end
    
    Query --> FlagLookup
    Query --> HealthLookup
    FlagLookup --> AS_Flags
    HealthLookup --> AS_Health
    
    style AppState fill:#e0f7fa
    style Lookup fill:#fff8e1
```

## Error Handling Flow

```mermaid
graph TD
    subgraph Errors["CloudMonError Types"]
        E1["ServiceNotSupported"]
        E2["EnvNotSupported"]
        E3["ExpressionError"]
        E4["GraphiteError"]
    end
    
    subgraph Handling["Error Handling"]
        Check1{"Service in config?"}
        Check2{"Env for service?"}
        Check3{"Expression valid?"}
        Check4{"TSDB responds?"}
        
        Check1 -->|No| E1
        Check1 -->|Yes| Check2
        Check2 -->|No| E2
        Check2 -->|Yes| Check4
        Check4 -->|No| E4
        Check4 -->|Yes| Check3
        Check3 -->|No| E3
        Check3 -->|Yes| Success["Return health data"]
    end
    
    subgraph HTTPResponse["HTTP Response Mapping"]
        E1 --> R409a["409 Conflict"]
        E2 --> R409b["409 Conflict"]
        E3 --> R500a["500 Internal Server Error"]
        E4 --> R500b["500 Internal Server Error"]
        Success --> R200["200 OK"]
    end
    
    style Errors fill:#ffebee
    style HTTPResponse fill:#e8f5e9
```

## Related Documentation

- [Architecture Overview](overview.md): Detailed component descriptions
- [Data Flow](data-flow.md): Step-by-step processing documentation
- [API Reference](../api/endpoints.md): HTTP endpoint specifications
