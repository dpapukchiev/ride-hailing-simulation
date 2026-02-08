# Matching

## `sim_core::matching`

Matching algorithm trait and implementations for driver-rider pairing.

- **`MatchingAlgorithm` trait**: Interface for matching algorithms with two methods:
  - `find_match(rider_entity, rider_pos, rider_destination, available_drivers, match_radius, clock_now_ms) -> Option<Entity>`: Finds a match for a single rider, returns the best driver entity or `None`.
  - `find_batch_matches(riders, available_drivers, match_radius, clock_now_ms) -> Vec<MatchResult>`: Finds matches for multiple riders (batch optimization). Default implementation calls `find_match` sequentially; algorithms can override for global optimization.
- **`SimpleMatching`**: First-match-within-radius algorithm. Finds the first available driver within `MatchRadius` H3 grid distance. Preserves original "first match wins" behavior.
- **`CostBasedMatching`**: Cost-based algorithm that scores driver-rider pairings by pickup distance and estimated pickup time. Selects the driver with the highest score (lowest cost). Configurable `eta_weight` parameter (default 0.1) controls ETA importance vs distance.
- **`HungarianMatching`**: Global batch optimization using Kuhn–Munkres (Hungarian) algorithm. Uses the same score formula as CostBasedMatching; overrides `find_batch_matches` to solve the assignment problem (minimize total cost). Single-rider `find_match` delegates to CostBasedMatching. Default algorithm when batch matching is enabled.
- **`MatchResult`**: Represents a successful match with `rider_entity` and `driver_entity`.
- **`MatchCandidate`**: Represents a potential pairing with scoring information (used internally by algorithms).

See [CONFIG.md](../../CONFIG.md#matching-algorithms) for detailed scoring formulas, ETA weight tuning, and algorithm selection guidance.

## `sim_core::systems::matching`

System: `matching_system`

- Reacts to `CurrentEvent`. When **batch matching is enabled** (via `BatchMatchingConfig`), this system does nothing (per-rider matching is not used).
- On `EventKind::TryMatch` with subject `Rider(rider_entity)` (only when batch matching disabled):
  - If that rider has the `Waiting` marker, queries the `MatchingAlgorithm` resource (required) to find a match.
  - Collects all drivers with the `Idle` marker with their positions as candidates (excludes `OffDuty` drivers).
  - Calls `find_match()` on the algorithm with the rider and available drivers.
  - If a match is found:
    - Rider stores `matched_driver = Some(driver_entity)`
    - Driver: `Idle` → `Evaluating` via `DriverStateCommands::set_driver_state_evaluating()` and stores `matched_rider = Some(rider_entity)`
    - Schedules `MatchAccepted` 1 second from now (`schedule_in_secs(1, ...)`) with subject `Driver(driver_entity)`.
  - If no driver is found, reschedules `TryMatch` after a short delay (30s).

## `sim_core::systems::batch_matching`

System: `batch_matching_system`

- Reacts to `CurrentEvent`.
- On `EventKind::BatchMatchRun` (no subject; global event):
  - When `BatchMatchingConfig` is present and enabled: collects all riders in `Waiting` with `matched_driver == None`, and all `Idle` drivers; calls `find_batch_matches()` on the matching algorithm; for each `MatchResult`, sets rider `matched_driver`, driver `matched_rider`, transitions the driver to `Evaluating` via `DriverStateCommands`, and schedules `MatchAccepted` 1s later for the driver. Schedules the next `BatchMatchRun` at `now + interval_secs`. Unmatched riders remain waiting for the next batch.

## `sim_core::systems::match_accepted`

System: `match_accepted_system`

- Reacts to `CurrentEvent`.
- On `EventKind::MatchAccepted` with subject `Driver(driver_entity)`:
  - Schedules `DriverDecision` 1 second from now (`schedule_in_secs(1, ...)`) for the same driver.

## `sim_core::systems::match_rejected`

System: `match_rejected_system`

- Reacts to `CurrentEvent`.
- On `EventKind::MatchRejected` with subject `Rider(rider_entity)`:
  - Handles rider-side cleanup after a driver rejects the match (scheduled by `driver_decision_system` at delta 0).
  - Clears the rider's `matched_driver` link.
  - If batch matching is **disabled** and rider still has the `Waiting` marker, schedules `TryMatch` after 30 seconds so the rider can be re-matched.
  - If batch matching is **enabled**, no retry is scheduled; the rider remains waiting and is included in the next `BatchMatchRun`.

