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
  oidc_issuer: "https://zitadel.example.com"
  oidc_key_file: "/etc/cloudmon/service-account.json"

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

Configures the URL and the Zitadel OIDC service identity used for communication with the status dashboard.

```yaml
status_dashboard:
  url: "https://status-dashboard.example.com"
  oidc_issuer: "https://zitadel.example.com"
  oidc_key_file: "/etc/cloudmon/service-account.json"
  oidc_scopes:
    - "urn:zitadel:iam:org:project:role:sd_reporters"
    - "urn:zitadel:iam:org:project:id:<projectId>:aud"
```

| Property       | Type     | Required | Default                                             | Description                                    |
|----------------|----------|----------|-----------------------------------------------------|------------------------------------------------|
| `url`          | string   | Yes      | -                                                   | Status Dashboard API URL                       |
| `oidc_issuer`  | string   | Yes      | -                                                   | Zitadel issuer URL                             |
| `oidc_key_file` | string  | Yes      | -                                                   | Path to the Zitadel machine user key file       |
| `oidc_scopes`  | string[] | Yes      | -                                                   | Requested token scopes                         |

The key file is the JSON key file downloaded from the Zitadel Console for a machine user
(`type: serviceaccount`); the reporter delegates the JWT Profile exchange (assertion signing and
token request) to the `zitadel` crate instead of authenticating with a client secret. The OIDC
token endpoint is discovered from `{oidc_issuer}/.well-known/openid-configuration`.

`oidc_scopes` has no default and has to list both a project role scope
(`urn:zitadel:iam:org:project:role:<role>`) and the audience scope of the Zitadel project shared
with the Status Dashboard (`urn:zitadel:iam:org:project:id:<projectId>:aud`, where `<projectId>` is
the value the Status Dashboard uses for `SD_OIDC_CLIENT_ID`). The reporter rejects a list that
misses either scope at startup, because Zitadel only reports the project roles claim when the
audience scope is requested and the Status Dashboard rejects a token whose audience is the client
id.

## health_query

Configures the time window for health metric queries.

```yaml
health_query:
  query_from: "-5min"
  query_to: "-2min"
```

| Property     | Type   | Required | Default | Description                                                         |
|--------------|--------|----------|---------|---------------------------------------------------------------------|
| `query_from` | string | No       | `-5min` | Start time offset for health metric queries (e.g., "-10min", "-1h") |
| `query_to`   | string | No       | `-2min` | End time offset for health metric queries (e.g., "-1min", "-30s")   |

## flag_metrics

Configures the flag metrics for the components and environments

## environments

Configures environment names and optional attributes (used once alerting the status dashboard component)

## health_metrics

Configures health metrics for components.
