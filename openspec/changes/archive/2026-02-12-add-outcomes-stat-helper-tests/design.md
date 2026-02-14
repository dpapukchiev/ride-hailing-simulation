## Context

`sim_ui` outcomes rendering computes timing and fare statistics through local helper functions in `crates/sim_ui/src/ui/controls/outcomes.rs`. These helpers are deterministic and side-effect free, but currently lack unit tests, leaving edge-case behavior implicit.

## Goals / Non-Goals

**Goals:**
- Add focused unit tests for timing distribution and percentile helpers.
- Make edge-case behavior explicit (empty input and invalid percentile).
- Preserve existing runtime behavior.

**Non-Goals:**
- No UI layout or text changes.
- No algorithm redesign for percentile math.
- No cross-crate refactors or telemetry model changes.

## Decisions

### Decision 1: Keep tests in `outcomes.rs` under a local `#[cfg(test)]` module

Tests will live beside the helper functions so behavior and assertions remain easy to maintain during future refactors. This avoids introducing a separate integration test harness for simple pure-function validation.

### Decision 2: Construct minimal `CompletedTripRecord` fixtures with deterministic values

Timing assertions need concrete trip records. Tests will use compact fixture builders with controlled timestamp fields so `time_to_match`, `time_to_pickup`, and `trip_duration` calculations are straightforward to verify.

### Decision 3: Validate nearest-rank behavior exactly as implemented

Percentile tests will assert current index math (`idx = p * (n - 1) / 100`) to lock in existing contract and prevent accidental behavior drift.
