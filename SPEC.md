# Ride-Hailing Simulation Spec (Current State)

This spec documents the code that exists today in this repository. It is the
single source of truth for the current implementation and should be updated
whenever code or spec changes are made.

## Overview

The project is a Rust-based discrete event simulation (DES) scaffold with a
minimal ECS-based agent model. It currently supports:

- A H3-based spatial index wrapper.
- A binary-heap simulation clock with targeted discrete events.
- ECS components for riders and drivers, including pairing links.
- Simple, deterministic systems for request intake, matching, trip completion,
  and rider pickup cancellations.
- A **runner API** that advances the clock and routes events (pop → insert
  `CurrentEvent` → run schedule).
- A **scenario** module that schedules rider requests at random times and spawns
  riders just-in-time when they request, plus spawns drivers continuously over time.
  Configurable match radius, trip duration (min/max H3 cells), and simulation start time.
  Spawners use time-of-day and day-of-week distributions to create realistic demand
  and supply patterns (rush hours, day/night variations).
- **Driver earnings and fatigue tracking**: Drivers accumulate earnings from completed trips
  and transition to `OffDuty` when they reach their daily earnings target or exceed their
  fatigue threshold. OffDuty drivers are excluded from matching.
- **Configurable pricing system**: Distance-based fare calculation (base fare + per-kilometer rate)
  with configurable commission rates. Driver earnings and platform revenue are tracked separately.
- **Rider quote flow**: Riders receive an explicit quote (fare + ETA) before committing; they can
  accept (proceed to matching), reject and request another quote, or give up after a configurable
  number of rejections (tracked in telemetry as abandoned-quote).

This is a "crawl/walk" foundation aligned with the research plan.

## Human-Readable System Flow

The simulation uses a **millisecond-scale timeline**: all timestamps and `clock.now()` are in ms. Time 0 maps to a real-world datetime via `epoch_ms` (e.g. Unix epoch or a fixed start time), so it is easy to convert simulation time ↔ real datetime. Events are scheduled at specific timestamps (`schedule_at`) or at a delta from current time (`schedule_in`). The timeline advances by popping the next scheduled event; when multiple events share the same ms, they are ordered by `EventKind` for determinism.

The system is a discrete-event loop where **clock progression and event routing
happen outside ECS systems**:

- The runner pops the next `Event` from `SimulationClock`, which advances time
  (`clock.now`) to that event’s timestamp.
- The runner inserts that event into the ECS world as a `CurrentEvent` resource.
- The ECS schedule is run, and **systems react to that concrete `CurrentEvent`**
  (and only mutate the targeted rider/driver).
- Systems may schedule follow-up events back onto `SimulationClock`.

Events are **targeted** via an optional subject (e.g. `EventSubject::Rider(Entity)`,
`EventSubject::Driver(Entity)`, or `EventSubject::Trip(Entity)`), which allows
multiple trips to be “in flight” at once without global “scan everything”
transitions.

Once a driver accepts, the simulation creates a dedicated **Trip entity** that
becomes the stable identifier for the rest of the lifecycle (movement, start,
completion).

In the current flow, riders and drivers are spawned dynamically by spawner systems
reacting to `SimulationStarted` and spawn events. Spawners use inter-arrival time
distributions to control spawn rates, enabling variable supply and demand patterns.
The default distributions vary rates based on time of day (hour) and day of week,
creating realistic patterns with rush hours (7-9 AM, 5-7 PM) and lower demand at night.
Riders spawn in `Browsing` state. One second after spawn, `ShowQuote` runs: the sim computes a quote (fare from trip distance, ETA from nearest idle driver or a default), attaches a `RiderQuote` component, and schedules `QuoteDecision` 1s later. On `QuoteDecision`, the rider stochastically accepts or rejects (configurable probability). If the rider **accepts** (`QuoteAccepted`), they transition to `Waiting`, `TryMatch` and pickup-timeout cancellation are scheduled, and matching proceeds as before. If the rider **rejects** (`QuoteRejected`), their quote rejection count increments; if under the configured max, `ShowQuote` is rescheduled after a delay (re-quote); otherwise the rider gives up (marked cancelled, despawned, and counted in `riders_abandoned_quote_total`). Drivers spawn
in `Idle` state continuously over the simulation window with earnings targets ($100-$300)
and fatigue thresholds (8-12 hours), evaluate a match offer, drive en route to pickup, and then move into
an on-trip state. If
a rider waits past a randomized pickup window, the ride is cancelled, the trip
is marked cancelled, and the driver returns to idle. When the trip completes,
the rider is despawned, the driver earns fare based on trip distance, and the driver returns to idle
(or transitions to `OffDuty` if earnings target or fatigue threshold is exceeded). OffDuty drivers
are excluded from matching and remain in that state for the remainder of the simulation. Matching uses a
configurable **match radius** (H3 grid distance): 0 = same cell only; a larger
radius allows matching to idle drivers within that many cells. Trip length is
configurable via min/max H3 cells from pickup to dropoff (movement uses 20–60
km/h city-driving speeds). Throughout this flow, riders and drivers store links
to each other so the pairing is explicit while a trip is in progress. The simulation
start time (epoch) is configurable, allowing scenarios to start at any real-world
datetime, which affects the time-of-day patterns applied to spawn rates.

## Workspace Layout

```text
README.md
Cargo.toml
stories/
  README.md
  core-sim/
  drivers/
  matching/
  pricing/
  riders/
  telemetry/
  ui/
crates/
  sim_core/
    Cargo.toml
    src/
      clock.rs
      ecs.rs
      lib.rs
      runner.rs
      scenario.rs
      spatial.rs
      telemetry.rs
      pricing.rs
      systems/
        mod.rs
        show_quote.rs
        quote_decision.rs
        quote_accepted.rs
        quote_rejected.rs
        matching.rs
        match_accepted.rs
        driver_decision.rs
        movement.rs
        rider_cancel.rs
        trip_started.rs
        trip_completed.rs
        driver_offduty.rs
    examples/
      scenario_run.rs
  sim_ui/
    Cargo.toml
    src/
      main.rs
      app.rs
      ui/
        mod.rs
        controls.rs
        rendering.rs
        utils.rs
        constants.rs
```

## Dependencies

`crates/sim_core/Cargo.toml`:

- `h3o = "0.8"` for H3 spatial indexing (stable toolchain compatible).
- `bevy_ecs = "0.13"` for ECS world, components, and systems.
- `rand = "0.8"` for scenario randomisation (positions, request times, destinations).
- `arrow` + `parquet` for Parquet export of completed trips and snapshots.

`crates/sim_ui/Cargo.toml`:

- `eframe` + `egui_plot` for the native visualization UI.
- `bevy_ecs` + `h3o` for shared types and map projection.

## Tooling

- `mise` is used for toolchain management via `.mise.toml`.
- Rust toolchain: `stable`.
- `README.md` includes setup and run commands.

## Core Modules

### `sim_core::spatial`

- `GeoIndex` stores a fixed H3 resolution.
- `grid_disk(origin, k)` wraps `h3o::CellIndex::grid_disk` and asserts the
  resolution matches the index resolution.
- Default resolution is `Resolution::Nine`.
- `distance_km_between_cells(a, b)` calculates haversine distance between two H3 cells in kilometers.

### `sim_core::pricing`

Configurable pricing system with commission support for marketplace analysis.

- **`BASE_FARE`**: Base fare constant (default $2.50).
- **`PER_KM_RATE`**: Per-kilometer rate (default $1.50/km).
- **`PricingConfig`** (ECS Resource): Configurable pricing parameters:
  - `base_fare: f64` - Base fare in currency units (default: 2.50)
  - `per_km_rate: f64` - Per-kilometer rate (default: 1.50)
  - `commission_rate: f64` - Commission rate as fraction 0.0-1.0 (default: 0.0, meaning no commission)
- **`calculate_trip_fare(pickup, dropoff)`**: Calculates fare using default constants (backward compatibility).
  Formula: `fare = BASE_FARE + (distance_km * PER_KM_RATE)`.
- **`calculate_trip_fare_with_config(pickup, dropoff, config)`**: Calculates fare using provided `PricingConfig`.
- **`calculate_commission(fare, commission_rate)`**: Calculates commission amount (`fare * commission_rate`).
- **`calculate_driver_earnings(fare, commission_rate)`**: Calculates driver net earnings (`fare * (1 - commission_rate)`).
- **`calculate_platform_revenue(fare, commission_rate)`**: Calculates platform revenue (same as commission).

### `sim_core::clock`

All time is in **milliseconds** (simulation ms). Time 0 maps to a real-world datetime via `epoch_ms`.

- **`SimulationClock`** (ECS `Resource`):
  - `now: u64` — current simulation time in ms (updated when an event is popped).
  - `epoch_ms: i64` — real-world ms corresponding to sim time 0 (e.g. from a datetime). Use `with_epoch(epoch_ms)` to set.
  - `set_epoch_ms(epoch_ms)` updates the epoch after construction (used by the UI).
  - `events: BinaryHeap<Event>` — min-heap by timestamp; **same-ms events** are ordered by `EventKind` for determinism.
- **Scheduling** (callers can use ms, seconds, or minutes):
  - **Absolute**: `schedule_at(at_ms, ...)`, `schedule_at_secs(at_secs, ...)`, `schedule_at_mins(at_mins, ...)` — schedule at a simulation timestamp.
  - **Relative**: `schedule_in(delta_ms, ...)`, `schedule_in_secs(delta_secs, ...)`, `schedule_in_mins(delta_mins, ...)` — schedule at `now + delta`.
  - `schedule(event)` — low-level; `event.timestamp` must be in ms, ≥ now.
- **Time readout**: `now()` (ms), `now_secs()`, `now_mins()`.
- **Conversion**:
  - `sim_to_real_ms(sim_ms) -> i64` = epoch_ms + sim_ms.
  - `real_to_sim_ms(real_ms) -> Option<u64>`; `None` if real_ms is before the epoch.
- **Constants**: `ONE_SEC_MS = 1000`, `ONE_MIN_MS = 60_000`, `ONE_HOUR_MS = 3_600_000`.
- **`Event`**: `timestamp` (u64, ms), `kind`, `subject`.
- **`CurrentEvent`** (ECS `Resource`): the event currently being handled.
- **`EventKind`** / **`EventSubject`**: includes `SimulationStarted` (at time 0), `SpawnRider`, `SpawnDriver`, `ShowQuote`, `QuoteDecision`, `QuoteAccepted`, `QuoteRejected` for the rider quote flow, `TryMatch`, `MatchAccepted`, `DriverDecision`, `MoveStep`, `PickupEtaUpdated`, `TripStarted`, `TripCompleted`, `RiderCancel` for pickup timeout events, and `CheckDriverOffDuty` for periodic earnings/fatigue checks.
- **`pending_event_count()`**: returns the number of events in the queue (for tests and scenario validation).

### `sim_core::ecs`

Components and state enums:

- `RiderState`: `Browsing`, `Waiting`, `InTransit`, `Completed`, `Cancelled`
- `Rider` component: `{ state, matched_driver, destination: Option<CellIndex>, requested_at: Option<u64>, quote_rejections: u32 }`
  - `destination`: requested dropoff cell. Must be set; riders without a destination will be rejected by the driver decision system.
  - `requested_at`: simulation time (ms) when the rider was spawned (set by spawner when spawning).
  - `quote_rejections`: number of times this rider has rejected a quote; used for give-up after `max_quote_rejections`.
- `RiderQuote` component (optional, attached while viewing a quote): `{ fare: f64, eta_ms: u64 }` — current quote shown to the rider (for UI/telemetry).
- `DriverState`: `Idle`, `Evaluating`, `EnRoute`, `OnTrip`, `OffDuty`
- `Driver` component: `{ state: DriverState, matched_rider: Option<Entity> }`
- `DriverEarnings` component: `{ daily_earnings: f64, daily_earnings_target: f64, session_start_time_ms: u64 }`
  - Tracks accumulated earnings for the current day, earnings target at which driver goes OffDuty, and session start time for fatigue calculation.
- `DriverFatigue` component: `{ fatigue_threshold_ms: u64 }`
  - Maximum time on duty (in milliseconds) before driver goes OffDuty.
- `TripState`: `EnRoute`, `OnTrip`, `Completed`, `Cancelled`
- `Trip` component: `{ state, rider, driver, pickup, dropoff, requested_at: u64, matched_at: u64, pickup_at: Option<u64>, dropoff_at: Option<u64> }`
  - `pickup` / `dropoff`: trip is completed when the driver reaches `dropoff` (not a fixed +1 tick).
  - `requested_at` / `matched_at` / `pickup_at` / `dropoff_at`: simulation time in ms; used for KPIs. `dropoff_at` is set in `trip_completed_system`.
- `Position` component: `{ CellIndex }` H3 cell position for spatial matching

These are minimal placeholders to validate state transitions via systems.

### `sim_core::runner`

Clock progression and event routing are implemented here (outside systems):

- **`run_next_event(world, schedule)`**: Pops the next event from `SimulationClock`,
  inserts it as `CurrentEvent`, runs the schedule. Returns `true` if an event was
  processed, `false` if the clock was empty.
- **`run_until_empty(world, schedule, max_steps)`**: Repeatedly calls
  `run_next_event` until the event queue is empty or `max_steps` is reached.
  Returns the number of steps executed.
- **`simulation_schedule()`**: Builds the default schedule with all event-reacting
  systems (including spawner systems) plus `apply_deferred` so that spawned entities (e.g. `Trip`) are
  applied before the next step.
- **`initialize_simulation(world)`**: Schedules `SimulationStarted` event at time 0. Call this after building the scenario and before running events.

Callers (tests or a binary) use the runner to drive the sim without
duplicating the pop → route → run loop. The simulation starts with `SimulationStarted` at time 0, which triggers spawner initialization.

### `sim_core::distributions`

Probability distributions for spawner inter-arrival times, enabling variable supply and demand patterns.

- **`InterArrivalDistribution` trait**: Interface for sampling inter-arrival times (in milliseconds). `sample_ms(spawn_count, current_time_ms)` samples the next inter-arrival time; `spawn_count` allows count-varying distributions, `current_time_ms` enables time-of-day patterns.
- **`UniformInterArrival`**: Constant inter-arrival time distribution. `new(interval_ms)` creates with fixed interval; `from_rate(rate_per_sec)` creates from entities per second.
- **`ExponentialInterArrival`**: Exponential distribution for Poisson process (constant rate, random inter-arrival times). `new(rate_per_sec, seed)` creates with rate parameter (lambda) and seed for reproducibility.
- **`TimeOfDayDistribution`**: Time-of-day and day-of-week aware distribution that varies spawn rates based on hour of day (0-23) and day of week (0=Monday, 6=Sunday). Uses a base rate multiplied by time-specific factors. `new(base_rate_per_sec, epoch_ms, seed)` creates with base rate, epoch (real-world ms corresponding to simulation time 0), and seed. `set_multiplier(day_of_week, hour, multiplier)` sets multiplier for specific time; `set_day_multipliers(day_of_week, multipliers)` sets multipliers for all hours of a day. The distribution converts simulation time to real-world datetime using the epoch to determine the current hour and day of week, then applies the appropriate multiplier to the base rate before sampling from an exponential distribution.

### `sim_core::spawner`

Entity spawners: dynamically spawn riders and drivers based on distributions.

- **`RiderSpawnerConfig`**: Configuration for rider spawner:
  - `inter_arrival_dist`: Boxed `InterArrivalDistribution` controlling spawn rate.
  - `lat_min`, `lat_max`, `lng_min`, `lng_max`: Geographic bounds for spawn positions.
  - `min_trip_cells`, `max_trip_cells`: Trip length bounds (H3 cells).
  - `start_time_ms`, `end_time_ms`: Optional time window for spawning.
  - `max_count`: Optional maximum number of riders to spawn (scheduled spawns only, excludes initial count).
  - `initial_count`: Number of riders to spawn immediately at simulation start (before scheduled spawning).
- **`RiderSpawner`** (ECS `Resource`): Active rider spawner tracking `next_spawn_time_ms`, `spawned_count`, and `initialized` flag. `should_spawn(current_time_ms)` checks if spawning should continue; `advance(current_time_ms)` samples next inter-arrival time using the distribution (passing `current_time_ms` for time-aware distributions) and updates state.
- **`DriverSpawnerConfig`**: Similar to `RiderSpawnerConfig` but without trip length bounds (drivers don't have destinations). Includes `initial_count` for immediate spawns at simulation start.
- **`DriverSpawner`** (ECS `Resource`): Active driver spawner with same interface as `RiderSpawner`. `advance(current_time_ms)` passes `current_time_ms` to the distribution for time-aware sampling.
- **`random_cell_in_bounds()`**: Helper function to sample random H3 cell within lat/lng bounds.

### `sim_core::scenario`

Scenario setup: configure spawners for riders and drivers.

- **`MatchRadius`** (ECS `Resource`, default 0): max H3 grid distance for matching rider to driver. 0 = same cell only; larger values allow matching to idle drivers within that many cells. Inserted by `build_scenario` from `ScenarioParams::match_radius`.
- **`MatchingAlgorithm`** (ECS `Resource`, required): boxed trait object implementing the matching algorithm. Defaults to `CostBasedMatching` with ETA weight 0.1. Can be swapped with `SimpleMatching` or custom implementations. Inserted by `build_scenario`. The resource can be updated dynamically during simulation execution (e.g., via UI), and changes take effect immediately for new matching attempts.
- **`RiderCancelConfig`** (ECS `Resource`): configuration for rider cancellation with uniform distribution sampling. Contains `min_wait_secs` and `max_wait_secs` (bounds for the distribution, defaults to 120–2400 seconds) and `seed` (for reproducible RNG, set from scenario seed). Inserted by `build_scenario`. Cancellation times are sampled uniformly between min and max bounds, with each rider getting a different sample based on their entity ID for variety while maintaining reproducibility.
- **`RiderQuoteConfig`** (ECS `Resource`): configuration for rider quote accept/reject and give-up. Contains `max_quote_rejections` (default 3), `re_quote_delay_secs` (default 10), `accept_probability` (0.0–1.0, default 0.8), and `seed` for reproducible RNG. Inserted by `build_scenario`. Riders who reject a quote can request another after `re_quote_delay_secs`; after `max_quote_rejections` they give up and are counted in `riders_abandoned_quote_total`.
- **`SpeedModel`** (ECS `Resource`): stochastic speed sampler (defaults to 20–60 km/h) seeded from `ScenarioParams::seed` to keep runs reproducible.
- **`ScenarioParams`**: configurable scenario parameters:
  - `num_riders`, `num_drivers`: total counts (includes both initial and scheduled spawns).
  - `initial_rider_count`, `initial_driver_count`: number of entities to spawn immediately at simulation start (before scheduled spawning). Defaults to 0. These are spawned at time 0 when `SimulationStarted` event is processed.
  - `seed`: optional RNG seed for reproducibility (used for spawner distributions).
  - `lat_min`, `lat_max`, `lng_min`, `lng_max`: bounding box (degrees) for random positions; default San Francisco Bay Area.
  - `request_window_ms`: time window over which riders spawn (used to calculate rider spawn rates). Scheduled riders spawn over this window; initial riders spawn immediately at start.
  - `driver_spread_ms`: time window over which drivers spawn (used to calculate driver spawn rates). Scheduled drivers spawn over this window; initial drivers spawn immediately at start. Defaults to same value as `request_window_ms`.
  - `match_radius`: max H3 grid distance for matching (0 = same cell only).
  - `min_trip_cells`, `max_trip_cells`: trip length in H3 cells; rider destinations are chosen at random distance in this range from pickup. Travel time depends on per-trip speeds (20–60 km/h).
  - `epoch_ms`: optional epoch in milliseconds (real-world time corresponding to simulation time 0). Used by time-of-day distributions to convert simulation time to real datetime. If `None`, defaults to 0.
  - `pricing_config`: optional `PricingConfig` for pricing parameters. If `None`, uses default `PricingConfig` (base_fare 2.50, per_km_rate 1.50, commission_rate 0.0).
  - Builders: `with_seed(seed)`, `with_request_window_hours(hours)`, `with_driver_spread_hours(hours)`, `with_match_radius(radius)`, `with_trip_duration_cells(min, max)`, `with_epoch_ms(epoch_ms)`, `with_pricing_config(config)`.
- **`build_scenario(world, params)`**: inserts `SimulationClock` (with epoch set from `params.epoch_ms`), `SimTelemetry`, `MatchRadius`, `MatchingAlgorithm` (defaults to `CostBasedMatching`), `RiderCancelConfig` (with seed derived from scenario seed), `RiderQuoteConfig` (max_quote_rejections 3, re_quote_delay_secs 10, accept_probability 0.8), `PricingConfig` (from `params.pricing_config` or default), `RiderSpawner`, and `DriverSpawner`. Rider spawner uses `TimeOfDayDistribution` with realistic demand patterns (rush hours: 7-9 AM and 5-7 PM have 2.5-3.2x multipliers, night: 2-5 AM has 0.3-0.4x multipliers, Friday/Saturday evenings have higher multipliers). Driver spawner uses `TimeOfDayDistribution` with supply patterns (more consistent than demand, higher availability during rush hours with 1.5-2.0x multipliers, lower at night with 0.6-0.8x multipliers). Scheduled riders spawn continuously over `request_window_ms` with time-varying rates; scheduled drivers spawn continuously over `driver_spread_ms` with time-varying rates. Initial entities (specified by `initial_rider_count` and `initial_driver_count`) are spawned immediately when `SimulationStarted` event is processed. The spawner `max_count` is set to `num_riders - initial_rider_count` (and similarly for drivers) so that total spawns match the configured counts.
- **`random_destination()`**: Optimized destination selection function that uses different strategies based on trip distance:
  - **Small radii (≤20 cells)**: Uses `grid_disk()` to generate all candidate cells and filters by distance/bounds (more accurate, efficient for small distances).
  - **Large radii (>20 cells)**: Uses rejection sampling - randomly samples cells within bounds and checks if distance matches the target range. This avoids generating huge grid disks (e.g., ~33k cells for k=105) which dramatically improves reset performance for scenarios with large trip distances (e.g., 600 riders with 25km max trips). Falls back to a smaller `grid_disk()` if rejection sampling fails.
- Helper functions: `create_simple_matching()` returns a `SimpleMatching` algorithm, `create_cost_based_matching(eta_weight)` returns a `CostBasedMatching` algorithm with the given ETA weight.
- Also inserts `SimSnapshotConfig` and `SimSnapshots` for periodic snapshot capture (used by the UI/export).

Large scenarios (e.g. 500 riders, 100 drivers) are run via the **example** only, not in automated tests. The `random_destination()` optimization ensures fast reset times even for large scenarios with long trip distances (e.g., 600 riders over 6 hours with 25km max trips).

### `sim_core::telemetry`

- **`SimTelemetry`** (ECS `Resource`, default): holds `completed_trips: Vec<CompletedTripRecord>` plus cumulative rider totals (`riders_cancelled_total`, `riders_completed_total`, `riders_abandoned_quote_total`) and `platform_revenue_total: f64`. `riders_abandoned_quote_total` counts riders who gave up after rejecting too many quotes (distinct from pickup-timeout cancels). `platform_revenue_total` accumulates commission revenue from completed trips.
- **`CompletedTripRecord`**: `{ trip_entity, rider_entity, driver_entity, completed_at, requested_at, matched_at, pickup_at }` (all timestamps in **simulation ms**). Helper methods: **`time_to_match()`**, **`time_to_pickup()`**, **`trip_duration()`** (all in ms).
- Insert `SimTelemetry::default()` when building the world to record completed trips; `trip_completed_system` pushes one record per completed trip with timestamps from the Trip and clock, and accumulates platform revenue.
- **`PricingConfig`** (ECS `Resource`): `{ base_fare, per_km_rate, commission_rate }` controls pricing parameters. Inserted by `build_scenario` (from `ScenarioParams.pricing_config` or default). Required by `show_quote_system` and `trip_completed_system`.
- **`SimSnapshotConfig`** (ECS `Resource`): `{ interval_ms, max_snapshots }` controls snapshot cadence and buffer size.
- **`SimSnapshots`** (ECS `Resource`): rolling `VecDeque<SimSnapshot>` plus `last_snapshot_at`; populated by the snapshot system.
- **`SimSnapshot`**: `{ timestamp_ms, counts, riders, drivers, trips }` with state-aware position snapshots plus trip state snapshots for visualization/export; counts include cumulative rider totals (including `riders_abandoned_quote_total`) to account for despawns.
- **`RiderSnapshot`**: `{ entity, cell, state, matched_driver: Option<Entity> }` captures rider state and position; `matched_driver` is `Some(driver_entity)` when a driver is matched (rider is waiting for pickup) and `None` when waiting for match.
- **`DriverSnapshot`**: `{ entity, cell, state, daily_earnings: Option<f64>, daily_earnings_target: Option<f64>, session_start_time_ms: Option<u64>, fatigue_threshold_ms: Option<u64> }` captures driver state, position, and earnings/fatigue data (if available) for visualization/export.

### `sim_core::telemetry_export`

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

### `sim_core::systems::spawner`

Spawner systems: react to spawn events and create riders/drivers dynamically.

- **`simulation_started_system`**: Reacts to `EventKind::SimulationStarted` (scheduled at time 0). Initializes `RiderSpawner` and `DriverSpawner` resources if present. Spawns initial entities immediately (`initial_rider_count` riders and `initial_driver_count` drivers) at time 0, then schedules their first `SpawnRider`/`SpawnDriver` events if scheduled spawning should continue.
- **`rider_spawner_system`**: Reacts to `EventKind::SpawnRider`. If the spawner should spawn at current time:
  - Generates random position and destination using seeded RNG (deterministic based on current time and spawn count).
  - Spawns rider entity in `Browsing` state with position, destination, `requested_at = Some(clock.now())`, and `quote_rejections = 0`.
  - Schedules `ShowQuote` 1 second from now for the newly spawned rider.
  - Advances spawner to next spawn time using inter-arrival distribution.
  - Schedules next `SpawnRider` event if spawning should continue.
- **`driver_spawner_system`**: Reacts to `EventKind::SpawnDriver`. Similar to `rider_spawner_system` but spawns drivers in `Idle` state (no destination needed). Drivers spawn with random positions within configured bounds. Each driver is initialized with:
  - `DriverEarnings` component: `daily_earnings = 0.0`, `daily_earnings_target` sampled from $100-$300 range, `session_start_time_ms = current_time_ms`.
  - `DriverFatigue` component: `fatigue_threshold_ms` sampled from 8-12 hours range.

### `sim_core::systems::show_quote`

System: `show_quote_system`

- Reacts to `CurrentEvent`.
- On `EventKind::ShowQuote` with subject `Rider(rider_entity)`:
  - Rider must be in `Browsing`. Reads `PricingConfig` from resources. Computes **fare** via `calculate_trip_fare_with_config(pickup, dropoff, config)` and **ETA** (nearest idle driver distance/speed, or default 300s). Inserts `RiderQuote { fare, eta_ms }` on the rider entity.
  - Schedules `QuoteDecision` 1 second from now for the same rider.

### `sim_core::systems::quote_decision`

System: `quote_decision_system`

- Reacts to `CurrentEvent`.
- On `EventKind::QuoteDecision` with subject `Rider(rider_entity)`:
  - Rider must be in `Browsing` with `RiderQuote`. Samples accept/reject using `RiderQuoteConfig::accept_probability` (seed + rider entity ID for reproducibility).
  - If accept: schedules `QuoteAccepted` at current time. If reject: schedules `QuoteRejected` at current time.

### `sim_core::systems::quote_accepted`

System: `quote_accepted_system`

- Reacts to `CurrentEvent`.
- On `EventKind::QuoteAccepted` with subject `Rider(rider_entity)`:
  - Rider: `Browsing` → `Waiting`; removes `RiderQuote` component.
  - Schedules `TryMatch` 1 second from now (`schedule_in_secs(1, ...)`) for the same rider.
  - Samples cancellation time from uniform distribution between `min_wait_secs` and `max_wait_secs` in `RiderCancelConfig` (using seed + rider entity ID for reproducibility with variety), then schedules `RiderCancel` at that sampled time.

### `sim_core::systems::quote_rejected`

System: `quote_rejected_system`

- Reacts to `CurrentEvent`.
- On `EventKind::QuoteRejected` with subject `Rider(rider_entity)`:
  - Rider must be in `Browsing`. Increments `rider.quote_rejections`.
  - If `quote_rejections < max_quote_rejections`: schedules `ShowQuote` at `now + re_quote_delay_secs` (rider requests another quote).
  - Else: rider gives up — state set to `Cancelled`, entity despawned, `SimTelemetry::riders_abandoned_quote_total` incremented.

### `sim_core::matching`

Matching algorithm trait and implementations for driver-rider pairing.

- **`MatchingAlgorithm` trait**: Interface for matching algorithms with two methods:
  - `find_match(rider_entity, rider_pos, rider_destination, available_drivers, match_radius, clock_now_ms) -> Option<Entity>`: Finds a match for a single rider, returns the best driver entity or `None`.
  - `find_batch_matches(riders, available_drivers, match_radius, clock_now_ms) -> Vec<MatchResult>`: Finds matches for multiple riders (batch optimization). Default implementation calls `find_match` sequentially; algorithms can override for global optimization.
- **`SimpleMatching`**: First-match-within-radius algorithm. Finds the first available driver within `MatchRadius` H3 grid distance. Preserves original "first match wins" behavior.
- **`CostBasedMatching`**: Cost-based algorithm that scores driver-rider pairings by:
  - Pickup distance (km) - lower is better
  - Estimated pickup time (ms) - lower is better
  - Score formula: `-pickup_distance_km - (pickup_eta_ms / 1000.0) * eta_weight`
  - Selects the driver with the highest score (lowest cost)
  - Configurable `eta_weight` parameter (default 0.1) controls ETA importance vs distance
- **`MatchResult`**: Represents a successful match with `rider_entity` and `driver_entity`.
- **`MatchCandidate`**: Represents a potential pairing with scoring information (used internally by algorithms).

### `sim_core::systems::matching`

System: `matching_system`

- Reacts to `CurrentEvent`.
- On `EventKind::TryMatch` with subject `Rider(rider_entity)`:
  - If that rider is `Waiting`, queries the `MatchingAlgorithm` resource (required) to find a match.
  - Collects all `Idle` drivers with their positions as candidates (excludes `OffDuty` drivers).
  - Calls `find_match()` on the algorithm with the rider and available drivers.
  - If a match is found:
    - Rider stores `matched_driver = Some(driver_entity)`
    - Driver: `Idle` → `Evaluating` and stores `matched_rider = Some(rider_entity)`
    - Schedules `MatchAccepted` 1 second from now (`schedule_in_secs(1, ...)`) with subject `Driver(driver_entity)`.
  - If no driver is found, reschedules `TryMatch` after a short delay (30s).

### `sim_core::systems::match_accepted`

System: `match_accepted_system`

- Reacts to `CurrentEvent`.
- On `EventKind::MatchAccepted` with subject `Driver(driver_entity)`:
  - Schedules `DriverDecision` 1 second from now (`schedule_in_secs(1, ...)`) for the same driver.

### `sim_core::systems::driver_decision`

System: `driver_decision_system`

- Reacts to `CurrentEvent`.
- On `EventKind::DriverDecision` with subject `Driver(driver_entity)`:
  - Applies a logit accept rule:
    - Accept: `Evaluating` → `EnRoute`, **spawns a `Trip` entity** with `pickup` =
      rider’s position, `dropoff` = rider’s `destination` or a neighbor of pickup,
      `requested_at` = rider’s `requested_at`, `matched_at` = clock.now(), `pickup_at` = None;
      schedules `MoveStep` 1 second from now (`schedule_in_secs(1, ...)`) for that trip (`subject: Trip(trip_entity)`).
    - Reject: `Evaluating` → `Idle`, clears `matched_rider`, clears the rider’s `matched_driver`,
      and reschedules `TryMatch` after a short delay (30s) if the rider is still `Waiting`.

### `sim_core::systems::rider_cancel`

System: `rider_cancel_system`

- Reacts to `CurrentEvent`.
- On `EventKind::RiderCancel` with subject `Rider(rider_entity)`:
  - If the rider is still `Waiting`:
    - Rider: `Waiting` → `Cancelled`, clears `matched_driver`, then the rider entity is despawned
    - If a matched driver exists, clears `matched_rider` and returns the driver to `Idle`
    - If an `EnRoute` trip exists for that rider, marks it `Cancelled`

### `sim_core::systems::movement`

System: `movement_system`

- Reacts to `CurrentEvent`.
- On `EventKind::MoveStep` with subject `Trip(trip_entity)`:
  - **EnRoute**: moves the trip’s driver one H3 hop toward `trip.pickup` (rider cell), updates
    `trip.pickup_eta_ms` using remaining haversine distance and a stochastic speed sample
    (default 20–60 km/h), and emits `PickupEtaUpdated` for the trip. If still en route, reschedules
    `MoveStep` based on the time to traverse the next hop; when driver reaches pickup, schedules
    `TripStarted` 1 second from now (`schedule_in_secs(1, ...)`).
  - **OnTrip**: moves the trip's driver one H3 hop toward `trip.dropoff`. On each movement step,
    the rider's position is updated to match the driver's position (rider is in the vehicle).
    If still en route, reschedules `MoveStep` based on the time to traverse the next hop; when
    driver reaches dropoff, schedules `TripCompleted` 1 second from now (`schedule_in_secs(1, ...)`).

### `sim_core::systems::pickup_eta_updated`

System: `pickup_eta_updated_system`

- Reacts to `CurrentEvent`.
- On `EventKind::PickupEtaUpdated` with subject `Trip(trip_entity)`:
  - If the trip is `EnRoute` and the rider is still `Waiting`, compares projected pickup time
    (`now + trip.pickup_eta_ms`) to the rider’s wait window (`RiderCancelConfig`).
  - If the projected pickup exceeds the wait deadline (after min wait), cancels the trip, marks
    the rider cancelled, despawns the rider, and returns the driver to `Idle`.
  - **Cancelled/Completed**: no-op.
- ETA in ms: derived from haversine distance and a stochastic speed sample
  (default 20–60 km/h), with a 1 second minimum (`ONE_SEC_MS`).

This is a deterministic, FCFS-style placeholder. No distance or cost logic
is implemented yet beyond H3 grid distance and simple stochastic speeds.

### `sim_core::systems::trip_started`

System: `trip_started_system`

- Reacts to `CurrentEvent`.
- On `EventKind::TripStarted` with subject `Trip(trip_entity)`:
  - If trip is `EnRoute` and the driver is co-located with the rider (who is `Waiting`
    and matched back to this driver), transitions:
    - Rider: `Waiting` → `InTransit`; rider's position is updated to match the driver's position
      (rider is now in the vehicle).
    - Driver: `EnRoute` → `OnTrip`
    - Trip: `EnRoute` → `OnTrip`; sets `pickup_at = Some(clock.now())`.
  - Schedules `MoveStep` 1 second from now (`schedule_in_secs(1, ...)`) for the same trip so the driver moves toward dropoff; completion is scheduled by the movement system when the driver reaches dropoff.

### `sim_core::systems::trip_completed`

System: `trip_completed_system`

- Reacts to `CurrentEvent`.
- On `EventKind::TripCompleted` with subject `Trip(trip_entity)`:
  - Reads `PricingConfig` from resources.
  - Calculates total fare using `calculate_trip_fare_with_config(trip.pickup, trip.dropoff, config)`.
  - Calculates commission and driver net earnings (fare minus commission).
  - Adds driver net earnings to driver's `daily_earnings`.
  - Accumulates commission to `telemetry.platform_revenue_total`.
  - Checks if driver should go OffDuty:
    - Earnings target: if `daily_earnings >= daily_earnings_target`, driver goes OffDuty.
    - Fatigue threshold: if `session_duration_ms >= fatigue_threshold_ms`, driver goes OffDuty.
  - Driver: `OnTrip` → `Idle` (or `OffDuty` if thresholds exceeded) and clears `matched_rider`
  - Rider: `InTransit` → `Completed` and clears `matched_driver`, then the rider entity is despawned
  - Trip: `OnTrip` → `Completed`
  - Pushes a `CompletedTripRecord` to `SimTelemetry` with trip/rider/driver entities and timestamps (requested_at, matched_at, pickup_at, completed_at) for KPIs.

### `sim_core::systems::driver_offduty`

System: `driver_offduty_check_system`

- Reacts to `CurrentEvent`.
- On `EventKind::CheckDriverOffDuty`:
  - Periodically checks all active drivers (not already OffDuty) for earnings targets and fatigue thresholds.
  - Skips drivers currently on a trip (`EnRoute` or `OnTrip`); they are checked after trip completion.
  - For each eligible driver:
    - Checks if `daily_earnings >= daily_earnings_target` (earnings target reached).
    - Checks if `session_duration_ms >= fatigue_threshold_ms` (fatigue threshold exceeded).
  - Transitions drivers to `OffDuty` if either threshold is exceeded.
  - Schedules next check in 5 minutes (`CHECK_INTERVAL_MS`) if there are active drivers remaining.
  - Stops scheduling periodic checks when no active drivers remain (prevents infinite event queue).
- On `EventKind::SimulationStarted`:
  - Initializes periodic checks by scheduling the first `CheckDriverOffDuty` event 5 minutes from simulation start.

### `sim_core::systems::telemetry_snapshot`

System: `capture_snapshot_system`

- Runs after each event and captures a snapshot when `interval_ms` has elapsed.
- Records rider/driver positions and state counts into `SimSnapshots` (rolling buffer).
- For drivers, includes earnings and fatigue data if available:
  - `daily_earnings`, `daily_earnings_target`, `session_start_time_ms` from `DriverEarnings` component
  - `fatigue_threshold_ms` from `DriverFatigue` component

## Tests

Unit tests exist in each module to confirm behavior:

- `spatial`: grid disk neighbors within K.
- `clock`: events pop in time order; `schedule_in_secs` / `schedule_in_mins` and sim↔real conversion.
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
  matches rider/driver, and KPI timestamps are ordered (requested_at ≤ matched_at ≤ pickup_at ≤ completed_at); `time_to_match()`, `time_to_pickup()`, `trip_duration()` are consistent.
- **End-to-end (concurrent trips)**: Same setup with spawners configured for two riders and two drivers
  (same cell), riders spawning at t=1s and t=2s. Calls `initialize_simulation()` and runs until empty. Asserts: two
  `Trip` entities in `Completed`, both riders `Completed`, both drivers `Idle` or `OffDuty` (if thresholds exceeded);
  `SimTelemetry.completed_trips.len() == 2`.
- **Scenario**: `build_scenario` with 10 riders, 3 drivers, seed 42; asserts
  spawner configurations are correct (max_count matches params). Large scenarios (e.g.
  500 riders, 100 drivers) are only in the example, not in automated tests.

All per-system unit tests emulate the runner by popping one event, inserting
`CurrentEvent`, then running the ECS schedule.

## Example

- **`scenario_run`** (`cargo run -p sim_core --example scenario_run`): Builds a
  scenario with configurable rider/driver counts (default 500 / 100), 4h request
  window, match radius 5, trip duration 5–60 cells. Runs until the event queue
  is empty (up to 2M steps) and prints steps executed, simulation time, completed
  trip count, and up to 100 sample completed trips (time_to_match, time_to_pickup,
  trip_duration, completed_at in seconds).
- Set `SIM_EXPORT_DIR=/path` to export `completed_trips.parquet`, `trips.parquet` (all trips with full details, same as UI table), `snapshot_counts.parquet`, and `agent_positions.parquet`.
- **`sim_ui`** (`cargo run -p sim_ui`): Native UI that runs the scenario in-process,
  renders riders/drivers on a map with icons and state-based colors, and charts for
  active trips, completed trips, waiting riders, idle drivers, cancelled riders, abandoned (quote), and cancelled trips. The UI starts paused, allows
  scenario parameter edits before start, shows sim/wall-clock datetimes, overlays
  a metric grid for scale, and includes a live trip table with all trips (all states) showing pickup distance at
  driver acceptance (km), pickup-to-dropoff distance (km), and full timestamp columns (requested, matched, started, completed, cancelled),
  with timestamps shown as simulation datetimes sorted by last updated time (descending, most recent first). Controls include
  **Start** button (enabled only before simulation starts), **Step** button (advances 1 event), **Step 100** button (advances 100 events),
  **Run/Pause** toggle (auto-advances simulation at configured clock speed), **Run to end** button (runs until event queue is empty),
  and **Reset** button (resets simulation with current parameters). Match radius, trip length, and map size inputs are
  configured in kilometers and converted to H3 cell distances (resolution 9, ~0.24 km per cell);
  the map size defines the scenario bounds used for spawning and destination sampling, so it is
  only editable before the simulation starts, and the grid overlay adapts to the map size. Rider
  cancellation wait windows (min/max minutes) are configurable before start.
  **Simulation start time** is configurable via year, month, day, hour, and minute inputs (UTC);
  defaults to 2026-02-03 20:12:00 UTC but can be set to any datetime via inputs or a **"Now"** button that sets it to current wall-clock time.
  This start time is used as the simulation epoch, affecting the time-of-day patterns applied to spawn rates (rush hours, day/night variations).
  A real-time clock speed selector (10x, 20x, 50x, 100x, 200x) controls simulation playback speed. Riders in `InTransit` state
  are hidden from the map (they are with the driver). Drivers in `OnTrip` state display "D(R)" instead
  of "D" to indicate they have a rider on board. The UI differentiates between riders waiting for a
  match (yellow/orange) and riders waiting for pickup (darker orange/red) based on whether `matched_driver`
  is set, making it easy to see which riders have a driver assigned and are waiting for pickup versus
  those still searching for a match. **Driver earnings and fatigue information** can be displayed on driver
  labels in compact format: `D[50/200][3/8h]` shows earnings (current/target) and fatigue (current hours/max hours).
  A toggle checkbox "Driver stats (earnings/fatigue)" controls whether this information is displayed; when disabled,
  drivers show only "D" or "D(R)" without the earnings and fatigue brackets. The font size is 8.5pt monospace
  for compact display. **Matching algorithm** can be changed at any time (even while simulation is running) via a dropdown
  selector; changes take effect immediately for new matching attempts (riders already waiting continue with their current
  matching attempts, but new `TryMatch` events will use the updated algorithm). The metrics chart includes an **Abandoned (quote)** series for riders who gave up after rejecting too many quotes.
  
  The UI is organized into collapsible sections:
  - **Scenario parameters**: Organized in a five-column layout:
    - **Supply (Drivers)**: Initial count, spawn count, spread (hours)
    - **Demand (Riders)**: Initial count, spawn count, spread (hours), cancel wait (min/max minutes)
    - **Pricing & Matching**: Base fare, per km rate, commission rate (displayed as percentage), matching algorithm (Simple or Cost-based), match radius (km)
    - **Map & Trips**: Map size (km), trip length range (km, min–max)
    - **Timing**: Simulation start time (year/month/day/hour/minute UTC with "Now" button), seed (optional)
    All parameters except matching algorithm are only editable before simulation starts. Platform revenue is displayed in the Run outcomes section.
  - **Run outcomes**: Shows outcome counters (riders completed, riders cancelled, abandoned quote, trips completed, total resolved, conversion %, platform revenue),
    current state breakdowns (riders now: browsing/waiting/in transit, drivers now: idle/evaluating/en route/on trip/off duty, trips now: en route/on trip),
    and timing distributions for completed trips (time to match, time to pickup, trip duration) with min, max, average, and percentiles (p50, p90, p95, p99).
  - **Fleet**: Shows driver utilization metrics (busy %, total drivers, active drivers), state breakdown with percentages,
    earnings metrics (sum daily earnings/targets, targets met, off duty count, average earnings/target per driver, earnings distribution with percentiles,
    earnings/target ratio distribution with percentiles), and fatigue metrics (drivers at fatigue limit, session duration min/avg/max,
    fatigue threshold min/avg/max, drivers with fatigue data count).
  
  The trip table displays all trips (all states: EnRoute, OnTrip, Completed, Cancelled) with columns: Trip entity ID, Rider entity ID, Driver entity ID,
  State, Pickup km (at driver acceptance), Distance km (pickup to dropoff), Requested (simulation datetime), Matched (simulation datetime),
  Started (simulation datetime, if applicable), Completed (simulation datetime, if applicable), Cancelled (simulation datetime, if applicable).
  The UI scales to 80% (pixels_per_point = 0.8) for better screen fit and includes toggle checkboxes for showing/hiding riders, drivers, driver stats, and grid overlay.

## Known Gaps (Not Implemented Yet)

- Batch matching system: periodic event that collects all waiting riders and calls `find_batch_matches()` for global optimization.
- Advanced matching algorithms: bipartite matching (Hungarian algorithm) for batch optimization, opportunity cost factoring, driver value weighting.
- Driver acceptance models and rider conversion.
- Event scheduling after match beyond fixed delays (e.g. variable trip duration).
- H3-based movement or routing.
