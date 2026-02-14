## 1. Outcomes helper tests (`crates/sim_ui/src/ui/controls/outcomes.rs`)

- [x] 1.1 Add a `#[cfg(test)]` module with minimal `CompletedTripRecord` fixture helpers.
- [x] 1.2 Add tests for `timing_distribution` on empty and non-empty trip inputs (min/mean/max + sorted output).
- [x] 1.3 Add tests for `TimingDistribution::percentile` covering valid percentiles and invalid `p > 100`.
- [x] 1.4 Add tests for `percentile_f64_sorted` covering empty, valid, and invalid percentile requests.

## 2. Verify

- [x] 2.1 Run `cargo test -p sim_ui` and ensure new tests pass.
