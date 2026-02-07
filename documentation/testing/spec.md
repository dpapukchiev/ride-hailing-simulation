# Testing

## Unit Tests

Unit tests exist in each module to confirm behavior:

- `spatial`: grid disk neighbors within K.
- `clock`: events pop in time order; `schedule_in_secs` / `schedule_in_mins` and sim-to-real conversion.
- `request_inbound`: rider transitions to `Browsing` and sets `requested_at`.
- `show_quote`: rider in Browsing gets quote (fare + ETA) and QuoteDecision scheduled.
- `quote_decision`: rider accepts or rejects quote (stochastic); QuoteAccepted or QuoteRejected scheduled.
- `quote_accepted`: rider transitions to `Waiting` and schedules `TryMatch`.
- `quote_rejected`: rider quote_rejections incremented; re-quote or give-up (despawn, telemetry).
- `matching`: targeted match attempt and transition using configurable matching algorithm.
- `match_accepted`: driver decision scheduled.
- `driver_decision`: driver accept/reject decision.
- `rider_cancel`: rider cancels when pickup wait expires.
- `movement`: driver moves toward rider and schedules trip start; `eta_ms` scales with distance.
- `trip_started`: trip start transitions and completion scheduling.
- `trip_completed`: rider/driver transition after completion; earnings calculation and OffDuty threshold checks.
- `driver_offduty`: periodic earnings/fatigue threshold checks and OffDuty transitions.
- `telemetry_export`: timestamp ordering validation for all trip states (EnRoute, OnTrip, Completed, Cancelled); integration test validates all trips in snapshots follow funnel order.
- **End-to-end (single ride)**: Inserts `SimulationClock`, `SimTelemetry`, spawners configured to spawn one rider and one driver in the same cell. Calls `initialize_simulation()` to schedule `SimulationStarted` at time 0. Runs `run_until_empty` with `simulation_schedule()`.
  Asserts: one `Trip` in `Completed` with correct rider/driver and pickup/dropoff;
  rider `Completed`, driver `Idle` or `OffDuty` (if thresholds exceeded); `SimTelemetry.completed_trips.len() == 1`, record
  matches rider/driver, and KPI timestamps are ordered (requested_at <= matched_at <= pickup_at <= completed_at); `time_to_match()`, `time_to_pickup()`, `trip_duration()` are consistent.
- **End-to-end (concurrent trips)**: Same setup with spawners configured for two riders and two drivers
  (same cell), riders spawning at t=1s and t=2s. Calls `initialize_simulation()` and runs until empty. Asserts: two
  `Trip` entities in `Completed`, both riders `Completed`, both drivers `Idle` or `OffDuty` (if thresholds exceeded);
  `SimTelemetry.completed_trips.len() == 2`.
- **Scenario**: `build_scenario` with 10 riders, 3 drivers, seed 42; asserts
  spawner configurations are correct (max_count matches params). Large scenarios (e.g.
  500 riders, 100 drivers) are only in the example, not in automated tests.

All per-system unit tests emulate the runner by popping one event, inserting
`CurrentEvent`, then running the ECS schedule.

## Benchmarks

Performance benchmarks are located in `crates/sim_core/benches/` using Criterion.rs:

- **`performance.rs`**: Benchmark suite with two groups:
  - `simulation_run`: Full simulation runs for small/medium/large scenarios (50/200/500 drivers, 100/500/1000 riders)
  - `matching_algorithms`: Matching algorithm performance comparison (Simple, Cost-based, Hungarian)
- **Baseline storage**: Criterion.rs automatically stores baseline data in `target/criterion/` (git-ignored). Each run replaces the previous baseline, so comparisons are always against the most recent run. Use named baselines (`--save-baseline`/`--baseline`) to compare against specific earlier versions.
- **HTML reports**: Generated in `target/criterion/<benchmark_name>/report/index.html` for detailed performance analysis.

Run benchmarks with `cargo bench --package sim_core`. See `crates/sim_core/benches/README.md` for details.

## Load Tests

Load tests are located in `crates/sim_core/tests/load_tests.rs`:

- **`test_sustained_load`**: Validates performance under sustained load (500 drivers, 1000 riders, 1 hour simulation). Requires >1000 events/sec.
- **`test_peak_load`**: Tests sudden demand spikes (200 drivers, 1000 riders in 1 hour). Requires >500 events/sec.
- **`test_long_running`**: Long-running stability test (200 drivers, 500 riders, 24-hour simulation). Tests for memory leaks and consistent performance. Requires >500 events/sec.

Load tests are marked with `#[ignore]` and must be run explicitly: `cargo test --package sim_core --test load_tests -- --ignored`.
