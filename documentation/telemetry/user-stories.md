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

- Story: As an analyst, I can track surge impact (additional cost due to surge
  pricing) for completed trips, enabling analysis of surge pricing effects on
  rider costs.
  Status: Done (surge_impact calculated as fare - base_fare in CompletedTripRecord; tracked in telemetry and displayed in UI)

- Story: As an analyst, I can track cumulative platform revenue and total fares
  collected across all completed trips, enabling revenue analysis.
  Status: Done (platform_revenue_total and total_fares_collected accumulated in SimTelemetry; displayed in UI)

- Story: As an analyst, I can track abandonment reasons with breakdowns
  (price too high, ETA too long, stochastic rejection) for riders who give up
  after rejecting quotes, enabling analysis of conversion barriers.
  Status: Done (RiderAbandonmentReason enum tracks reasons; breakdown counters in SimTelemetry: riders_abandoned_price, riders_abandoned_eta, riders_abandoned_stochastic; displayed in UI)

- Story: As an analyst, I can track rider cancellation breakdowns (pickup
  timeout vs. other reasons) to understand why riders cancel during the waiting
  phase.
  Status: Done (riders_cancelled_pickup_timeout counter tracks timeout cancellations; displayed separately from other cancellations in UI)

- Story: As an analyst, I can compute conversion rates (completed trips / total
  resolved riders) to measure marketplace efficiency.
  Status: Done (conversion rate calculated as completed / (completed + cancelled + abandoned); displayed in UI run outcomes)

- Story: As an analyst, I can compute driver earnings inequality (Gini) and
  churn rates.
  Status: Backlog
