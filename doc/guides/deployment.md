# Deployment Guide

This guide covers deployment patterns and operational considerations for the metrics-processor.

## Overview

The metrics-processor consists of two binaries:
- **cloudmon-metrics-convertor** - API server that converts raw TSDB metrics to health indicators
- **cloudmon-metrics-reporter** - Background service that reports component status to status dashboard

Both share the same configuration file.

---

## Docker Container Deployment

### Building the Image

```bash
# Build the multi-stage Docker image
docker build -t metrics-processor:latest .

# The image includes both binaries
# - /cloudmon/cloudmon-metrics-convertor
# - /cloudmon/cloudmon-metrics-reporter
```

### Running the Convertor

```bash
# Create configuration directory
mkdir -p config

# Create your config.yaml (see configuration docs)
cat > config/config.yaml << 'EOF'
datasource:
  url: https://graphite.example.com
  timeout: 10

server:
  address: 0.0.0.0
  port: 3000

environments:
  - name: production
    attributes:
      region: eu-de

metric_templates:
  api_down:
    query: "stats.counters.api.$environment.*.$service.*.failed.count"
    op: eq
    threshold: 100

flag_metrics:
  - name: api_down
    service: compute
    template:
      name: api_down
    environments:
      - name: production

health_metrics:
  compute:
    service: compute
    component_name: "Compute Service"
    category: compute
    metrics:
      - compute.api_down
    expressions:
      - expression: "compute.api_down"
        weight: 2
EOF

# Run the convertor
docker run -d \
  --name metrics-convertor \
  -p 3000:3000 \
  -v $(pwd)/config:/cloudmon/config:ro \
  -e RUST_LOG=info \
  metrics-processor:latest \
  /cloudmon/cloudmon-metrics-convertor

# Verify it's running
curl http://localhost:3000/api/v1/info
```

### Running the Reporter

```bash
# Reporter requires convertor to be running and accessible
docker run -d \
  --name metrics-reporter \
  --network host \
  -v $(pwd)/config:/cloudmon/config:ro \
  -e RUST_LOG=info \
  -e MP_STATUS_DASHBOARD__SECRET=your-jwt-secret \
  metrics-processor:latest \
  /cloudmon/cloudmon-metrics-reporter
```

### Docker Compose

```yaml
version: '3.8'

services:
  convertor:
    image: metrics-processor:latest
    container_name: metrics-convertor
    command: /cloudmon/cloudmon-metrics-convertor
    ports:
      - "3000:3000"
    volumes:
      - ./config:/cloudmon/config:ro
    environment:
      - RUST_LOG=info
    healthcheck:
      test: ["CMD", "wget", "-q", "--spider", "http://localhost:3000/api/v1/info"]
      interval: 30s
      timeout: 5s
      retries: 3
    restart: unless-stopped

  reporter:
    image: metrics-processor:latest
    container_name: metrics-reporter
    command: /cloudmon/cloudmon-metrics-reporter
    volumes:
      - ./config:/cloudmon/config:ro
    environment:
      - RUST_LOG=info
      - MP_STATUS_DASHBOARD__SECRET=${STATUS_DASHBOARD_SECRET}
    depends_on:
      convertor:
        condition: service_healthy
    network_mode: "service:convertor"
    restart: unless-stopped
```

---

## Kubernetes Deployment

### ConfigMap for Configuration

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: metrics-processor-config
  namespace: monitoring
data:
  config.yaml: |
    datasource:
      url: https://graphite.example.com
      timeout: 15

    server:
      address: 0.0.0.0
      port: 3000

    environments:
      - name: production
        attributes:
          region: eu-de
      - name: staging
        attributes:
          region: eu-de

    metric_templates:
      api_success_rate_low:
        query: "asPercent(sumSeries(stats.counters.api.$environment.*.$service.*.{2*,3*}.count), sumSeries(stats.counters.api.$environment.*.$service.*.attempted.count))"
        op: lt
        threshold: 90
      api_down:
        query: "asPercent(sumSeries(stats.counters.api.$environment.*.$service.*.failed.count), sumSeries(stats.counters.api.$environment.*.$service.*.attempted.count))"
        op: eq
        threshold: 100

    flag_metrics:
      - name: api_down
        service: compute
        template:
          name: api_down
        environments:
          - name: production
          - name: staging
      - name: api_success_rate_low
        service: compute
        template:
          name: api_success_rate_low
        environments:
          - name: production

    health_metrics:
      compute:
        service: compute
        component_name: "Compute Service"
        category: compute
        metrics:
          - compute.api_down
          - compute.api_success_rate_low
        expressions:
          - expression: "compute.api_success_rate_low"
            weight: 1
          - expression: "compute.api_down"
            weight: 2

    status_dashboard:
      url: https://status.cloudmon.com
```

### Secret for Sensitive Data

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: metrics-processor-secrets
  namespace: monitoring
type: Opaque
stringData:
  status-dashboard-secret: "your-jwt-secret-here"
```

### Convertor Deployment

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: metrics-convertor
  namespace: monitoring
  labels:
    app: metrics-processor
    component: convertor
spec:
  replicas: 2
  selector:
    matchLabels:
      app: metrics-processor
      component: convertor
  template:
    metadata:
      labels:
        app: metrics-processor
        component: convertor
      annotations:
        prometheus.io/scrape: "true"
        prometheus.io/port: "3000"
    spec:
      securityContext:
        runAsUser: 10001
        runAsGroup: 10001
        fsGroup: 10001
      containers:
        - name: convertor
          image: metrics-processor:latest
          command: ["/cloudmon/cloudmon-metrics-convertor"]
          ports:
            - containerPort: 3000
              name: http
          env:
            - name: RUST_LOG
              value: "info"
          volumeMounts:
            - name: config
              mountPath: /cloudmon/config.yaml
              subPath: config.yaml
              readOnly: true
          resources:
            requests:
              memory: "64Mi"
              cpu: "50m"
            limits:
              memory: "256Mi"
              cpu: "500m"
          livenessProbe:
            httpGet:
              path: /api/v1/info
              port: 3000
            initialDelaySeconds: 5
            periodSeconds: 10
          readinessProbe:
            httpGet:
              path: /api/v1/info
              port: 3000
            initialDelaySeconds: 5
            periodSeconds: 5
      volumes:
        - name: config
          configMap:
            name: metrics-processor-config
---
apiVersion: v1
kind: Service
metadata:
  name: metrics-convertor
  namespace: monitoring
spec:
  selector:
    app: metrics-processor
    component: convertor
  ports:
    - port: 3000
      targetPort: 3000
      name: http
  type: ClusterIP
```

### Reporter Deployment

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: metrics-reporter
  namespace: monitoring
  labels:
    app: metrics-processor
    component: reporter
spec:
  replicas: 1  # Only one instance needed
  selector:
    matchLabels:
      app: metrics-processor
      component: reporter
  template:
    metadata:
      labels:
        app: metrics-processor
        component: reporter
    spec:
      securityContext:
        runAsUser: 10001
        runAsGroup: 10001
        fsGroup: 10001
      containers:
        - name: reporter
          image: metrics-processor:latest
          command: ["/cloudmon/cloudmon-metrics-reporter"]
          env:
            - name: RUST_LOG
              value: "info"
            - name: MP_STATUS_DASHBOARD__SECRET
              valueFrom:
                secretKeyRef:
                  name: metrics-processor-secrets
                  key: status-dashboard-secret
          volumeMounts:
            - name: config
              mountPath: /cloudmon/config.yaml
              subPath: config.yaml
              readOnly: true
          resources:
            requests:
              memory: "32Mi"
              cpu: "10m"
            limits:
              memory: "128Mi"
              cpu: "100m"
      volumes:
        - name: config
          configMap:
            name: metrics-processor-config
```

### Ingress Configuration

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: metrics-processor
  namespace: monitoring
  annotations:
    nginx.ingress.kubernetes.io/proxy-read-timeout: "60"
    nginx.ingress.kubernetes.io/proxy-send-timeout: "60"
spec:
  ingressClassName: nginx
  rules:
    - host: metrics.example.com
      http:
        paths:
          - path: /
            pathType: Prefix
            backend:
              service:
                name: metrics-convertor
                port:
                  number: 3000
```

---

## Configuration Management Strategies

### Split Configuration with conf.d

The metrics-processor supports splitting configuration across multiple files:

```
config/
├── config.yaml          # Main configuration
└── conf.d/
    ├── compute.yaml     # Compute service metrics
    ├── network.yaml     # Network service metrics
    └── storage.yaml     # Storage service metrics
```

**Main config.yaml:**
```yaml
datasource:
  url: https://graphite.example.com
  timeout: 15

server:
  address: 0.0.0.0
  port: 3000

environments:
  - name: production
  - name: staging

metric_templates:
  api_down:
    query: "stats.counters.api.$environment.*.$service.*.failed.count"
    op: eq
    threshold: 100

# Empty - populated from conf.d
flag_metrics: []
health_metrics: {}
```

**conf.d/compute.yaml:**
```yaml
flag_metrics:
  - name: api_down
    service: compute
    template:
      name: api_down
    environments:
      - name: production

health_metrics:
  compute:
    service: compute
    component_name: "Compute Service"
    category: compute
    metrics:
      - compute.api_down
    expressions:
      - expression: "compute.api_down"
        weight: 2
```

### Environment Variable Overrides

Override configuration values using environment variables prefixed with `MP_`:

```bash
# Override status dashboard secret
export MP_STATUS_DASHBOARD__SECRET=production-secret

# Override datasource URL
export MP_DATASOURCE__URL=https://graphite-prod.example.com

# Override server port
export MP_SERVER__PORT=8080
```

Use `__` (double underscore) to separate nested keys.

### GitOps with Kustomize

```yaml
# kustomization.yaml
apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization

namespace: monitoring

resources:
  - deployment.yaml
  - service.yaml
  - configmap.yaml

configMapGenerator:
  - name: metrics-processor-config
    files:
      - config.yaml

secretGenerator:
  - name: metrics-processor-secrets
    literals:
      - status-dashboard-secret=your-secret

images:
  - name: metrics-processor
    newTag: v1.2.3
```

---

## Monitoring and Logging Setup

### Structured Logging

The application uses `tracing` for structured logging. Configure log levels:

```yaml
# Kubernetes env
env:
  - name: RUST_LOG
    value: "info,cloudmon_metrics::graphite=debug"
```

Log level options:
- `error` - Only errors
- `warn` - Warnings and errors
- `info` - Informational messages (default)
- `debug` - Debug information
- `trace` - Very verbose tracing

### Log Aggregation

Configure your log collector to parse JSON logs:

```yaml
# Fluent Bit config example
[FILTER]
    Name         parser
    Match        kube.monitoring.metrics-*
    Key_Name     log
    Parser       docker
    Preserve_Key On
    Reserve_Data On
```

### Prometheus Metrics

While the application doesn't expose native Prometheus metrics, you can monitor:

1. **HTTP metrics via sidecar:**
   ```yaml
   - name: nginx-exporter
     image: nginx/nginx-prometheus-exporter
     args:
       - -nginx.scrape-uri=http://localhost:3000/stub_status
   ```

2. **External monitoring:**
   ```yaml
   # Prometheus blackbox exporter probe
   - job_name: 'metrics-processor'
     metrics_path: /probe
     params:
       module: [http_2xx]
     static_configs:
       - targets:
         - http://metrics-convertor:3000/api/v1/info
   ```

### Health Checks

```bash
# Liveness check - server is running
curl -f http://localhost:3000/api/v1/info

# Readiness check - can process requests
curl -f "http://localhost:3000/api/v1/health?service=compute&environment=production&from=-5min&to=now"
```

---

## High Availability Patterns

### Convertor HA

The convertor is stateless and can be horizontally scaled:

```yaml
spec:
  replicas: 3
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxSurge: 1
      maxUnavailable: 0
```

### Load Balancing

```yaml
apiVersion: v1
kind: Service
metadata:
  name: metrics-convertor
spec:
  type: ClusterIP
  sessionAffinity: None  # Stateless, no session affinity needed
  ports:
    - port: 3000
```

### Reporter Singleton

The reporter should run as a single instance to avoid duplicate status reports:

```yaml
spec:
  replicas: 1
  strategy:
    type: Recreate  # Ensure only one instance
```

For leader election in multi-replica scenarios, use a sidecar or external coordination.

### Pod Disruption Budget

```yaml
apiVersion: policy/v1
kind: PodDisruptionBudget
metadata:
  name: metrics-convertor-pdb
spec:
  minAvailable: 1
  selector:
    matchLabels:
      app: metrics-processor
      component: convertor
```

### Anti-Affinity

Spread pods across nodes:

```yaml
spec:
  template:
    spec:
      affinity:
        podAntiAffinity:
          preferredDuringSchedulingIgnoredDuringExecution:
            - weight: 100
              podAffinityTerm:
                labelSelector:
                  matchLabels:
                    app: metrics-processor
                topologyKey: kubernetes.io/hostname
```

---

## Backup and Disaster Recovery

### Configuration Backup

1. **Version control:** Store configuration in Git
2. **Kubernetes backup:** Use Velero for cluster backups

```bash
# Backup ConfigMaps and Secrets
kubectl get configmap metrics-processor-config -n monitoring -o yaml > backup/configmap.yaml
kubectl get secret metrics-processor-secrets -n monitoring -o yaml > backup/secret.yaml
```

### State Considerations

The metrics-processor is **stateless**:
- No persistent storage required
- State is derived from configuration and TSDB queries
- Recovery = redeploy with configuration

### Disaster Recovery Checklist

1. **Configuration:**
   - [ ] Configuration files in version control
   - [ ] Secrets in secure vault (HashiCorp Vault, AWS Secrets Manager)

2. **Dependencies:**
   - [ ] Graphite TSDB URL and credentials
   - [ ] Status Dashboard URL and JWT secret

3. **Recovery steps:**
   ```bash
   # 1. Deploy infrastructure (if needed)
   kubectl apply -f namespace.yaml
   
   # 2. Restore secrets
   kubectl apply -f secrets.yaml
   
   # 3. Apply configuration
   kubectl apply -f configmap.yaml
   
   # 4. Deploy application
   kubectl apply -f deployment.yaml
   
   # 5. Verify
   kubectl get pods -n monitoring
   curl http://metrics.example.com/api/v1/info
   ```

### Multi-Region Deployment

For global deployments:

```yaml
# Region A
apiVersion: v1
kind: ConfigMap
metadata:
  name: metrics-processor-config
data:
  config.yaml: |
    datasource:
      url: https://graphite-region-a.example.com
    environments:
      - name: region-a
        attributes:
          region: eu-de
---
# Region B (separate cluster)
apiVersion: v1
kind: ConfigMap
metadata:
  name: metrics-processor-config
data:
  config.yaml: |
    datasource:
      url: https://graphite-region-b.example.com
    environments:
      - name: region-b
        attributes:
          region: na-us
```

---

## Security Considerations

### Network Policies

```yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: metrics-processor
  namespace: monitoring
spec:
  podSelector:
    matchLabels:
      app: metrics-processor
  policyTypes:
    - Ingress
    - Egress
  ingress:
    - from:
        - namespaceSelector:
            matchLabels:
              name: ingress
      ports:
        - protocol: TCP
          port: 3000
  egress:
    - to:
        - ipBlock:
            cidr: 10.0.0.0/8  # Internal network
      ports:
        - protocol: TCP
          port: 443  # Graphite HTTPS
    - to:
        - namespaceSelector: {}
      ports:
        - protocol: TCP
          port: 53  # DNS
        - protocol: UDP
          port: 53
```

### Secret Management

Never store secrets in plain text:

```yaml
# Use external secrets operator
apiVersion: external-secrets.io/v1beta1
kind: ExternalSecret
metadata:
  name: metrics-processor-secrets
spec:
  secretStoreRef:
    name: vault-backend
    kind: ClusterSecretStore
  target:
    name: metrics-processor-secrets
  data:
    - secretKey: status-dashboard-secret
      remoteRef:
        key: secret/metrics-processor
        property: jwt-secret
```

### Container Security

```yaml
spec:
  containers:
    - name: convertor
      securityContext:
        allowPrivilegeEscalation: false
        readOnlyRootFilesystem: true
        runAsNonRoot: true
        capabilities:
          drop:
            - ALL
```

---

## Operational Runbook

### Deployment Checklist

- [ ] Configuration validated with `yamllint`
- [ ] Graphite connectivity verified
- [ ] Status Dashboard connectivity verified (reporter)
- [ ] Resource limits configured
- [ ] Health checks configured
- [ ] Logging configured
- [ ] Secrets deployed securely
- [ ] Network policies applied

### Common Operations

**Scale convertor:**
```bash
kubectl scale deployment metrics-convertor --replicas=5 -n monitoring
```

**Update configuration:**
```bash
kubectl create configmap metrics-processor-config --from-file=config.yaml -o yaml --dry-run=client | kubectl apply -f -
kubectl rollout restart deployment/metrics-convertor -n monitoring
```

**View logs:**
```bash
kubectl logs -f deployment/metrics-convertor -n monitoring
kubectl logs -f deployment/metrics-reporter -n monitoring
```

**Debug pod:**
```bash
kubectl run debug --rm -it --image=curlimages/curl -- sh
curl http://metrics-convertor:3000/api/v1/info
```
