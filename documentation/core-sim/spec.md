# Core Simulation

## Human-Readable System Flow

The simulation uses a **millisecond-scale timeline**: all timestamps and `clock.now()` are in ms. Time 0 maps to a real-world datetime via `epoch_ms` (e.g. Unix epoch or a fixed start time), so it is easy to convert simulation time ↔ real datetime. Events are scheduled at specific timestamps (`schedule_at`) or at a delta from current time (`schedule_in`). The timeline advances by popping the next scheduled event; when multiple events share the same ms, they are ordered by `EventKind` for determinism.

The system is a discrete-event loop where **clock progression and event routing
happen outside ECS systems**. **The simulation executes sequentially**—one event
at a time, with systems running in a fixed order:

- The runner pops the next `Event` from `SimulationClock`, which advances time
  (`clock.now`) to that event's timestamp.
- The runner inserts that event into the ECS world as a `CurrentEvent` resource.
- The ECS schedule is run, and **systems react to that concrete `CurrentEvent`**
  (and only mutate the targeted rider/driver). Systems execute sequentially in a
  fixed order (see `simulation_schedule()`).
- Systems may schedule follow-up events back onto `SimulationClock`.
- The process repeats: pop next event → run schedule → repeat.

This sequential execution model ensures deterministic behavior and makes the
simulation easier to reason about. Parallelism is intended for running multiple
simulation runs simultaneously (e.g., Monte Carlo experiments), not for
parallelizing agents within a single simulation.

Events are **targeted** via an optional subject (e.g. `EventSubject::Rider(Entity)`,
`EventSubject::Driver(Entity)`, or `EventSubject::Trip(Entity)`), which allows
multiple trips to be "in flight" at once without global "scan everything"
transitions.

Once a driver accepts, the simulation creates a dedicated **Trip entity** that
becomes the stable identifier for the rest of the lifecycle (movement, start,
completion).

In the current flow, riders and drivers are spawned dynamically by spawner systems
reacting to `SimulationStarted` and spawn events. Spawners use inter-arrival time
distributions to control spawn rates, enabling variable supply and demand patterns.
The default distributions vary rates based on time of day (hour) and day of week,
creating realistic patterns with rush hours (7-9 AM, 5-7 PM) and lower demand at night.
Riders spawn with the `Browsing` marker. One second after spawn, `ShowQuote` runs: the sim computes a quote (fare from trip distance, ETA from nearest idle driver or a default), attaches a `RiderQuote` component, and schedules `QuoteDecision` 1s later. On `QuoteDecision`, the rider stochastically accepts or rejects (configurable probability). If the rider **accepts** (`QuoteAccepted`), they transition to `Waiting` and pickup-timeout cancellation is scheduled. When batch matching is **disabled**, `TryMatch` is scheduled 1s later for that rider; when batch matching is **enabled**, the rider remains waiting until the next `BatchMatchRun`. If the rider **rejects** (`QuoteRejected`), their quote rejection count increments; if under the configured max, `ShowQuote` is rescheduled after a delay (re-quote); otherwise the rider gives up (marked cancelled, despawned, and counted in `riders_abandoned_quote_total`). Drivers spawn
in `Idle` (marker) continuously over the simulation window with earnings targets ($100-$300)
and fatigue thresholds (8-12 hours), evaluate a match offer, drive en route to pickup, and then move into
an on-trip marker. If
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

## `sim_core::spatial`

- `GeoIndex` stores a fixed H3 resolution.
- `grid_disk(origin, k)` wraps `h3o::CellIndex::grid_disk` and asserts the
  resolution matches the index resolution.
- Default resolution is `Resolution::Nine`.
- `distance_km_between_cells(a, b)` calculates haversine distance between two H3 cells in kilometers. Uses a global LRU cache (50,000 entries, ~800KB memory) to avoid repeated H3 cell → LatLng conversions and Haversine calculations for frequently accessed cell pairs. Cache keys use symmetric ordering (smaller cell first) to maximize cache hits. All cache mutex locks use graceful fallbacks: if a mutex is poisoned, the function computes the result without caching instead of panicking.
- `grid_disk_cached(origin, k)` returns grid disk results with LRU caching (1,000 entries). Falls back to uncached computation on mutex poisoning.
- `grid_path_cells_cached(from, to)` returns grid path results with LRU caching (5,000 entries). Only caches successful paths. Falls back to uncached computation on mutex poisoning.

## `sim_core::clock`

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
- **`EventKind`** / **`EventSubject`**: includes `SimulationStarted` (at time 0), `SpawnRider`, `SpawnDriver`, `ShowQuote`, `QuoteDecision`, `QuoteAccepted`, `QuoteRejected` for the rider quote flow, `TryMatch`, `BatchMatchRun` (global batch matching when batch mode is enabled), `MatchAccepted`, `DriverDecision`, `MatchRejected` (rider-side cleanup after driver rejects), `MoveStep`, `PickupEtaUpdated`, `TripStarted`, `TripCompleted`, `RiderCancel` for pickup timeout events, and `CheckDriverOffDuty` for periodic earnings/fatigue checks.
- **`pending_event_count()`**: returns the number of events in the queue (for tests and scenario validation).

## `sim_core::ecs`

Components and state markers:

- Rider state markers: `Browsing`, `Waiting`, `InTransit`, `RiderCompleted`, `RiderCancelled`
- `Rider` component: `{ matched_driver, assigned_trip: Option<Entity>, destination: Option<CellIndex>, requested_at: Option<u64>, quote_rejections: u32, accepted_fare: Option<f64>, last_rejection_reason: Option<RiderAbandonmentReason> }`
  - `assigned_trip`: backlink to the active Trip entity. Set when a Trip is spawned (driver_decision_system), cleared on trip completion/cancellation. Enables O(1) trip lookup instead of scanning all Trip entities.
  - `destination`: requested dropoff cell. Must be set; riders without a destination will be rejected by the driver decision system.
  - `requested_at`: simulation time (ms) when the rider was spawned (set by spawner when spawning).
  - `quote_rejections`: number of times this rider has rejected a quote; used for give-up after `max_quote_rejections`.
  - `accepted_fare`: fare the rider accepted when transitioning to Waiting; used for driver earnings and trip completion.
  - `last_rejection_reason`: tracks the reason for the most recent quote rejection (`QuotePriceTooHigh`, `QuoteEtaTooLong`, `QuoteStochasticRejection`); used to record abandonment reason when rider gives up.
- `RiderQuote` component (optional, attached while viewing a quote): `{ fare: f64, eta_ms: u64 }` — current quote shown to the rider (for UI/telemetry).
- Driver state markers: `Idle`, `Evaluating`, `EnRoute`, `OnTrip`, `OffDuty`
- `Driver` component: `{ matched_rider: Option<Entity>, assigned_trip: Option<Entity> }`
  - `assigned_trip`: backlink to the active Trip entity (same as Rider). Enables O(1) trip lookup.
- `DriverStateCommands` (extension trait for `EntityCommands`): helper methods to transition a driver to a single marker by clearing all driver state markers and inserting the target (`set_driver_state_idle`, `set_driver_state_evaluating`, `set_driver_state_en_route`, `set_driver_state_on_trip`, `set_driver_state_off_duty`).
- `DriverEarnings` component: `{ daily_earnings: f64, daily_earnings_target: f64, session_start_time_ms: u64, session_end_time_ms: Option<u64> }`
  - Tracks accumulated earnings for the current day, earnings target at which driver goes OffDuty, session start time for fatigue calculation, and session end time (set when the driver goes OffDuty, `None` while active).
- `DriverFatigue` component: `{ fatigue_threshold_ms: u64 }`
  - Maximum time on duty (in milliseconds) before driver goes OffDuty.
- Trip state markers: `TripEnRoute`, `TripOnTrip`, `TripCompleted`, `TripCancelled`
- `Trip` component (core identity + spatial): `{ rider: Entity, driver: Entity, pickup: CellIndex, dropoff: CellIndex }`
  - `pickup` / `dropoff`: trip is completed when the driver reaches `dropoff` (not a fixed +1 tick).
- `TripTiming` component (lifecycle timestamps): `{ requested_at: u64, matched_at: u64, pickup_at: Option<u64>, dropoff_at: Option<u64>, cancelled_at: Option<u64> }`
  - `requested_at` / `matched_at` / `pickup_at` / `dropoff_at`: simulation time in ms; used for KPIs. `dropoff_at` is set in `trip_completed_system`.
  - `cancelled_at`: simulation time when the trip was cancelled; set in `rider_cancel_system`.
- `TripFinancials` component (fare + distance metrics): `{ agreed_fare: Option<f64>, pickup_distance_km_at_accept: f64 }`
  - `agreed_fare`: fare agreed at quote accept (may include surge); used for driver earnings and platform revenue in `trip_completed_system`.
  - `pickup_distance_km_at_accept`: distance from driver to pickup at match acceptance time (km).
- `TripLiveData` component (actively updated during en-route): `{ pickup_eta_ms: u64 }`
  - `pickup_eta_ms`: estimated time to pickup from current driver position (ms), updated in `movement_system`.
- `TripRoute` component (optional, attached after first MoveStep): `{ cells: Vec<CellIndex>, current_index: usize, total_distance_km: f64 }` — resolved route for a trip. Contains the full cell path so subsequent MoveSteps advance along it without re-querying the route provider.
- `Position` component: `{ CellIndex }` H3 cell position for spatial matching

These are minimal placeholders to validate state transitions via systems.
Telemetry snapshots/export use enum states defined in `sim_core::telemetry` and are derived from the marker components at snapshot time.

## `sim_core::runner`

Clock progression and event routing are implemented here (outside systems):

- **`run_next_event(world, schedule)`**: Pops the next event from `SimulationClock`,
  inserts it as `CurrentEvent`, runs the schedule. Returns `true` if an event was
  processed, `false` if the clock was empty or if the next event is at or past
  `SimulationEndTimeMs` (when that resource is present). **Executes sequentially**—one event
  at a time. If an `EventMetrics` resource exists, records the event kind for performance tracking.
- **`run_next_event_with_hook(world, schedule, hook)`**: Similar to `run_next_event` but invokes `hook` after the schedule completes. Useful for custom per-event processing.
- **`run_until_empty(world, schedule, max_steps)`**: Repeatedly calls
  `run_next_event` until the event queue is empty or `max_steps` is reached.
  Returns the number of steps executed.
- **`run_until_empty_with_hook(world, schedule, max_steps, hook)`**: Similar to `run_until_empty` but invokes `hook` after each step.
- **`simulation_schedule()`**: Builds the default schedule with all event-reacting
  systems (including spawner systems) plus `apply_deferred` so that spawned entities (e.g. `Trip`) are
  applied before the next step. Systems are conditionally executed based on event type using `run_if` conditions to reduce overhead (only systems relevant to the current event type run). Systems **execute
  sequentially** for each event. While bevy_ecs may parallelize queries within a
  system, the systems themselves run one after another in the defined order.
- **`initialize_simulation(world)`**: Schedules `SimulationStarted` event at time 0. Call this after building the scenario and before running events.

Callers (tests or a binary) use the runner to drive the sim without
duplicating the pop → route → run loop. The simulation starts with `SimulationStarted` at time 0, which triggers spawner initialization.

## `sim_core::distributions`

Probability distributions for spawner inter-arrival times, enabling variable supply and demand patterns.

- **`InterArrivalDistribution` trait**: Interface for sampling inter-arrival times (in milliseconds). `sample_ms(spawn_count, current_time_ms)` samples the next inter-arrival time; `spawn_count` allows count-varying distributions, `current_time_ms` enables time-of-day patterns.
- **`UniformInterArrival`**: Constant inter-arrival time distribution. `new(interval_ms)` creates with fixed interval; `from_rate(rate_per_sec)` creates from entities per second.
- **`ExponentialInterArrival`**: Exponential distribution for Poisson process (constant rate, random inter-arrival times). `new(rate_per_sec, seed)` creates with rate parameter (lambda) and seed for reproducibility.
- **`TimeOfDayDistribution`**: Time-of-day and day-of-week aware distribution that varies spawn rates based on hour of day (0-23) and day of week (0=Monday, 6=Sunday). Uses a base rate multiplied by time-specific factors. `new(base_rate_per_sec, epoch_ms, seed)` creates with base rate, epoch (real-world ms corresponding to simulation time 0), and seed. `set_multiplier(day_of_week, hour, multiplier)` sets multiplier for specific time; `set_day_multipliers(day_of_week, multipliers)` sets multipliers for all hours of a day. The distribution converts simulation time to real-world datetime using the epoch to determine the current hour and day of week, then applies the appropriate multiplier to the base rate before sampling from an exponential distribution.

See [CONFIG.md](../../CONFIG.md#spawner-configuration--patterns) for time-of-day multiplier patterns, spawn rate calculations, and inter-arrival sampling formulas.

## `sim_core::spawner`

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

## `sim_core::scenario`

Scenario setup: configure spawners for riders and drivers.

- **`MatchRadius`** (ECS `Resource`, default 0): max H3 grid distance for matching rider to driver. 0 = same cell only; larger values allow matching to idle drivers within that many cells. Inserted by `build_scenario` from `ScenarioParams::match_radius`.
- **`SimulationEndTimeMs`** (ECS `Resource`, optional): when present, the runner stops processing events once the next event would be at or after this timestamp (simulation time in ms). Inserted by `build_scenario` when `ScenarioParams::simulation_end_time_ms` is set. Used so simulations with recurring events (e.g. batch matching) can finish in bounded time.
- **`BatchMatchingConfig`** (ECS `Resource`): `enabled` (bool) and `interval_secs` (u64). When enabled, `BatchMatchRun` events are scheduled and per-rider `TryMatch` is not used. Default: enabled true, interval 5s. Inserted by `build_scenario`.
- **`MatchingAlgorithm`** (ECS `Resource`, required): boxed trait object implementing the matching algorithm. Defaults to `HungarianMatching` with ETA weight 0.1. Can be swapped with `SimpleMatching`, `CostBasedMatching`, or `HungarianMatching`. Inserted by `build_scenario`. The resource can be updated dynamically during simulation execution (e.g., via UI), and changes take effect immediately for new matching attempts.
- **`RiderCancelConfig`** (ECS `Resource`): configuration for rider cancellation with uniform distribution sampling. Contains `min_wait_secs` and `max_wait_secs` (bounds for the distribution, defaults to 120–2400 seconds) and `seed` (for reproducible RNG, set from scenario seed). Inserted by `build_scenario`. Cancellation times are sampled uniformly between min and max bounds, with each rider getting a different sample based on their entity ID for variety while maintaining reproducibility.
- **`RiderQuoteConfig`** (ECS `Resource`): configuration for rider quote accept/reject and give-up. Contains `max_quote_rejections` (default 3), `re_quote_delay_secs` (default 10), `accept_probability` (0.0–1.0, default 0.8), `seed`, `max_willingness_to_pay` (default 100.0), and `max_acceptable_eta_ms` (default 600_000). Inserted by `build_scenario` from `ScenarioParams::rider_quote_config` or default. Riders reject the quote if fare > max_willingness_to_pay or eta_ms > max_acceptable_eta_ms; otherwise accept/reject is stochastic. After `max_quote_rejections` they give up and are counted in `riders_abandoned_quote_total`.
- **`DriverDecisionConfig`** (ECS `Resource`): configuration for driver accept/reject decisions using a stochastic logit model. Contains `seed`, `fare_weight` (default 0.1), `pickup_distance_penalty` (default -2.0), `trip_distance_bonus` (default 0.5), `earnings_progress_weight` (default -0.5), `fatigue_penalty` (default -1.0), and `base_acceptance_score` (default 1.0). Inserted by `build_scenario` from `ScenarioParams::driver_decision_config` or default. Driver acceptance probability is calculated from a logit score based on fare, distances, earnings progress, and fatigue. See [CONFIG.md](../../CONFIG.md#driver-behavior) for detailed formulas.
- **`SpeedModel`** (ECS `Resource`): stochastic speed sampler (defaults to 20–60 km/h) seeded from `ScenarioParams::seed` to keep runs reproducible.
- **`ScenarioParams`**: configurable scenario parameters (see [CONFIG.md](../../CONFIG.md#spawner-configuration--patterns) for defaults and detailed descriptions).
- **`build_scenario(world, params)`**: inserts all required resources and configures spawners. Rider spawner uses `TimeOfDayDistribution` with realistic demand patterns; driver spawner uses `TimeOfDayDistribution` with supply patterns. Scheduled riders/drivers spawn continuously over their respective time windows with time-varying rates. Initial entities are spawned immediately when `SimulationStarted` event is processed. The spawner `max_count` is set to `num_riders - initial_rider_count` (and similarly for drivers) so that total spawns match the configured counts.
- **`random_destination()`**: Optimized destination selection function that uses different strategies based on trip distance:
  - **Small radii (≤20 cells)**: Uses `grid_disk()` to generate all candidate cells and filters by distance/bounds (more accurate, efficient for small distances).
  - **Large radii (>20 cells)**: Uses rejection sampling - randomly samples cells within bounds and checks if distance matches the target range. This avoids generating huge grid disks (e.g., ~33k cells for k=105) which dramatically improves reset performance for scenarios with large trip distances (e.g., 600 riders with 25km max trips). Falls back to a smaller `grid_disk()` if rejection sampling fails.
- Helper functions: `create_simple_matching()`, `create_cost_based_matching(eta_weight)`, `create_hungarian_matching(eta_weight)` return the corresponding algorithm. Default algorithm in `build_scenario` is Hungarian.
- Also inserts `SimSnapshotConfig` and `SimSnapshots` for periodic snapshot capture (used by the UI/export).

Large scenarios (e.g. 500 riders, 100 drivers) are run via the **example** only, not in automated tests. The `random_destination()` optimization ensures fast reset times even for large scenarios with long trip distances (e.g., 600 riders over 6 hours with 25km max trips).

See [CONFIG.md](../../CONFIG.md) for detailed configuration parameters, formulas, time-of-day patterns, and spawn rate calculations.

## `sim_core::systems::spawner`

Spawner systems: react to spawn events and create riders/drivers dynamically.

- **`simulation_started_system`**: Reacts to `EventKind::SimulationStarted` (scheduled at time 0). When `BatchMatchingConfig` is present and enabled, schedules the first `BatchMatchRun` at time 0. Initializes `RiderSpawner` and `DriverSpawner` resources if present. Spawns initial entities immediately (`initial_rider_count` riders and `initial_driver_count` drivers) at time 0, then schedules their first `SpawnRider`/`SpawnDriver` events if scheduled spawning should continue.
- **`rider_spawner_system`**: Reacts to `EventKind::SpawnRider`. If the spawner should spawn at current time:
  - Generates random position and destination using seeded RNG (deterministic based on current time and spawn count).
  - Spawns rider entity with `Browsing` marker, position, destination, `requested_at = Some(clock.now())`, and `quote_rejections = 0`.
  - Schedules `ShowQuote` 1 second from now for the newly spawned rider.
  - Advances spawner to next spawn time using inter-arrival distribution.
  - Schedules next `SpawnRider` event if spawning should continue.
- **`driver_spawner_system`**: Reacts to `EventKind::SpawnDriver`. Similar to `rider_spawner_system` but spawns drivers with the `Idle` marker (no destination needed). Drivers spawn with random positions within configured bounds. Each driver is initialized with:
  - `DriverEarnings` component: `daily_earnings = 0.0`, `daily_earnings_target` sampled from $100-$300 range, `session_start_time_ms = current_time_ms`, `session_end_time_ms = None`.
  - `DriverFatigue` component: `fatigue_threshold_ms` sampled from 8-12 hours range.

See [CONFIG.md](../../CONFIG.md#driver-behavior) for driver earnings target and fatigue threshold sampling formulas.

## `sim_core::systems::movement`

System: `movement_system`

- Reacts to `CurrentEvent`.
- On `EventKind::MoveStep` with subject `Trip(trip_entity)`:
  - **EnRoute (`TripEnRoute`)**: moves the trip's driver one H3 hop toward `trip.pickup` (rider cell), updates
    `trip.pickup_eta_ms` using remaining haversine distance and a stochastic speed sample
    (default 20–60 km/h), and emits `PickupEtaUpdated` for the trip. If still en route, reschedules
    `MoveStep` based on the time to traverse the next hop; when driver reaches pickup, schedules
    `TripStarted` 1 second from now (`schedule_in_secs(1, ...)`).
  - **OnTrip (`TripOnTrip`)**: moves the trip's driver one H3 hop toward `trip.dropoff`. On each movement step,
    the rider's position is updated to match the driver's position (rider is in the vehicle).
    If still en route, reschedules `MoveStep` based on the time to traverse the next hop; when
    driver reaches dropoff, schedules `TripCompleted` 1 second from now (`schedule_in_secs(1, ...)`).

## `sim_core::systems::trip_started`

System: `trip_started_system`

- Reacts to `CurrentEvent`.
- On `EventKind::TripStarted` with subject `Trip(trip_entity)`:
  - If trip has `TripEnRoute` and the driver is co-located with the rider (who has `Waiting`
    and matched back to this driver), transitions:
    - Rider: `Waiting` → `InTransit` (marker swap); rider's position is updated to match the driver's position
      (rider is now in the vehicle).
    - Driver: `EnRoute` → `OnTrip` (via `DriverStateCommands`)
    - Trip: `TripEnRoute` → `TripOnTrip`; sets `pickup_at = Some(clock.now())`.
  - Schedules `MoveStep` 1 second from now (`schedule_in_secs(1, ...)`) for the same trip so the driver moves toward dropoff; completion is scheduled by the movement system when the driver reaches dropoff.

## `sim_core::systems::trip_completed`

System: `trip_completed_system`

- Reacts to `CurrentEvent`.
- On `EventKind::TripCompleted` with subject `Trip(trip_entity)`:
  - Fare = `trip.agreed_fare` if present, else `calculate_trip_fare_with_config(trip.pickup, trip.dropoff, config)`.
  - Calculates commission and driver net earnings (fare minus commission).
  - Adds driver net earnings to driver's `daily_earnings`.
  - Accumulates commission to `telemetry.platform_revenue_total` and fare to `telemetry.total_fares_collected`.
  - Driver: `OnTrip` → `Idle` (marker swap) and clears `matched_rider` and `assigned_trip`
  - Schedules `CheckDriverOffDuty` at delta 0 for the driver so `driver_offduty_check_system` handles the earnings/fatigue threshold check and potential `OffDuty` transition.
  - Rider: `InTransit` → `RiderCompleted` (marker swap) and clears `matched_driver`, then the rider entity is despawned
  - Trip: `TripOnTrip` → `TripCompleted`
  - Pushes a `CompletedTripRecord` to `SimTelemetry` with trip/rider/driver entities, timestamps (requested_at, matched_at, pickup_at, completed_at), and fare for KPIs.

## `sim_core::profiling`

Performance profiling infrastructure: system timing, event rate tracking, and metrics collection.

- **`SystemTiming`**: Per-system timing metrics with total duration, call count, min/max durations, and average calculation.
- **`SystemTimings`** (ECS `Resource`): Aggregated system timing metrics, keyed by system name. Provides `record(system_name, duration)` to track execution time, `get(system_name)` to retrieve timing data, and `print_summary()` to display statistics sorted by total duration.
- **`EventMetrics`** (ECS `Resource`): Event processing rate metrics. Tracks total events processed, events per kind, and calculates events per second. Automatically records events when present in the world (integrated into `run_next_event`). Provides `record_event(kind)` to manually track events, `events_per_second()` for current rate, and `print_summary()` to display statistics.
- **`time_system!` macro**: Helper macro to time a system execution. Usage: `time_system!(system_name, timings_resource, { system_body })`. Records timing if `SystemTimings` resource exists.


