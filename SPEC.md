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

This is a "crawl/walk" foundation aligned with the research plan.

## Human-Readable System Flow

The system is a discrete-event loop that advances time by popping events from
the `SimulationClock`. Each event triggers an ECS system that updates rider and
driver states. Riders start by requesting a trip, browse a quote, then wait for
matching. Drivers start idle, evaluate a match offer, drive en route to pickup,
and then move into an on-trip state. When the trip completes, the rider is
marked completed and the driver returns to idle. Matching is currently limited
to riders and drivers in the same H3 cell. Throughout this flow, riders and
drivers store links to each other so the pairing is explicit while a trip is in
progress.

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
  - `MatchAccepted`
  - `TripStarted`
  - `TripCompleted`
- `Event`:
  - `timestamp: u64`
  - `kind: EventKind`
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

- Pops the next event from `SimulationClock`.
- If `EventKind::RequestInbound`, transitions:
  - `RiderState::Requesting` → `RiderState::Browsing`
- Schedules `QuoteAccepted` at `clock.now() + 1`.

### `sim_core::systems::quote_accepted`

System: `quote_accepted_system`

- Pops the next event from `SimulationClock`.
- If `EventKind::QuoteAccepted`, transitions:
  - `RiderState::Browsing` → `RiderState::Waiting`

### `sim_core::systems::simple_matching`

System: `simple_matching_system`

- Finds the first rider in `Waiting` and the first driver in `Idle` within the
  same H3 cell.
- If both exist, transitions:
  - Rider: `Waiting` stays `Waiting` and stores `matched_driver`
  - Driver: `Idle` → `Evaluating` and stores `matched_rider`
- Schedules `MatchAccepted` at `clock.now() + 1`.

### `sim_core::systems::match_accepted`

System: `match_accepted_system`

- Pops the next event from `SimulationClock`.
- If `EventKind::MatchAccepted`, transitions:
  - Driver: `Evaluating` → `EnRoute`
- Schedules `TripStarted` at `clock.now() + 1`.

This is a deterministic, FCFS-style placeholder. No distance or cost logic
is implemented yet.

### `sim_core::systems::trip_started`

System: `trip_started_system`

- Pops the next event from `SimulationClock`.
- If `EventKind::TripStarted`, transitions:
  - Rider: `Waiting` → `InTransit`
  - Driver: `EnRoute` → `OnTrip`
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
- `request_inbound`: rider transitions to `Browsing`.
- `quote_accepted`: rider transitions to `Waiting`.
- `simple_matching`: rider/driver match and transition.
- `match_accepted`: driver transitions to `EnRoute`.
- `trip_started`: trip start transitions and completion scheduling.
- `trip_completed`: rider/driver transition after completion.

## Known Gaps (Not Implemented Yet)

- Real matching algorithms (bipartite matching / cost matrices).
- Driver acceptance models and rider conversion.
- Event scheduling after match beyond fixed delays (pickup, durations).
- H3-based movement or routing.
- Telemetry output / KPIs.
