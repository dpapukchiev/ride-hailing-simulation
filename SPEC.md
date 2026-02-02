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

Events are **targeted** via an optional subject (`EventSubject::Rider(Entity)` or
`EventSubject::Driver(Entity)`), which allows multiple trips to be “in flight” at
once without global “scan everything” transitions.

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
      spatial.rs
      lib.rs
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
- `Position` component: `{ CellIndex }` H3 cell position for spatial matching

These are minimal placeholders to validate state transitions via systems.

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
    - Accept: `Evaluating` → `EnRoute` and schedules `MoveStep` for that driver.
    - Reject: `Evaluating` → `Idle` and clears `matched_rider`.

### `sim_core::systems::movement`

System: `movement_system`

- Reacts to `CurrentEvent`.
- On `EventKind::MoveStep` with subject `Driver(driver_entity)`:
  - If that driver is `EnRoute`, moves it one H3 hop toward its matched rider.
  - Reschedules `MoveStep` for the same driver if still en route.
  - Schedules `TripStarted` for the same driver when it reaches the rider cell.
- Uses a simple ETA helper based on H3 grid distance to pick the next
  `MoveStep` timestamp.

This is a deterministic, FCFS-style placeholder. No distance or cost logic
is implemented yet beyond H3 grid distance.

### `sim_core::systems::trip_started`

System: `trip_started_system`

- Reacts to `CurrentEvent`.
- On `EventKind::TripStarted` with subject `Driver(driver_entity)`:
  - If driver is `EnRoute` and co-located with its matched rider (who is `Waiting`
    and matched back to this driver), transitions:
    - Rider: `Waiting` → `InTransit`
    - Driver: `EnRoute` → `OnTrip`
  - Schedules `TripCompleted` at `clock.now() + 1` for the same driver.

### `sim_core::systems::trip_completed`

System: `trip_completed_system`

- Reacts to `CurrentEvent`.
- On `EventKind::TripCompleted` with subject `Driver(driver_entity)`:
  - Driver: `OnTrip` → `Idle` and clears `matched_rider`
  - Rider (matched to that driver): `InTransit` → `Completed` and clears `matched_driver`

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
- `systems (end-to-end)`: simulates one full ride end-to-end by running a
  runner loop (pop clock → insert `CurrentEvent` → run schedule) until the event
  queue is empty, asserting the rider ends `Completed` and the driver ends `Idle`.

All system tests now emulate the runner: they pop the next event from the clock,
insert it as `CurrentEvent`, then run the ECS schedule.

## Known Gaps (Not Implemented Yet)

- Real matching algorithms (bipartite matching / cost matrices).
- Driver acceptance models and rider conversion.
- Event scheduling after match beyond fixed delays (pickup, durations).
- H3-based movement or routing.
- Telemetry output / KPIs.
