# Ride-Hailing Simulation Spec (Current State)

This spec documents the code that exists today in this repository. It is the
single source of truth for the current implementation and should be updated
whenever code or spec changes are made.

## Overview

The project is a Rust-based discrete event simulation (DES) scaffold with a
minimal ECS-based agent model. It currently supports:

- A H3-based spatial index wrapper.
- A binary-heap simulation clock with discrete events.
- ECS components for riders and drivers, including pairing links.
- Simple, deterministic systems for request intake, matching, and trip
  completion.
- Docker-based build and test execution.

This is a "crawl/walk" foundation aligned with the research plan.

## Workspace Layout

```
Cargo.toml
Dockerfile
docker-run.sh
.dockerignore
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
        simple_matching.rs
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
  - `TripStarted`
  - `TripCompleted`
- `Event`:
  - `timestamp: u64`
  - `kind: EventKind`
- The heap is a min-heap by timestamp.

### `sim_core::ecs`

Components and state enums:

- `RiderState`: `Requesting`, `WaitingForMatch`, `Matched`, `InTransit`, `Completed`
- `Rider` component: `{ state: RiderState, matched_driver: Option<Entity> }`
- `DriverState`: `Idle`, `Assigned`, `OnTrip`
- `Driver` component: `{ state: DriverState, matched_rider: Option<Entity> }`

These are minimal placeholders to validate state transitions via systems.

### `sim_core::systems::request_inbound`

System: `request_inbound_system`

- Pops the next event from `SimulationClock`.
- If `EventKind::RequestInbound`, transitions:
  - `RiderState::Requesting` → `RiderState::WaitingForMatch`

### `sim_core::systems::simple_matching`

System: `simple_matching_system`

- Finds the first rider in `WaitingForMatch` and the first driver in `Idle`.
- If both exist, transitions:
  - Rider: `WaitingForMatch` → `Matched` and stores `matched_driver`
  - Driver: `Idle` → `Assigned` and stores `matched_rider`
- Schedules `TripStarted` at `clock.now() + 1`.

This is a deterministic, FCFS-style placeholder. No distance or cost logic
is implemented yet.

### `sim_core::systems::trip_started`

System: `trip_started_system`

- Pops the next event from `SimulationClock`.
- If `EventKind::TripStarted`, transitions:
  - Rider: `Matched` → `InTransit`
  - Driver: `Assigned` → `OnTrip`
- Schedules `TripCompleted` at `clock.now() + 1`.

### `sim_core::systems::trip_completed`

System: `trip_completed_system`

- Pops the next event from `SimulationClock`.
- If `EventKind::TripCompleted`, transitions:
  - Rider: `InTransit` → `Completed` and clears `matched_driver`
  - Driver: `OnTrip` → `Idle` and clears `matched_rider`

## Tests

Unit tests exist in each module to confirm behavior:

- `spatial`: grid disk neighbors within K.
- `clock`: events pop in chronological order.
- `request_inbound`: rider transitions to `WaitingForMatch`.
- `simple_matching`: rider/driver match and transition.
- `trip_started`: trip start transitions and completion scheduling.
- `trip_completed`: rider/driver transition after completion.

## Running in Docker

Build and run tests via the provided script:

```
sh docker-run.sh
```

The container runs `cargo test --workspace`.

## Known Gaps (Not Implemented Yet)

- Real matching algorithms (bipartite matching / cost matrices).
- Driver acceptance models and rider conversion.
- Event scheduling after match beyond fixed delays (pickup, durations).
- H3-based movement or routing.
- Telemetry output / KPIs.
