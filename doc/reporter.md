# CloudMon metrics reporter

cloudmon-metrics-reporter component is a glue layer
between raw metrics and the status dashboard. It runs
an endless loop and at a given interval fetches health
metrics from the convertor component and if the state
is greater then 0 issues an API call towards status
dashboard with the current state of the component.
Status dashboard is then responsible for further
incident processing logic (is it necessary to open an
incident or there is an open incident already).
