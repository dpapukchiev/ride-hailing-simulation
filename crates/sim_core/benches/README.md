# Performance Benchmarks

This directory contains performance benchmarks for sim_core using Criterion.rs.

## Running Benchmarks

```bash
# Run all benchmarks
cargo bench --package sim_core

# Run specific benchmark group
cargo bench --package sim_core --bench performance simulation_run

# Compare against previous run (automatic)
cargo bench --package sim_core
```

## Baseline Storage

Criterion.rs automatically stores baseline data in `target/criterion/` (git-ignored). Each benchmark run creates a baseline that future runs compare against.

**Note**: The `target/` directory is git-ignored, so baseline data is local only. This is intentional - baseline comparisons work automatically on your local machine.

If you need to share baseline metrics across machines, you can:
1. Export summary statistics manually
2. Use Criterion's HTML reports (in `target/criterion/`)
3. Document baseline performance in commit messages or documentation

## Viewing Results

HTML reports are generated in `target/criterion/<benchmark_name>/report/index.html`. Open these in a browser to see detailed performance comparisons.

## Benchmark Groups

- `simulation_run`: Full simulation runs (small/medium/large scenarios)
- `matching_algorithms`: Matching algorithm performance (simple/cost-based/Hungarian)
