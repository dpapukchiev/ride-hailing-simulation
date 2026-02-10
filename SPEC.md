# Ride-Hailing Simulation Spec (Current State)

This spec documents the code that exists today in this repository. It is the
single source of truth for the current implementation and should be updated
whenever code or spec changes are made.

**For configuration parameters, formulas, and detailed rules, see [CONFIG.md](./CONFIG.md).**
CONFIG.md provides a comprehensive reference for all configurable parameters, formulas, and clearly distinguishes between random/stochastic elements and deterministic/hard-coded rules.

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
  with configurable commission rates and optional surge pricing (dynamic multipliers when demand exceeds supply in local H3 clusters). Driver earnings and platform revenue are tracked separately.
- **Rider quote flow**: Riders receive an explicit quote (fare + ETA) before committing; they can
  accept (proceed to matching), reject and request another quote, or give up after a configurable
  number of rejections (tracked in telemetry as abandoned-quote).
- **Batch matching**: Optional mode where a periodic `BatchMatchRun` event collects all unmatched waiting riders and idle drivers, runs a global matching algorithm (e.g. Hungarian), and applies matches; when enabled, per-rider `TryMatch` is not scheduled. Rejected riders re-enter the next batch.
- **Simulation end time**: Optional `SimulationEndTimeMs` resource stops the runner when the next event is at or after that time, so runs with recurring events (e.g. batch matching) finish in bounded time.
- **Parallel experimentation**: Run multiple simulations in parallel with varying parameters to explore parameter space and analyze marketplace health.
- **Pluggable routing**: A `RouteProvider` trait abstracts routing backends. Three implementations: `H3GridRouteProvider` (zero-dependency default), `OsrmRouteProvider` (OSRM HTTP, feature-gated), and `PrecomputedRouteProvider` (binary route table, feature-gated). Routes are cached in an LRU cache with H3 fallback on failure. The active provider is selected via `RouteProviderKind` in `ScenarioParams`.
- **Traffic model**: Time-of-day speed profiles (`TrafficProfile`), spatial congestion zones (`CongestionZones`), and dynamic density-based congestion. These multiply the effective vehicle speed via `SpeedFactors.multiplier`. A Berlin profile is built-in with rush-hour slowdowns.
- **Berlin map support**: Default geographic bounds are Berlin (lat 52.34–52.68, lng 13.08–13.76). OSRM setup scripts and Docker Compose are provided in `infra/osrm/` for running a local OSRM instance with Berlin OpenStreetMap data.

This is a "crawl/walk" foundation aligned with the research plan.

## Documentation

Detailed documentation is organized by topic in the [`documentation/`](./documentation/) folder.
Each subfolder contains a `spec.md` with technical details and a `user-stories.md` with
implementable stories and their status.

| Topic | Description |
|---|---|
| [core-sim](./documentation/core-sim/) | System flow, clock, ECS components, runner, spawner, scenario, distributions, movement, trip lifecycle, profiling |
| [drivers](./documentation/drivers/) | Driver decision system, off-duty checks, driver ECS components |
| [riders](./documentation/riders/) | Quote decision, accept/reject, cancellation, pickup ETA, rider ECS components |
| [matching](./documentation/matching/) | Matching algorithms (Simple, Cost-based, Hungarian), per-rider and batch matching systems |
| [pricing](./documentation/pricing/) | Pricing module, show-quote system, surge pricing |
| [telemetry](./documentation/telemetry/) | Telemetry resources, Parquet export, snapshot system |
| [experiments](./documentation/experiments/) | Parallel experimentation framework, parameter sweeps, health scores |
| [ui](./documentation/ui/) | Native simulation UI, controls, charts, trip table |
| [testing](./documentation/testing/) | Unit tests, benchmarks (Criterion), load tests |
| [project](./documentation/project/) | Workspace layout, dependencies, tooling, task runner, CI |

## Examples

- **`scenario_run`** (`cargo run -p sim_core --example scenario_run`): Builds a
  scenario with configurable rider/driver counts (default 500 / 100), 4h request
  window, match radius 5, trip duration 5–60 cells, and a simulation end time
  of 6h (request window + 2h buffer for in-flight trips). The end time is
  required because recurring events (batch matching) keep the queue non-empty;
  without it `run_until_empty` would never terminate. Runs until the event queue
  is empty or the end time is reached (up to 2M steps) and prints steps executed,
  simulation time, completed trip count, and up to 100 sample completed trips
  (time_to_match, time_to_pickup, trip_duration, completed_at in seconds).
- Set `SIM_EXPORT_DIR=/path` to export `completed_trips.parquet`, `trips.parquet` (all trips with full details, same as UI table), `snapshot_counts.parquet`, and `agent_positions.parquet`.
- **`scenario_run_large`** (`cargo run -p sim_core --example scenario_run_large --release`): Large-scale
  scenario with 10,000 riders / 7,000 drivers over a 4h simulation window with 15% commission rate
  and surge pricing (radius 2, max multiplier 1.3x). Reports detailed performance metrics: wall-clock
  time, events per second (~12,200 events/sec in release build), outcome breakdown (completed/cancelled/abandoned
  with reasons), timing distributions (time to match, time to pickup, trip duration with avg/p50/p90/p99/max),
  and event processing summary. Uses `EventMetrics` for throughput tracking. Recommended to run with
  `--release` for realistic performance numbers. Supports `SIM_EXPORT_DIR` for Parquet export.

## Known Gaps (Not Implemented Yet)

- Opportunity cost and driver-value weighting in matching.
- Driver acceptance models and rider conversion.
- Event scheduling after match beyond fixed delays (e.g. variable trip duration).
- Distributed experimentation (coordinator/worker execution beyond single-machine sweeps).
