# CloudMon metrics convertor

cloudmon-metrics-convertor is a component
that is evaluating health state of the
component based on series of metrics in TSDB.
This is done in few steps.

## Flag metrics

First step of analysing component health is to process
raw TSDB metrics and convert them to flags (raised,
lowered). For this a component receives configuration
with set of flag metrics with attached TSDB query and
additional logic applied to the query result (i.e.
`aggregate(stats.counters.api.*.mean)`, `gt`, `1000` -
raise a flag once mean value is greater then 1000 ms).

## Health metrics

Knowing state of flags tied to the certain aspects of
the component it is possible to evaluate overall
component health state applying boolean logic to the
flags and emit an integer number representing the
resulting state (i.e. `api_slow || success_rate_low` =>
1; `api_down || error_rate_at_100` => 2).

## API

convertor component provides an API that is emiting component health at the requested timeframe according to the configuration.
