## Telemetry

- Story: As an analyst, I can export completed trips, all trips (including
  in-progress and cancelled), snapshot counts, and agent positions to Parquet
  for external analysis.
  Status: Done

- Story: As an analyst, I can track basic KPIs (time to match, time to pickup,
  trip duration) from recorded timestamps.
  Status: Done

- Story: As an analyst, I can validate trip timestamp ordering (funnel validation)
  to ensure data quality.
  Status: Done

- Story: As a simulation engine, I can capture periodic snapshots of simulation
  state (riders, drivers, trips with positions and states) at configurable intervals,
  storing them in a rolling buffer for visualization and export. Snapshots include
  driver earnings and fatigue data when available.
  Status: Done

- Story: As an analyst, I can compute driver earnings inequality (Gini) and
  churn rates.
  Status: Backlog
