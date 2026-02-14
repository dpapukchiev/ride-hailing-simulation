## Why

The outcomes control panel in `sim_ui` displays distribution and percentile metrics that rely on helper functions with no direct unit coverage. Adding focused tests now reduces regression risk in user-visible statistics and documents expected behavior for edge cases.

## What Changes

- Add unit tests for `timing_distribution`, `TimingDistribution::percentile`, and `percentile_f64_sorted` in the outcomes module.
- Cover empty input, single-value input, multi-value input, and invalid percentile inputs (`p > 100`).
- Keep runtime behavior unchanged; this change improves confidence and maintainability through test coverage.

## Capabilities

### New Capabilities
- `outcomes-stats-validation`: Defines expected statistical helper behavior used by outcomes UI metrics, including percentile and distribution edge cases.

### Modified Capabilities

## Impact

- `crates/sim_ui/src/ui/controls/outcomes.rs`: add a unit test module for helper functions.
