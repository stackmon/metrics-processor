# Metrics processor

When monitoring cloud it is not unusual to
have a variety of metrics types (latencies,
status codes, rates). Visualizing overall
state of the service based on those metrics
is not an easy task in this case. It is
desired to have something like a semaphore to
visualize overall "health" of the component
(green - up and running, yellow - there are
some issues, red - complete outage).
Depending on the used TSDB there might be no
way to do this at all.

metrics-processor is there to address 2 primary needs:

- convert series of raw metrics of different
  types into single semaphore-like metric
- inform status dashboard once certain
  component status is not healthy.


## Project structure

- src - rust source code
- doc - mdbook sources for the documentation
- docs - mdbook rendered content (built from `doc`) - temporarily, in future
  documentation will be reworked.
