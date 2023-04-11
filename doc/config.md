# Configuration

All components of the cloudmon-metrics processor are sharing the single configuration file.

Example:

```
datasource:
  url: https://graphite.example.com
  type: graphite

server:
  address: 192.168.1.14
  port: 3005

metric_templates:
  api_success_rate_low:
    query: "asPercent(sumSeries(stats.counters.openstack.api.$environment.*.$service.*.*.{2*,3*,404}.count), sumSeries(stats.counters.openstack.api.$environment.*.$service.*.*.attempted.count))"
    op: "lt"
    threshold: 90
  api_down:
    query: "asPercent(sumSeries(stats.counters.openstack.api.$environment.*.$service.*.*.failed.count), sumSeries(stats.counters.openstack.api.$environment.*.$service.*.*.attempted.count))"
    op: "eq"
    threshold: 100
  api_slow:
    query: "consolidateBy(aggregate(stats.timers.openstack.api.$environment.*.$service.*.*.*.mean, 'average'), 'avgerage')"
    op: "gt"
    threshold: 300

environments:
  - name: "production"
    attributes:
      region: "Region1"

status_dashboard:
  url: "https://status.cloudmon.com"
  secret: "dev"

flag_metrics:
  ### Comp1
  - name: "api_down"
    service: "comp1"
    template:
      name: "api_down"
    environments:
      - name: "production"
  - name: "api_slow"
    service: "comp1"
    template:
      name: "api_slow"
    environments:
      - name: "production"
  - name: "api_success_rate_low"
    service: "comp1"
    template:
      name: "api_success_rate_low"
    environments:
      - name: "production"

health_metrics:
  ## Compute
  ### DEH
  comp1:
    service: comp1
    component_name: "Component 1 name"
    category: category1
    metrics:
      - comp1.api_down
      - comp1.api_slow
      - comp1.api_success_rate_low
    expressions:
      - expression: "comp1.api_slow || comp1.api_success_rate_low"
        weight: 1
      - expression: "comp1.api_down"
        weight: 2
```

## datasource

datasource section describes url and type of the TSDB that stores the raw metrics

## server

Server section describes address and port to bind to

## metric_templates

This section is providing capability to describe query templates to be later referred by the individual flag metrics

## status_dashboard

Configures URL and jwt secret for communication with the status dashboard

## flag_metrics

Configures the flag metrics for the components and environments

## environments

Configures environment names and optional attributes (used once alerting the status dashboard component)

## health_metrics

Configures health metrics for components.
