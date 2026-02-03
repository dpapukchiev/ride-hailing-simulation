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
- A **scenario** module that spawns riders and drivers with random positions
  (H3 cells in a bounding box) and random request times, plus configurable
  match radius and trip duration (min/max H3 cells).

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

In the current flow, riders start by requesting a trip, browse a quote, then
wait for matching. Drivers start idle, evaluate a match offer, drive en route to
pickup, and then move into an on-trip state. If a rider waits past a randomized
pickup window, the ride is cancelled, the trip is marked cancelled, and the
driver returns to idle. When the trip completes, the rider is marked completed
and the driver returns to idle. Matching uses a configurable **match radius**
(H3 grid distance): 0 = same cell only; a larger radius allows matching to idle
drivers within that many cells. Trip length is configurable via min/max H3 cells
from pickup to dropoff (movement uses 1 min per cell, so e.g. 5–60 cells ≈ 5
min–1h). Throughout this flow, riders and drivers store links to each other so
the pairing is explicit while a trip is in progress.

## Workspace Layout

```text
README.md
Cargo.toml
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
      systems/
        mod.rs
        request_inbound.rs
        quote_accepted.rs
        simple_matching.rs
        match_accepted.rs
        driver_decision.rs
        movement.rs
        rider_cancel.rs
        trip_started.rs
        trip_completed.rs
    examples/
      scenario_run.rs
  sim_ui/
    Cargo.toml
    src/
      main.rs
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
- **Constants**: `ONE_SEC_MS = 1000`, `ONE_MIN_MS = 60_000`.
- **`Event`**: `timestamp` (u64, ms), `kind`, `subject`.
- **`CurrentEvent`** (ECS `Resource`): the event currently being handled.
- **`EventKind`** / **`EventSubject`**: includes `RiderCancel` for pickup timeout events.
- **`pending_event_count()`**: returns the number of events in the queue (for tests and scenario validation).

### `sim_core::ecs`

Components and state enums:

- `RiderState`: `Requesting`, `Browsing`, `Waiting`, `InTransit`, `Completed`, `Cancelled`
- `Rider` component: `{ state, matched_driver, destination: Option<CellIndex>, requested_at: Option<u64> }`
  - `destination`: requested dropoff cell; if `None`, a neighbor of pickup is used when the trip is created.
  - `requested_at`: simulation time (ms) when the rider transitioned to Browsing (set by `request_inbound_system`).
- `DriverState`: `Idle`, `Evaluating`, `EnRoute`, `OnTrip`, `OffDuty`
- `Driver` component: `{ state: DriverState, matched_rider: Option<Entity> }`
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
  systems plus `apply_deferred` so that spawned entities (e.g. `Trip`) are
  applied before the next step.

Callers (tests or a binary) use the runner to drive the sim without
duplicating the pop → route → run loop.

### `sim_core::scenario`

Scenario setup: spawn riders and drivers with random positions and request times.

- **`MatchRadius`** (ECS `Resource`, default 0): max H3 grid distance for matching rider to driver. 0 = same cell only; larger values allow matching to idle drivers within that many cells. Inserted by `build_scenario` from `ScenarioParams::match_radius`.
- **`RiderCancelConfig`** (ECS `Resource`): randomized pickup-wait window in seconds. Defaults to 120–600 seconds, inserted by `build_scenario`.
- **`ScenarioParams`**: configurable scenario parameters:
  - `num_riders`, `num_drivers`: counts.
  - `seed`: optional RNG seed for reproducibility.
  - `lat_min`, `lat_max`, `lng_min`, `lng_max`: bounding box (degrees) for random positions; default San Francisco Bay Area.
  - `request_window_ms`: rider `RequestInbound` times are uniform in `[0, request_window_ms]`.
  - `match_radius`: max H3 grid distance for matching (0 = same cell only).
  - `min_trip_cells`, `max_trip_cells`: trip length in H3 cells; rider destinations are chosen at random distance in this range from pickup. Movement uses 1 min per cell (e.g. 5–60 ≈ 5 min–1h).
  - Builders: `with_seed(seed)`, `with_request_window_hours(hours)`, `with_match_radius(radius)`, `with_trip_duration_cells(min, max)`.
- **`build_scenario(world, params)`**: inserts `SimulationClock`, `SimTelemetry`, `MatchRadius`, and `RiderCancelConfig`; spawns riders (with random `Position` and optional `destination` in `[min_trip_cells, max_trip_cells]` from pickup) and drivers (random `Position`); schedules one `RequestInbound` per rider at a random sim time in `[0, request_window_ms]`.
- Also inserts `SimSnapshotConfig` and `SimSnapshots` for periodic snapshot capture (used by the UI/export).

Large scenarios (e.g. 500 riders, 100 drivers) are run via the **example** only, not in automated tests.

### `sim_core::telemetry`

- **`SimTelemetry`** (ECS `Resource`, default): holds `completed_trips: Vec<CompletedTripRecord>`.
- **`CompletedTripRecord`**: `{ trip_entity, rider_entity, driver_entity, completed_at, requested_at, matched_at, pickup_at }` (all timestamps in **simulation ms**). Helper methods: **`time_to_match()`**, **`time_to_pickup()`**, **`trip_duration()`** (all in ms).
- Insert `SimTelemetry::default()` when building the world to record completed trips; `trip_completed_system` pushes one record per completed trip with timestamps from the Trip and clock.
- **`SimSnapshotConfig`** (ECS `Resource`): `{ interval_ms, max_snapshots }` controls snapshot cadence and buffer size.
- **`SimSnapshots`** (ECS `Resource`): rolling `VecDeque<SimSnapshot>` plus `last_snapshot_at`; populated by the snapshot system.
- **`SimSnapshot`**: `{ timestamp_ms, counts, riders, drivers, trips }` with state-aware position snapshots plus trip state snapshots for visualization/export.

### `sim_core::telemetry_export`

- Parquet export helpers for analytics:
  - `write_completed_trips_parquet(path, telemetry)`
  - `write_snapshot_counts_parquet(path, snapshots)`
  - `write_agent_positions_parquet(path, snapshots)`

### `sim_core::systems::request_inbound`

System: `request_inbound_system`

- Reacts to `CurrentEvent`.
- On `EventKind::RequestInbound` with subject `Rider(rider_entity)`:
  - Rider: `Requesting` → `Browsing`; sets `requested_at = Some(clock.now())`.
  - Schedules `QuoteAccepted` 1 second from now (`schedule_in_secs(1, ...)`) for the same rider.

### `sim_core::systems::quote_accepted`

System: `quote_accepted_system`

- Reacts to `CurrentEvent`.
- On `EventKind::QuoteAccepted` with subject `Rider(rider_entity)`:
  - Rider: `Browsing` → `Waiting`
  - Schedules `TryMatch` 1 second from now (`schedule_in_secs(1, ...)`) for the same rider.

### `sim_core::systems::simple_matching`

System: `simple_matching_system`

- Reacts to `CurrentEvent`.
- On `EventKind::TryMatch` with subject `Rider(rider_entity)`:
  - If that rider is `Waiting`, finds an `Idle` driver within H3 grid distance ≤ `MatchRadius` (optional resource; if absent, 0 = same cell only).
  - If both exist:
    - Rider stores `matched_driver = Some(driver_entity)`
    - Driver: `Idle` → `Evaluating` and stores `matched_rider = Some(rider_entity)`
    - Schedules `MatchAccepted` 1 second from now (`schedule_in_secs(1, ...)`) with subject `Driver(driver_entity)`.

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
    - Reject: `Evaluating` → `Idle` and clears `matched_rider`.

### `sim_core::systems::rider_cancel`

System: `rider_cancel_system`

- Reacts to `CurrentEvent`.
- On `EventKind::RiderCancel` with subject `Rider(rider_entity)`:
  - If the rider is still `Waiting` with a matched driver and an `EnRoute` trip:
    - Rider: `Waiting` → `Cancelled`, clears `matched_driver`, then the rider entity is despawned
    - Driver: `EnRoute` → `Idle`, clears `matched_rider`
    - Trip: `EnRoute` → `Cancelled`

### `sim_core::systems::movement`

System: `movement_system`

- Reacts to `CurrentEvent`.
- On `EventKind::MoveStep` with subject `Trip(trip_entity)`:
  - **EnRoute**: moves the trip’s driver one H3 hop toward `trip.pickup` (rider cell), updates
    `trip.pickup_eta_ms` based on remaining distance, and emits `PickupEtaUpdated` for the trip.
    If still en route, reschedules `MoveStep` with `schedule_in(eta_ms(remaining), ...)`; when
    driver reaches pickup, schedules `TripStarted` 1 second from now (`schedule_in_secs(1, ...)`).
  - **OnTrip**: moves the trip’s driver one H3 hop toward `trip.dropoff`. If still en route,
    reschedules `MoveStep` with `schedule_in(eta_ms(remaining), ...)`; when driver reaches dropoff,
    schedules `TripCompleted` 1 second from now (`schedule_in_secs(1, ...)`).

### `sim_core::systems::pickup_eta_updated`

System: `pickup_eta_updated_system`

- Reacts to `CurrentEvent`.
- On `EventKind::PickupEtaUpdated` with subject `Trip(trip_entity)`:
  - If the trip is `EnRoute` and the rider is still `Waiting`, compares projected pickup time
    (`now + trip.pickup_eta_ms`) to the rider’s wait window (`RiderCancelConfig`).
  - If the projected pickup exceeds the wait deadline (after min wait), cancels the trip, marks
    the rider cancelled, despawns the rider, and returns the driver to `Idle`.
  - **Cancelled/Completed**: no-op.
- ETA in ms: `eta_ms(distance)` — at least 1 second (`ONE_SEC_MS`); otherwise distance × 1 min per H3 cell (`ONE_MIN_MS`). Used for `schedule_in(eta_ms(remaining), ...)`.

This is a deterministic, FCFS-style placeholder. No distance or cost logic
is implemented yet beyond H3 grid distance.

### `sim_core::systems::trip_started`

System: `trip_started_system`

- Reacts to `CurrentEvent`.
- On `EventKind::TripStarted` with subject `Trip(trip_entity)`:
  - If trip is `EnRoute` and the driver is co-located with the rider (who is `Waiting`
    and matched back to this driver), transitions:
    - Rider: `Waiting` → `InTransit`
    - Driver: `EnRoute` → `OnTrip`
    - Trip: `EnRoute` → `OnTrip`; sets `pickup_at = Some(clock.now())`.
  - Schedules `MoveStep` 1 second from now (`schedule_in_secs(1, ...)`) for the same trip so the driver moves toward dropoff; completion is scheduled by the movement system when the driver reaches dropoff.

### `sim_core::systems::trip_completed`

System: `trip_completed_system`

- Reacts to `CurrentEvent`.
- On `EventKind::TripCompleted` with subject `Trip(trip_entity)`:
  - Driver: `OnTrip` → `Idle` and clears `matched_rider`
  - Rider: `InTransit` → `Completed` and clears `matched_driver`, then the rider entity is despawned
  - Trip: `OnTrip` → `Completed`
  - Pushes a `CompletedTripRecord` to `SimTelemetry` with trip/rider/driver entities and timestamps (requested_at, matched_at, pickup_at, completed_at) for KPIs.

### `sim_core::systems::telemetry_snapshot`

System: `capture_snapshot_system`

- Runs after each event and captures a snapshot when `interval_ms` has elapsed.
- Records rider/driver positions and state counts into `SimSnapshots` (rolling buffer).

## Tests

Unit tests exist in each module to confirm behavior:

- `spatial`: grid disk neighbors within K.
- `clock`: events pop in time order; `schedule_in_secs` / `schedule_in_mins` and sim↔real conversion.
- `request_inbound`: rider transitions to `Browsing` and sets `requested_at`.
- `quote_accepted`: rider transitions to `Waiting` and schedules `TryMatch`.
- `simple_matching`: targeted match attempt and transition.
- `match_accepted`: driver decision scheduled.
- `driver_decision`: driver accept/reject decision.
- `rider_cancel`: rider cancels when pickup wait expires.
- `movement`: driver moves toward rider and schedules trip start; `eta_ms` scales with distance.
- `trip_started`: trip start transitions and completion scheduling.
- `trip_completed`: rider/driver transition after completion.
- **End-to-end (single ride)**: Inserts `SimulationClock`, `SimTelemetry`. Seeds one
  rider (with `destination: None`) and one driver in the same cell, schedules
  `RequestInbound` (rider). Runs `run_until_empty` with `simulation_schedule()`.
  Asserts: one `Trip` in `Completed` with correct rider/driver and pickup/dropoff;
  rider `Completed`, driver `Idle`; `SimTelemetry.completed_trips.len() == 1`, record
  matches rider/driver, and KPI timestamps are ordered (requested_at ≤ matched_at ≤ pickup_at ≤ completed_at); `time_to_match()`, `time_to_pickup()`, `trip_duration()` are consistent.
- **End-to-end (concurrent trips)**: Same setup with two riders and two drivers
  (same cell), `RequestInbound` at t=1 and t=2. Runs until empty. Asserts: two
  `Trip` entities in `Completed`, both riders `Completed`, both drivers `Idle`;
  `SimTelemetry.completed_trips.len() == 2`.
- **Scenario**: `build_scenario` with 10 riders, 3 drivers, seed 42; asserts
  rider/driver counts, `pending_event_count() == 10`. Large scenarios (e.g.
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
- Set `SIM_EXPORT_DIR=/path` to export `completed_trips.parquet`, `snapshot_counts.parquet`,
  and `agent_positions.parquet`.
- **`sim_ui`** (`cargo run -p sim_ui`): Native UI that runs the scenario in-process,
  renders riders/drivers on a map with icons and state-based colors, and charts for
  active trips, completed trips, waiting riders, and idle drivers. The UI starts paused, allows
  scenario parameter edits before start, shows sim/wall-clock datetimes, overlays
  a metric grid for scale, and includes a live trip table with pickup distance at
  driver acceptance (km), pickup-to-dropoff distance (km), and cancellation time,
  with timestamps shown as simulation datetimes sorted by request time (ascending). Controls include
  step buttons and a one-click "Run to end". Match radius, trip length, and map size inputs are
  configured in kilometers and converted to H3 cell distances (resolution 9, ~0.24 km per cell);
  the map size defines the scenario bounds used for spawning and destination sampling, so it is
  only editable before the simulation starts, and the grid overlay adapts to the map size. Rider
  cancellation wait windows (min/max minutes) are configurable before start.
  to complete the simulation; a real-time clock speed selector (2x–50x) controls
  simulation playback.

## Known Gaps (Not Implemented Yet)

- Real matching algorithms (bipartite matching / cost matrices).
- Driver acceptance models and rider conversion.
- Event scheduling after match beyond fixed delays (e.g. variable trip duration).
- H3-based movement or routing.
