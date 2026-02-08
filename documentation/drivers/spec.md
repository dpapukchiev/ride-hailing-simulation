# Drivers

## ECS Components

- `DriverState`: `Idle`, `Evaluating`, `EnRoute`, `OnTrip`, `OffDuty`
- `Driver` component: `{ state: DriverState, matched_rider: Option<Entity>, assigned_trip: Option<Entity> }`
  - `assigned_trip`: backlink to the active Trip entity. Enables O(1) trip lookup.
- `DriverEarnings` component: `{ daily_earnings: f64, daily_earnings_target: f64, session_start_time_ms: u64, session_end_time_ms: Option<u64> }`
  - Tracks accumulated earnings for the current day, earnings target at which driver goes OffDuty, session start time for fatigue calculation, and session end time (set when the driver goes OffDuty, `None` while active).
- `DriverFatigue` component: `{ fatigue_threshold_ms: u64 }`
  - Maximum time on duty (in milliseconds) before driver goes OffDuty.

## `sim_core::systems::driver_decision`

System: `driver_decision_system`

- Reacts to `CurrentEvent`.
- On `EventKind::DriverDecision` with subject `Driver(driver_entity)`:
  - Calculates a logit score based on trip and driver characteristics:
    - **Fare**: Higher fare increases acceptance probability
    - **Pickup distance**: Longer pickup distance decreases acceptance probability
    - **Trip distance**: Longer trips increase acceptance probability (more earnings)
    - **Earnings progress**: Drivers closer to earnings target are less likely to accept (diminishing returns)
    - **Fatigue**: More fatigued drivers are less likely to accept
  - Score formula: `score = base_acceptance_score + (fare × fare_weight) + (pickup_distance_km × pickup_distance_penalty) + (trip_distance_km × trip_distance_bonus) + (earnings_progress × earnings_progress_weight) + (fatigue_ratio × fatigue_penalty)`
  - Converts score to probability using logit function: `probability = 1 / (1 + exp(-score))`
  - Samples stochastically using seeded RNG (seed: `driver_decision_config.seed + driver_entity_id`)
  - Applies logit accept rule:
    - Accept: `Evaluating` → `EnRoute`, **spawns a Trip entity bundle** (`Trip` + `TripTiming` + `TripFinancials` + `TripLiveData`) with `pickup` =
      rider's position, `dropoff` = rider's `destination` or a neighbor of pickup,
      `requested_at` = rider's `requested_at`, `matched_at` = clock.now(), `pickup_at` = None;
      schedules `MoveStep` 1 second from now (`schedule_in_secs(1, ...)`) for that trip (`subject: Trip(trip_entity)`).
    - Reject: `Evaluating` → `Idle`, clears `matched_rider`. Schedules `MatchRejected` at delta 0 for the rider, which delegates rider-side cleanup (clearing `matched_driver`, rescheduling `TryMatch`) to `match_rejected_system`.

## `sim_core::systems::driver_offduty`

System: `driver_offduty_check_system`

- Reacts to `CurrentEvent`.
- On `EventKind::CheckDriverOffDuty`:
  - Periodically checks all active drivers (not already OffDuty) for earnings targets and fatigue thresholds, including drivers in `EnRoute` or `OnTrip`, so that limits are enforced on the 5-minute tick and drivers cannot exceed them by staying in back-to-back trips between checks.
  - For each driver (excluding only those already OffDuty):
    - Checks if `daily_earnings >= daily_earnings_target` (earnings target reached).
    - Checks if `session_duration_ms >= fatigue_threshold_ms` (fatigue threshold exceeded).
  - Transitions drivers to `OffDuty` if either threshold is exceeded. A driver marked OffDuty while `EnRoute` or `OnTrip` still finishes the current trip (movement and trip completion are unchanged); they simply receive no new matches afterward.
  - Always schedules the next check in 5 minutes (`CHECK_INTERVAL_MS`) to ensure newly spawned drivers are checked even if all current drivers are OffDuty.

See [CONFIG.md](../../CONFIG.md#driver-behavior) for OffDuty transition rules and threshold formulas.
- The first `CheckDriverOffDuty` event is scheduled by `simulation_started_system` during simulation initialization.
