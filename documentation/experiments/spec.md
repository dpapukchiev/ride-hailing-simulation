# Experiments

## `sim_experiments`

Parallel experimentation framework for parameter sweeps and marketplace health analysis.

- **`ParameterSpace`**: Defines parameter spaces for exploration (grid search, random sampling). Supports varying pricing parameters (commission rate, base fare, per-km rate, surge settings including `surge_radius_k`), supply/demand (num_riders, num_drivers), matching configuration (matching algorithm type, batch matching enabled/interval, ETA weight), simulation timing (epoch_ms, simulation_duration_hours), and other configuration parameters. Invalid combinations (e.g., Hungarian matching without batch matching) are automatically filtered out.
- **`parameter_spaces`**: Pre-defined parameter space configurations for common experiment types:
  - `comprehensive_space()`: Explores all major dimensions (pricing, supply/demand, matching algorithms, timing)
  - `pricing_focused_space()`: Pricing analysis with fixed supply/demand and matching configuration
  - `matching_focused_space()`: Matching algorithm comparison with fixed pricing
  - `supply_demand_space()`: Supply/demand analysis with fixed pricing and matching
  - `minimal_space()`: Quick testing with minimal parameter variations
- **`ParameterSet`**: Wraps `ScenarioParams` with experiment metadata (experiment ID, run ID, seed) for tracking and reproducibility.
- **`run_parallel_experiments`**: Executes multiple simulations in parallel using rayon. Each simulation runs independently with no shared state. Defaults to using all available CPU cores but allows specifying thread count.
- **`SimulationResult`**: Aggregated metrics extracted from completed simulations:
  - Conversion rate (completed / total resolved)
  - Platform revenue and driver payouts
  - Timing statistics (average/median/P90 for time to match and time to pickup)
  - Abandoned rides breakdown (price, ETA, stochastic)
- **`HealthWeights`**: Configurable weights for marketplace health score calculation. Default weights: conversion 30%, revenue 25%, driver payouts 15%, time to match 15%, time to pickup 15%, abandoned penalty -20%.
- **`calculate_health_scores`**: Calculates weighted health scores by normalizing metrics across all results and applying weights. Higher scores indicate healthier marketplace outcomes.
- **`export_to_parquet`** / **`export_to_json`**: Export experiment results for external analysis.
- **`find_best_parameters`**: Finds parameter set with highest health score.

**Dependencies**: `sim_core`, `rayon` (parallel execution), `serde`/`serde_json` (serialization), `arrow`/`parquet` (export).

**Usage**: Define parameter space (or use pre-defined spaces from `parameter_spaces`), generate parameter sets, run parallel experiments, calculate health scores, export results. See `examples/parameter_sweep.rs` for complete example using pre-defined parameter spaces.

## AWS Serverless Sweep Path

The repository also includes a deployed AWS serverless sweep path for distributed shard execution.

- **Ingress + orchestration**: API Gateway `POST /sweep-run` invokes a unified runtime Lambda that validates requests, computes deterministic shard plans, and enqueues shard messages to SQS.
- **Runtime crates**:
  - `crates/sim_serverless_sweep_core`: shared contract types, request validation, shard planning, and storage key conventions
  - `crates/sim_serverless_sweep_lambda`: unified runtime handlers plus AWS adapter boundaries
- **Infrastructure wiring**: `infra/aws_serverless_sweep/terraform` provisions API Gateway, Lambda, SQS, IAM, and environment wiring only.
- **Outcome storage**: Queue-driven worker execution writes partitioned Parquet datasets to S3 for Athena analytics (shard outcomes, shard metrics, trip data, snapshot counts).
- **Retry idempotency**: `run_date` is assigned once during parent dispatch and propagated in each shard payload so SQS retries/DLQ redrives keep writing to the same `run_id`/`shard_id` partition.
- **Operator docs**:
  - Deploy and infra details: `infra/aws_serverless_sweep/README.md`
  - Deploy/invoke/verify/rollback runbook: `documentation/experiments/serverless-sweep-runbook.md`
