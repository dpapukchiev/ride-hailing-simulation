# sim_experiments

Parallel experimentation framework for ride-hailing simulation parameter sweeps.

This crate enables running multiple simulations in parallel with varying parameters, extracting comprehensive metrics, and calculating marketplace health scores to analyze how pricing changes and supply/demand balance affect marketplace outcomes.

## Overview

The `sim_experiments` crate provides:

- **Parameter Variation**: Define parameter spaces (grid search, random sampling) to systematically explore configurations
- **Parallel Execution**: Run multiple simulations concurrently using rayon for CPU-bound parallelism
- **Metrics Extraction**: Extract comprehensive metrics including conversion rates, revenue, driver payouts, and timing statistics
- **Health Scoring**: Calculate weighted marketplace health scores combining multiple metrics
- **Result Export**: Export results to Parquet/JSON for external analysis

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
sim_experiments = { path = "../sim_experiments" }
```

## Quick Start

```rust
use sim_experiments::{ParameterSpace, run_parallel_experiments, HealthWeights, find_best_result_index};

// Define parameter space (grid search)
let space = ParameterSpace::grid()
    .commission_rate(vec![0.0, 0.1, 0.2, 0.3])
    .num_drivers(vec![50, 100, 150])
    .num_riders(vec![300, 500, 700]);

// Generate parameter sets
let parameter_sets = space.generate();

// Run experiments in parallel
let results = run_parallel_experiments(parameter_sets, None);

// Calculate health scores and find best result
let weights = HealthWeights::default();
let best_idx = find_best_result_index(&results, &weights).unwrap();
println!("Best result: {:?}", results[best_idx]);
```

## API Overview

### Parameter Space Definition

```rust
use sim_experiments::ParameterSpace;

// Grid search: all combinations
let space = ParameterSpace::grid()
    .commission_rate(vec![0.0, 0.1, 0.2, 0.3])
    .num_drivers(vec![50, 100, 150])
    .num_riders(vec![300, 500, 700])
    // Optional: vary simulation start time and duration
    .epoch_ms(vec![Some(1700000000000), Some(1700086400000)]) // Unix timestamp in ms
    .simulation_duration_hours(vec![Some(4), Some(8), Some(12)]); // Duration in hours

let parameter_sets = space.generate(); // Cartesian product

// Random sampling: Monte Carlo style
let space = ParameterSpace::grid()
    .commission_rate(vec![0.0, 0.1, 0.2, 0.3])
    .num_drivers(vec![50, 100, 150]);

let parameter_sets = space.sample_random(100, 42); // 100 random samples
```

#### Available Parameters

The `ParameterSpace` supports varying the following parameters:

- **Pricing**: `commission_rate()`, `base_fare()`, `per_km_rate()`, `surge_enabled()`, `surge_max_multiplier()`
- **Supply/Demand**: `num_drivers()`, `num_riders()`, `match_radius()`
- **Simulation Timing**: `epoch_ms()` (start datetime as Unix timestamp in milliseconds), `simulation_duration_hours()` (simulation duration in hours)

When `simulation_duration_hours` is specified, the simulation end time is automatically calculated as `request_window_ms + duration_hours * 3600000`. If not specified, the runner will use a default buffer.

**Example with timing parameters**:
```rust
use sim_experiments::ParameterSpace;

// Convert datetime to epoch_ms (you can use sim_ui::datetime_to_unix_ms or provide directly)
let space = ParameterSpace::grid()
    .epoch_ms(vec![
        Some(1700000000000),  // 2023-11-15 00:00:00 UTC
        Some(1700086400000),  // 2023-11-16 00:00:00 UTC
    ])
    .simulation_duration_hours(vec![Some(4), Some(8)]); // 4 or 8 hour simulations
```

### Running Experiments

```rust
use sim_experiments::run_parallel_experiments;

// Use all available CPU cores
let results = run_parallel_experiments(parameter_sets, None);

// Use specific number of threads
let results = run_parallel_experiments(parameter_sets, Some(4));
```

### Health Scoring

```rust
use sim_experiments::{HealthWeights, calculate_health_scores};

// Default weights (conversion: 30%, revenue: 25%, etc.)
let weights = HealthWeights::default();

// Custom weights
let weights = HealthWeights::new(
    0.4,  // conversion_weight
    0.3,  // revenue_weight
    0.1,  // driver_payouts_weight
    0.1,  // time_to_match_weight
    0.1,  // time_to_pickup_weight
    -0.2, // abandoned_penalty
);

let scores = calculate_health_scores(&results, &weights);
```

### Exporting Results

```rust
use sim_experiments::{export_to_json, export_to_parquet};

// Export to JSON (human-readable)
export_to_json(&results, "results.json")?;

// Export to Parquet (efficient, compatible with Pandas/Polars)
export_to_parquet(&results, "results.parquet")?;
```

## Examples

See `examples/parameter_sweep.rs` for a complete example demonstrating:

- Parameter space definition
- Parallel execution
- Health score calculation
- Finding optimal parameters
- Result export

Run with:

```bash
cargo run --example parameter_sweep -p sim_experiments
```

## Scaling to Multiple Machines

The current implementation uses rayon for single-machine parallelism, which is efficient for CPU-bound simulation work. For distributed execution across multiple machines, the architecture would use a coordinator/worker model:

**Architecture**: Coordinator/worker model with HTTP-based task distribution. The coordinator runs a tokio HTTP server that distributes `ParameterSet` tasks to worker nodes. Workers poll for tasks via HTTP, run simulations locally using rayon for parallelism, and return results. This design separates networking concerns (tokio) from CPU-bound work (rayon), allowing the simulation code to remain synchronous with minimal changes.

**Future Structure**: When scaling becomes necessary, add `distributed/coordinator.rs` (task queue and distribution), `distributed/worker.rs` (client that polls and executes), and `distributed/protocol.rs` (message types). The current rayon-based implementation provides a solid foundation that can be wrapped in a network layer without refactoring core simulation logic.

## Metrics

The `SimulationResult` struct includes:

- **Conversion**: Completed riders / total riders
- **Revenue**: Platform revenue from commissions
- **Driver Payouts**: Total driver earnings
- **Timing**: Average/median/P90 time to match and time to pickup
- **Abandoned Rides**: Breakdown by reason (price, ETA, stochastic)

## Health Score Formula

The marketplace health score is a weighted sum of normalized metrics:

```
health_score = 
    conversion_norm × conversion_weight +
    revenue_norm × revenue_weight +
    payouts_norm × driver_payouts_weight +
    (1 - match_time_norm) × time_to_match_weight +  // inverted: lower is better
    (1 - pickup_time_norm) × time_to_pickup_weight + // inverted: lower is better
    (1 - abandoned_norm) × abandoned_penalty        // inverted: lower is better
```

Metrics are normalized to [0, 1] using min-max normalization across all results.

## License

This project is for demonstration purposes.
