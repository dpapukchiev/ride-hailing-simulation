# Telemetry

## `sim_core::telemetry`

- **`RiderAbandonmentReason`** enum: `QuotePriceTooHigh`, `QuoteEtaTooLong`, `QuoteStochasticRejection`, `PickupTimeout`. Used to track why riders abandoned their ride requests. Stored in `Rider.last_rejection_reason` when quotes are rejected, and used to increment the appropriate breakdown counter in `SimTelemetry` when riders give up.
- **`SimTelemetry`** (ECS `Resource`, default): holds `completed_trips: Vec<CompletedTripRecord>` plus cumulative rider totals (`riders_cancelled_total`, `riders_completed_total`, `riders_abandoned_quote_total`), breakdown fields for abandonment reasons (`riders_abandoned_price`, `riders_abandoned_eta`, `riders_abandoned_stochastic`, `riders_cancelled_pickup_timeout`), `platform_revenue_total: f64`, and `total_fares_collected: f64`. `riders_abandoned_quote_total` counts riders who gave up after rejecting too many quotes (distinct from pickup-timeout cancels), with breakdown by reason: `riders_abandoned_price` (rejected due to price too high), `riders_abandoned_eta` (rejected due to ETA too long), `riders_abandoned_stochastic` (stochastic rejection). `riders_cancelled_pickup_timeout` counts riders who cancelled while waiting for pickup. `platform_revenue_total` accumulates commission revenue from completed trips. `total_fares_collected` is the sum of agreed fares for completed trips.
- **`CompletedTripRecord`**: `{ trip_entity, rider_entity, driver_entity, completed_at, requested_at, matched_at, pickup_at, fare, surge_impact }` (timestamps in **simulation ms**, `fare` is agreed fare paid, `surge_impact` is additional cost due to surge pricing calculated as `fare - base_fare`). Helper methods: **`time_to_match()`**, **`time_to_pickup()`**, **`trip_duration()`** (all in ms).
- Insert `SimTelemetry::default()` when building the world to record completed trips; `trip_completed_system` pushes one record per completed trip with timestamps from the Trip and clock, calculates `surge_impact` by comparing the agreed fare to the base fare (recalculated using current pricing config), and accumulates platform revenue.
- **`PricingConfig`** (ECS `Resource`): `{ base_fare, per_km_rate, commission_rate, surge_enabled, surge_radius_k, surge_max_multiplier }` controls pricing and optional surge. Inserted by `build_scenario` (from `ScenarioParams.pricing_config` or default). Required by `show_quote_system` and `trip_completed_system`.
- **`SimSnapshotConfig`** (ECS `Resource`): `{ interval_ms, max_snapshots }` controls snapshot cadence and buffer size.
- **`SimSnapshots`** (ECS `Resource`): rolling `VecDeque<SimSnapshot>` plus `last_snapshot_at`; populated by the snapshot system.
- **`SimSnapshot`**: `{ timestamp_ms, counts, riders, drivers, trips }` with state-aware position snapshots plus trip state snapshots for visualization/export; counts include cumulative rider totals (including `riders_abandoned_quote_total`) to account for despawns.
- **`RiderSnapshot`**: `{ entity, cell, state, matched_driver: Option<Entity> }` captures rider state and position; `matched_driver` is `Some(driver_entity)` when a driver is matched (rider is waiting for pickup) and `None` when waiting for match.
- **`DriverSnapshot`**: `{ entity, cell, state, daily_earnings: Option<f64>, daily_earnings_target: Option<f64>, session_start_time_ms: Option<u64>, session_end_time_ms: Option<u64>, fatigue_threshold_ms: Option<u64> }` captures driver state, position, and earnings/fatigue data (if available) for visualization/export. `session_end_time_ms` is set when the driver goes OffDuty and `None` while active.

## `sim_core::telemetry_export`

- Parquet export helpers for analytics:
  - `write_completed_trips_parquet(path, telemetry)` - exports only completed trips
  - `write_trips_parquet(path, snapshots)` - exports all trips (same data as UI trip table), includes all states with full details
  - `write_snapshot_counts_parquet(path, snapshots)` - time-series counts
  - `write_agent_positions_parquet(path, snapshots)` - position snapshots for riders and drivers
- **`validate_trip_timestamp_ordering(trip)`**: Validates that timestamps in a `TripSnapshot` follow the funnel order:
  - **EnRoute**: `requested_at ≤ matched_at`, no pickup/dropoff/cancelled timestamps
  - **OnTrip**: `requested_at ≤ matched_at ≤ pickup_at`, no dropoff/cancelled timestamps
  - **Completed**: `requested_at ≤ matched_at ≤ pickup_at ≤ dropoff_at`, no cancelled timestamp
  - **Cancelled**: `requested_at ≤ matched_at ≤ cancelled_at` (and `pickup_at ≤ cancelled_at` if pickup exists), no dropoff timestamp
  Returns `Option<String>` with error message if validation fails, `None` if valid.

## `sim_core::systems::telemetry_snapshot`

System: `capture_snapshot_system`

- Runs conditionally after each event (via schedule condition) and captures a snapshot when `interval_ms` has elapsed.
- Records rider/driver positions and state counts into `SimSnapshots` (rolling buffer).
- For drivers, includes earnings and fatigue data if available:
  - `daily_earnings`, `daily_earnings_target`, `session_start_time_ms`, `session_end_time_ms` from `DriverEarnings` component
  - `fatigue_threshold_ms` from `DriverFatigue` component
- Optimized to collect riders, drivers, and trips in single passes (removed double iteration).
