# Ride-Hailing Simulation

A high-performance, discrete-event simulation of a ride-hailing marketplace built in Rust. This project demonstrates advanced simulation techniques, spatial indexing, and real-time visualization for modeling complex multi-agent systems.

## Overview

This simulation models a ride-hailing marketplace (similar to Uber/Bolt) with realistic demand and supply patterns, driver-rider matching algorithms, and comprehensive telemetry. It's built using:

- **Discrete Event Simulation (DES)**: Millisecond-precision event scheduling with deterministic execution
- **Entity Component System (ECS)**: Efficient multi-agent state management using Bevy ECS
- **H3 Spatial Indexing**: Geographic indexing for efficient spatial queries and matching
- **Real-time Visualization**: Native UI built with egui for live simulation monitoring

The simulation supports hundreds of concurrent riders and drivers, realistic time-of-day demand patterns (rush hours, day/night variations), and multiple matching algorithms for driver-rider pairing.

## Features

### Core Simulation
- **Discrete Event System**: Binary heap-based event queue with deterministic ordering
- **Dynamic Spawning**: Time-of-day aware spawners for riders and drivers with configurable distributions
- **State Machine**: Complete lifecycle modeling (browsing → quote accept/reject → waiting → matched → en route → completed/cancelled). Riders can reject quotes and request another, or give up after N rejections.
- **Driver Behavior**: Earnings targets, fatigue thresholds, and off-duty transitions
- **Pricing System**: Distance-based fare calculation with configurable base fare, per-km rate, and commission rates. Tracks driver earnings and platform revenue separately.

### Matching Algorithms
- **Simple Matching**: First-available driver within radius
- **Cost-Based Matching**: Optimizes for pickup distance and estimated time of arrival (ETA)
- **Extensible**: Trait-based design allows easy addition of new algorithms (e.g., bipartite matching)

### Spatial Features
- **H3 Grid System**: Resolution 9 (~240m cell size) for efficient spatial operations
- **Configurable Match Radius**: Match riders to drivers within N H3 cells
- **Haversine Distance**: Accurate distance calculations for trip pricing

### Visualization & Analytics
- **Real-time Map**: Live visualization of riders and drivers with state-based coloring
- **Time-series Charts**: Track active trips, waiting riders, idle drivers, cancellations, abandoned (quote), completed and cancelled trips
- **Trip Table**: Detailed trip information with timestamps and distances
- **Export**: Parquet export for completed trips, snapshots, and agent positions

### Realistic Patterns
- **Time-of-Day Distributions**: Rush hour multipliers (7-9 AM, 5-7 PM peak demand)
- **Day-of-Week Variations**: Different patterns for weekdays vs. weekends
- **Driver Supply Patterns**: More consistent than demand, with rush hour availability spikes

## Architecture

### High-Level Design

```
┌─────────────────┐
│   Simulation    │
│     Clock       │ ← Binary heap event queue
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Event Router   │ ← Pops events, inserts CurrentEvent
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│   ECS Systems   │ ← React to CurrentEvent, mutate entities
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Telemetry &    │ ← Snapshot capture, Parquet export
│    Export       │
└─────────────────┘
```

### Key Components

**Clock (`sim_core::clock`)**
- Millisecond-precision timeline
- Binary heap for O(log n) event insertion/removal
- Deterministic ordering for same-timestamp events
- Epoch support for real-world datetime mapping

**ECS (`sim_core::ecs`)**
- Components: `Rider`, `Driver`, `Trip`, `Position`
- States: `RiderState`, `DriverState`, `TripState`
- Resources: `SimulationClock`, `MatchRadius`, `MatchingAlgorithm`

**Systems (`sim_core::systems`)**
- Event-driven: React to `CurrentEvent` resource
- Targeted: Use `EventSubject` to target specific entities
- Modular: Each system handles one aspect (matching, movement, etc.)

**Spatial (`sim_core::spatial`)**
- H3 resolution 9 wrapper
- Grid disk queries for radius-based matching
- Haversine distance calculations

**Matching (`sim_core::matching`)**
- Trait-based algorithm interface
- Pluggable implementations
- Batch matching support (for future global optimization)

## Usage

### Setup

This project uses `mise` for toolchain management:

```sh
# Install Rust toolchain
mise install

# Activate mise in your shell (if not already active)
mise activate
```

### Running the Simulation

**Command-line example:**
```sh
# Run tests
cargo test -p sim_core

# Run example scenario (500 riders, 100 drivers, 4 hours)
cargo run -p sim_core --example scenario_run

# Export to Parquet (optional)
SIM_EXPORT_DIR=/path/to/export cargo run -p sim_core --example scenario_run
```

**Interactive UI:**
```sh
# Launch visualization UI
cargo run -p sim_ui
```

The UI provides:
- Real-time map visualization with state-based coloring and optional grid overlay
- Interactive parameter adjustment organized in five columns:
  - Supply (drivers: initial, spawn count, spread)
  - Demand (riders: initial, spawn count, spread, cancel wait)
  - Pricing & Matching (base fare, per km rate, commission rate, matching algorithm, match radius)
  - Map & Trips (map size, trip length range)
  - Timing (simulation start time, seed)
- Time-series charts (active trips, waiting riders, idle drivers, cancellations, abandoned quotes, completed/cancelled trips)
- Trip detail table (all trips with timestamps, distances, and states)
- Playback controls (start, step, step 100, run/pause, run to end, reset, speed multiplier 10x-200x)
- Fleet metrics (utilization, earnings distributions, fatigue tracking)
- Run outcomes (conversion rates, timing distributions with percentiles, platform revenue)

### Example: Custom Scenario

```rust
use sim_core::pricing::PricingConfig;
use sim_core::scenario::{build_scenario, ScenarioParams};
use sim_core::runner::{initialize_simulation, run_until_empty, simulation_schedule};

let mut world = World::new();
build_scenario(
    &mut world,
    ScenarioParams::default()
        .with_seed(42)
        .with_request_window_hours(6)
        .with_match_radius(5)
        .with_trip_duration_cells(5, 60)
        .with_epoch_ms(1700000000000) // Custom start time
        .with_pricing_config(PricingConfig {
            base_fare: 2.50,
            per_km_rate: 1.50,
            commission_rate: 0.15, // 15% commission
        }),
);

initialize_simulation(&mut world);
let mut schedule = simulation_schedule();
let steps = run_until_empty(&mut world, &mut schedule, 1_000_000);
```

## Performance

The simulation is optimized for scale and performance:

- **Tested Scale**: 500 riders, 100 drivers over 4+ hours
- **Event Throughput**: Processes millions of events efficiently
- **Memory**: Efficient ECS storage with minimal allocations
- **Deterministic**: Reproducible results with seeded RNG

**Optimizations:**
- Binary heap for O(log n) event scheduling
- H3 spatial indexing for O(1) cell lookups
- ECS component storage for cache-friendly access patterns
- Rejection sampling for large trip distances (avoids generating huge grid disks)

## Technical Highlights

### Rust-Specific Features
- **Zero-cost abstractions**: ECS queries compile to efficient code
- **Type safety**: Strong typing prevents common simulation bugs
- **Memory safety**: No unsafe code in core simulation logic
- **Error handling**: Result types for fallible operations

### Design Patterns
- **Strategy Pattern**: Pluggable matching algorithms via traits
- **Observer Pattern**: Event-driven system reactions
- **Builder Pattern**: Fluent API for scenario configuration
- **Resource Pattern**: ECS resources for global state

### Code Quality
- **Comprehensive tests**: Unit tests for all systems, end-to-end integration tests
- **Documentation**: Inline docs with examples for public APIs
- **Linting**: Clippy with strict warnings enabled
- **Formatting**: Consistent rustfmt configuration

## Project Structure

```
ride-hailing-simulation/
├── crates/
│   ├── sim_core/          # Core simulation engine
│   │   ├── src/
│   │   │   ├── clock.rs   # Event scheduling
│   │   │   ├── ecs.rs     # Components and states
│   │   │   ├── systems/   # Event-driven systems
│   │   │   ├── matching/  # Matching algorithms
│   │   │   ├── spatial.rs # H3 spatial operations
│   │   │   └── ...
│   │   └── examples/
│   │       └── scenario_run.rs
│   └── sim_ui/            # Visualization UI
│       └── src/
│           ├── app.rs     # Application state
│           ├── ui/        # UI modules
│           └── main.rs    # Entry point
├── SPEC.md                # Detailed specification
└── README.md              # This file
```

## Dependencies

**Core:**
- `bevy_ecs`: Entity Component System
- `h3o`: H3 geospatial indexing
- `rand`: Random number generation
- `arrow` + `parquet`: Data export

**UI:**
- `eframe`: Native window framework
- `egui`: Immediate mode GUI
- `egui_plot`: Time-series plotting

## Future Enhancements

Potential improvements (not yet implemented):
- Batch matching system with global optimization
- Advanced matching algorithms (Hungarian algorithm, opportunity cost)
- Driver acceptance models and rider conversion
- H3-based routing for realistic movement
- Replay system for saved simulations
- CSV export option (currently Parquet only)

## Contributing

This is a portfolio project demonstrating simulation and Rust expertise. The codebase follows strict quality standards:

- All code must compile without warnings
- Public APIs must be documented
- Tests required for new features
- Clippy pedantic lints enabled

## License

This project is for demonstration purposes.

## Screenshots

*Note: Add screenshots of the UI showing:*
- *Map visualization with riders and drivers*
- *Time-series charts*
- *Trip detail table*

## Acknowledgments

Built as a demonstration of discrete-event simulation, spatial algorithms, and Rust systems programming. Inspired by real-world ride-hailing marketplace dynamics.
