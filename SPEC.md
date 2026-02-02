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
- Simple, deterministic systems for request intake, matching, and trip
  completion.
- A **runner API** that advances the clock and routes events (pop → insert
  `CurrentEvent` → run schedule).

This is a "crawl/walk" foundation aligned with the research plan.

## Human-Readable System Flow

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
pickup, and then move into an on-trip state. When the trip completes, the rider
is marked completed and the driver returns to idle. Matching is currently
limited to riders and drivers in the same H3 cell. Throughout this flow, riders
and drivers store links to each other so the pairing is explicit while a trip is
in progress.

## Workspace Layout

```
Cargo.toml
crates/
  sim_core/
    Cargo.toml
    src/
      clock.rs
      ecs.rs
      lib.rs
      runner.rs
      spatial.rs
      systems/
        mod.rs
        request_inbound.rs
        quote_accepted.rs
        simple_matching.rs
        match_accepted.rs
        driver_decision.rs
        movement.rs
        trip_started.rs
        trip_completed.rs
```

## Dependencies

`crates/sim_core/Cargo.toml`:

- `h3o = "0.8"` for H3 spatial indexing (stable toolchain compatible).
- `bevy_ecs = "0.13"` for ECS world, components, and systems.

## Core Modules

### `sim_core::spatial`

- `GeoIndex` stores a fixed H3 resolution.
- `grid_disk(origin, k)` wraps `h3o::CellIndex::grid_disk` and asserts the
  resolution matches the index resolution.
- Default resolution is `Resolution::Nine`.

### `sim_core::clock`

- `SimulationClock` is an ECS `Resource` holding:
  - `now: u64`
  - `events: BinaryHeap<Event>`
- `EventKind`:
  - `RequestInbound`
  - `QuoteAccepted`
  - `TryMatch`
  - `MatchAccepted`
  - `DriverDecision`
  - `MoveStep`
  - `TripStarted`
  - `TripCompleted`
- `EventSubject`:
  - `Rider(Entity)`
  - `Driver(Entity)`
  - `Trip(Entity)`
- `Event`:
  - `timestamp: u64`
  - `kind: EventKind`
  - `subject: Option<EventSubject>`
- `CurrentEvent` (ECS `Resource`):
  - Wraps the concrete `Event` currently being handled by the ECS schedule.
- The heap is a min-heap by timestamp.

### `sim_core::ecs`

Components and state enums:

- `RiderState`: `Requesting`, `Browsing`, `Waiting`, `InTransit`, `Completed`
- `Rider` component: `{ state: RiderState, matched_driver: Option<Entity> }`
- `DriverState`: `Idle`, `Evaluating`, `EnRoute`, `OnTrip`, `OffDuty`
- `Driver` component: `{ state: DriverState, matched_rider: Option<Entity> }`
- `TripState`: `EnRoute`, `OnTrip`, `Completed`
- `Trip` component: `{ state: TripState, rider: Entity, driver: Entity }`
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

### `sim_core::systems::request_inbound`

System: `request_inbound_system`

- Reacts to `CurrentEvent`.
- On `EventKind::RequestInbound` with subject `Rider(rider_entity)`:
  - Rider: `Requesting` → `Browsing`
  - Schedules `QuoteAccepted` at `clock.now() + 1` for the same rider.

### `sim_core::systems::quote_accepted`

System: `quote_accepted_system`

- Reacts to `CurrentEvent`.
- On `EventKind::QuoteAccepted` with subject `Rider(rider_entity)`:
  - Rider: `Browsing` → `Waiting`
  - Schedules `TryMatch` at `clock.now() + 1` for the same rider.

### `sim_core::systems::simple_matching`

System: `simple_matching_system`

- Reacts to `CurrentEvent`.
- On `EventKind::TryMatch` with subject `Rider(rider_entity)`:
  - If that rider is `Waiting`, finds an `Idle` driver in the same H3 cell.
  - If both exist:
    - Rider stores `matched_driver = Some(driver_entity)`
    - Driver: `Idle` → `Evaluating` and stores `matched_rider = Some(rider_entity)`
    - Schedules `MatchAccepted` at `clock.now() + 1` with subject `Driver(driver_entity)`.

### `sim_core::systems::match_accepted`

System: `match_accepted_system`

- Reacts to `CurrentEvent`.
- On `EventKind::MatchAccepted` with subject `Driver(driver_entity)`:
  - Schedules `DriverDecision` at `clock.now() + 1` for the same driver.

### `sim_core::systems::driver_decision`

System: `driver_decision_system`

- Reacts to `CurrentEvent`.
- On `EventKind::DriverDecision` with subject `Driver(driver_entity)`:
  - Applies a logit accept rule:
    - Accept: `Evaluating` → `EnRoute`, **spawns a `Trip` entity**, and schedules
      `MoveStep` for that trip (`subject: Trip(trip_entity)`).
    - Reject: `Evaluating` → `Idle` and clears `matched_rider`.

### `sim_core::systems::movement`

System: `movement_system`

- Reacts to `CurrentEvent`.
- On `EventKind::MoveStep` with subject `Trip(trip_entity)`:
  - If that trip is `EnRoute`, moves the trip’s driver one H3 hop toward the trip’s rider.
  - Reschedules `MoveStep` for the same trip if still en route.
  - Schedules `TripStarted` for the same trip when the driver reaches the rider cell.
- Uses a simple ETA helper based on H3 grid distance to pick the next
  `MoveStep` timestamp.

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
    - Trip: `EnRoute` → `OnTrip`
  - Schedules `TripCompleted` at `clock.now() + 1` for the same trip.

### `sim_core::systems::trip_completed`

System: `trip_completed_system`

- Reacts to `CurrentEvent`.
- On `EventKind::TripCompleted` with subject `Trip(trip_entity)`:
  - Driver: `OnTrip` → `Idle` and clears `matched_rider`
  - Rider: `InTransit` → `Completed` and clears `matched_driver`
  - Trip: `OnTrip` → `Completed`

## Tests

Unit tests exist in each module to confirm behavior:

- `spatial`: grid disk neighbors within K.
- `clock`: events pop in chronological order.
- `request_inbound`: rider transitions to `Browsing`.
- `quote_accepted`: rider transitions to `Waiting` and schedules `TryMatch`.
- `simple_matching`: targeted match attempt and transition.
- `match_accepted`: driver decision scheduled.
- `driver_decision`: driver accept/reject decision.
- `movement`: driver moves toward rider and schedules trip start.
- `trip_started`: trip start transitions and completion scheduling.
- `trip_completed`: rider/driver transition after completion.
- **End-to-end (single ride)**: Uses `sim_core::runner::run_until_empty` with
  `simulation_schedule()`. Seeds one
  `RequestInbound` (rider), runs until the clock is empty, then asserts one
  `Trip` in `Completed`, rider `Completed`, driver `Idle`.
- **End-to-end (concurrent trips)**: Seeds two riders and two drivers (same H3
  cell), schedules `RequestInbound` for rider1 at t=1 and rider2 at t=2, runs
  until empty. Asserts two `Trip` entities in `Completed`, both riders
  `Completed`, both drivers `Idle`.

All per-system unit tests emulate the runner by popping one event, inserting
`CurrentEvent`, then running the ECS schedule.

## Known Gaps (Not Implemented Yet)

- Real matching algorithms (bipartite matching / cost matrices).
- Driver acceptance models and rider conversion.
- Event scheduling after match beyond fixed delays (pickup, durations).
- H3-based movement or routing.
- Telemetry output / KPIs.
