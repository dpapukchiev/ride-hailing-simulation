# Configuration & Formulas Reference

This document provides a comprehensive reference for all configuration parameters, formulas, and rules used in the simulation. It clearly distinguishes between **random/stochastic** elements and **deterministic/hard-coded** rules.

## Table of Contents

- [Pricing Configuration](#pricing-configuration)
- [Spawner Configuration & Patterns](#spawner-configuration--patterns)
- [Matching Algorithms](#matching-algorithms)
- [Driver Behavior](#driver-behavior)
- [Rider Behavior](#rider-behavior)
- [Movement & Speed](#movement--speed)
- [System Timing](#system-timing)
- [Spatial Configuration](#spatial-configuration)

---

## Pricing Configuration

### Configuration Parameters

| Parameter | Default | Type | Description |
|-----------|---------|------|-------------|
| `base_fare` | 2.50 | f64 | Base fare in currency units (e.g., dollars) |
| `per_km_rate` | 1.50 | f64 | Per-kilometer rate in currency units |
| `commission_rate` | 0.0 | f64 | Commission rate as fraction (0.0-1.0). 0.15 = 15% commission |
| `surge_enabled` | false | bool | When true, apply surge multiplier when demand > supply |
| `surge_radius_k` | 1 | u32 | H3 grid disk radius (k) for surge cluster around pickup |
| `surge_max_multiplier` | 2.0 | f64 | Maximum surge multiplier cap (e.g., 2.0 = 2x base fare) |

### Formulas

#### Base Fare Calculation
**Deterministic** (based on distance)

```
fare_base = base_fare + (distance_km × per_km_rate)
```

Where:
- `distance_km` = Haversine distance between pickup and dropoff H3 cells (km)

#### Surge Multiplier Calculation
**Deterministic** (based on local supply/demand counts)

When `surge_enabled = true`:

```
demand = count(riders in Browsing or Waiting state within grid_disk(pickup, surge_radius_k))
supply = count(drivers in Idle state within grid_disk(pickup, surge_radius_k))

IF demand > supply AND supply > 0:
    multiplier = min(1.0 + (demand - supply) / supply, surge_max_multiplier)
ELSE IF demand > supply AND supply == 0:
    multiplier = surge_max_multiplier
ELSE:
    multiplier = 1.0

fare_final = fare_base × multiplier
```

#### Commission & Earnings
**Deterministic** (based on fare and commission rate)

```
commission = fare × commission_rate
driver_earnings = fare × (1.0 - commission_rate)
platform_revenue = commission
```

**Verification**: `driver_earnings + commission = fare` (always)

---

## Spawner Configuration & Patterns

### Configuration Parameters

| Parameter | Default | Type | Description |
|-----------|---------|------|-------------|
| `num_riders` | 500 | usize | Total number of riders to spawn |
| `num_drivers` | 100 | usize | Total number of drivers to spawn |
| `initial_rider_count` | 0 | usize | Riders spawned immediately at simulation start |
| `initial_driver_count` | 0 | usize | Drivers spawned immediately at simulation start |
| `request_window_ms` | 3,600,000 | u64 | Time window (ms) over which scheduled riders spawn (default: 1 hour) |
| `driver_spread_ms` | 3,600,000 | u64 | Time window (ms) over which scheduled drivers spawn (default: 1 hour) |
| `lat_min`, `lat_max` | 37.6, 37.85 | f64 | Geographic bounds (degrees) for spawn positions (default: SF Bay Area) |
| `lng_min`, `lng_max` | -122.55, -122.35 | f64 | Geographic bounds (degrees) for spawn positions |
| `min_trip_cells` | 5 | u32 | Minimum trip length in H3 cells (riders only) |
| `max_trip_cells` | 60 | u32 | Maximum trip length in H3 cells (riders only) |
| `epoch_ms` | 0 | i64 | Real-world time (ms) corresponding to simulation time 0 (for time-of-day patterns) |
| `seed` | None | Option<u64> | Random seed for reproducibility (if None, uses thread RNG) |

### Spawn Rate Calculation
**Deterministic** (based on target counts and time windows)

For riders:
```
scheduled_rider_count = num_riders - initial_rider_count
avg_rate_per_sec = scheduled_rider_count / (request_window_ms / 1000.0)
base_rate_per_sec = avg_rate_per_sec / RIDER_DEMAND_AVERAGE_MULTIPLIER
```

Where `RIDER_DEMAND_AVERAGE_MULTIPLIER = 1.3` (accounts for time-of-day variations)

For drivers:
```
scheduled_driver_count = num_drivers - initial_driver_count
avg_rate_per_sec = scheduled_driver_count / (driver_spread_ms / 1000.0)
base_rate_per_sec = avg_rate_per_sec / DRIVER_SUPPLY_AVERAGE_MULTIPLIER
```

Where `DRIVER_SUPPLY_AVERAGE_MULTIPLIER = 1.2` (accounts for time-of-day variations)

### Time-of-Day Patterns
**Deterministic** (hard-coded multipliers by hour and day of week)

#### Rider Demand Patterns

**Weekday Pattern** (Monday-Thursday, Sunday):
- **Rush Hours**: 7-9 AM (2.5-3.0x), 5-7 PM (2.8-3.2x)
- **Night**: 12 AM-6 AM (0.4x)
- **Daytime**: 10 AM-4 PM (1.2x)
- **Evening**: 8-11 PM (1.5x)

**Weekend Pattern** (Friday-Saturday):
- **Late Night**: 12 AM-4 AM (2.0-3.0x)
- **Morning**: 6-10 AM (0.8-1.5x)
- **Afternoon**: 10 AM-5 PM (0.4-0.5x)
- **Evening Rush**: 5-8 PM (2.8-3.5x)
- **Night**: 8 PM-12 AM (1.5-2.5x)

#### Driver Supply Patterns

**Weekday Pattern** (Monday-Thursday, Sunday):
- **Rush Hours**: 7-9 AM (1.5-1.8x), 5-7 PM (1.7-2.0x)
- **Night**: 12 AM-6 AM (0.6x)
- **Daytime**: 10 AM-4 PM (1.2x)
- **Evening**: 8 PM-12 AM (1.3x)

**Weekend Pattern** (Friday-Saturday):
- **Late Night**: 12 AM-6 AM (0.7-1.3x)
- **Morning**: 6-10 AM (1.2-1.6x)
- **Afternoon**: 10 AM-5 PM (0.7-1.0x)
- **Evening Rush**: 5-8 PM (1.7-2.2x)
- **Night**: 8 PM-12 AM (1.2-1.8x)

### Inter-Arrival Time Sampling
**Random** (exponential distribution, seeded for reproducibility)

```
current_multiplier = get_multiplier_for_current_hour_and_day(epoch_ms, current_time_ms)
adjusted_rate = base_rate_per_sec × current_multiplier
inter_arrival_ms = -ln(U) / adjusted_rate × 1000
```

Where:
- `U` = uniform random [0, 1) sampled from seeded RNG
- Seed = `scenario_seed + spawn_count` (for variety with reproducibility)

### Spawn Position & Destination Selection
**Random** (seeded for reproducibility)

- **Rider pickup position**: Random H3 cell within geographic bounds (lat/lng)
- **Rider destination**: Random H3 cell within `[min_trip_cells, max_trip_cells]` distance from pickup
  - Uses `grid_disk()` for small distances (≤20 cells)
  - Uses rejection sampling for large distances (>20 cells) for efficiency
- **Driver position**: Random H3 cell within geographic bounds

**Seed derivation**:
- Rider spawns: `seed + current_time_ms + spawn_count`
- Driver spawns: `seed + 0xdead_beef + current_time_ms + spawn_count`

---

## Matching Algorithms

### Configuration Parameters

| Parameter | Default | Type | Description |
|-----------|---------|------|-------------|
| `match_radius` | 0 | u32 | Max H3 grid distance for matching (0 = same cell only) |
| `batch_matching_enabled` | true | bool | When true, use batch matching instead of per-rider matching |
| `batch_interval_secs` | 5 | u64 | Interval (seconds) between batch matching runs |
| `eta_weight` | 0.1 | f64 | Weight for ETA in cost-based matching (default for Hungarian/CostBased) |

### Matching Algorithm Types

#### Simple Matching
**Deterministic** (first match within radius)

- Finds first available driver within `match_radius` H3 grid distance
- No scoring or optimization

#### Cost-Based Matching
**Deterministic** (distance + ETA scoring)

**Score Formula**:
```
pickup_distance_km = haversine_distance(rider_pos, driver_pos)
pickup_eta_ms = max(1000, (pickup_distance_km / 40.0) × 3600 × 1000)
score = -pickup_distance_km - (pickup_eta_ms / 1000.0) × eta_weight
```

Selects driver with **highest score** (lowest combined cost).

**ETA Weight Tuning**:
- `eta_weight = 0.0`: Pure distance-based (ignores ETA)
- `eta_weight = 0.1`: Default (distance slightly more important)
- `eta_weight = 1.0`: Equal weight for distance and ETA
- `eta_weight > 1.0`: ETA prioritized over distance

#### Hungarian Matching (Batch)
**Deterministic** (global optimization)

- Uses Kuhn–Munkres (Hungarian) algorithm for global batch optimization
- Uses same score formula as Cost-Based Matching
- Minimizes total cost across all rider-driver pairs
- Only used when `batch_matching_enabled = true`

---

## Driver Behavior

### Configuration Parameters

| Parameter | Default | Type | Description |
|-----------|---------|------|-------------|
| `daily_earnings_target` | **Random** $100-$300 | f64 | Target earnings before going OffDuty |
| `fatigue_threshold_ms` | **Random** 8-12 hours | u64 | Maximum time on duty before going OffDuty (ms) |

### Driver Earnings Target
**Random** (uniform distribution, seeded)

```
daily_earnings_target = random_uniform(100.0, 300.0)
```

Sampled when driver spawns, using seed `scenario_seed + driver_entity_id`.

### Driver Fatigue Threshold
**Random** (uniform distribution, seeded)

```
fatigue_threshold_ms = random_uniform(8_hours, 12_hours) × 3600 × 1000
```

Sampled when driver spawns, using seed `scenario_seed + driver_entity_id + 0xbeef`.

### Driver Decision (Accept/Reject Match)
**Deterministic** (logit accept rule)

Currently implemented as deterministic accept (all matches accepted). Future: logit model based on fare, distance, driver state.

### OffDuty Transitions
**Deterministic** (threshold checks)

Driver transitions to `OffDuty` when:
1. `daily_earnings >= daily_earnings_target` (earnings target reached)
2. `session_duration_ms >= fatigue_threshold_ms` (fatigue threshold exceeded)

Where `session_duration_ms = current_time_ms - session_start_time_ms`

**Note**: Drivers already `EnRoute` or `OnTrip` finish their current trip before going OffDuty.

---

## Rider Behavior

### Configuration Parameters

| Parameter | Default | Type | Description |
|-----------|---------|------|-------------|
| `max_quote_rejections` | 3 | u32 | Maximum quote rejections before rider gives up |
| `re_quote_delay_secs` | 10 | u64 | Delay (seconds) before requesting another quote after rejection |
| `accept_probability` | 0.8 | f64 | Probability (0.0-1.0) of accepting quote when within price/ETA limits |
| `max_willingness_to_pay` | 100.0 | f64 | Maximum fare rider will accept; reject if quote fare exceeds this |
| `max_acceptable_eta_ms` | 600,000 | u64 | Maximum acceptable ETA (ms); reject if quote ETA exceeds this (default: 10 min) |
| `min_wait_secs` | 120 | u64 | Minimum wait time (seconds) before pickup cancellation |
| `max_wait_secs` | 2400 | u64 | Maximum wait time (seconds) before pickup cancellation |

### Quote Decision Logic
**Random** (stochastic accept/reject, seeded for reproducibility)

```
IF quote.fare > max_willingness_to_pay:
    REJECT (deterministic)
ELSE IF quote.eta_ms > max_acceptable_eta_ms:
    REJECT (deterministic)
ELSE:
    accept = random_bool(accept_probability)  // Random, seeded
    IF accept:
        ACCEPT
    ELSE:
        REJECT
```

**Seed**: `rider_quote_config.seed + rider_entity_id` (for reproducibility with variety)

### Quote Rejection & Give-Up
**Deterministic** (counter-based)

```
IF quote_rejections < max_quote_rejections:
    quote_rejections++
    schedule ShowQuote after re_quote_delay_secs
ELSE:
    rider.state = Cancelled
    rider despawned
    riders_abandoned_quote_total++
```

### Pickup Cancellation Window
**Random** (uniform distribution, seeded)

```
cancellation_time_ms = random_uniform(min_wait_secs, max_wait_secs) × 1000
```

**Seed**: `rider_cancel_config.seed + rider_entity_id` (for reproducibility with variety)

Cancellation is scheduled when rider accepts quote and transitions to `Waiting` state.

---

## Movement & Speed

### Configuration Parameters

| Parameter | Default | Type | Description |
|-----------|---------|------|-------------|
| `min_kmh` | 20.0 | f64 | Minimum vehicle speed (km/h) |
| `max_kmh` | 60.0 | f64 | Maximum vehicle speed (km/h) |
| `h3_resolution` | Resolution::Nine | Resolution | H3 grid resolution (~0.24 km per cell at res 9) |

### Speed Sampling
**Random** (uniform distribution, seeded)

```
speed_kmh = random_uniform(min_kmh, max_kmh)
```

**Seed**: `speed_model_seed + trip_entity_id + movement_step_count` (for variety with reproducibility)

Speed is sampled per movement step (each H3 cell hop).

### Movement Calculation
**Deterministic** (based on distance and sampled speed)

```
distance_km = haversine_distance(current_cell, target_cell)
time_ms = (distance_km / speed_kmh) × 3600 × 1000
time_ms = max(time_ms, 1000)  // Minimum 1 second per hop
```

Driver moves one H3 cell per `MoveStep` event toward pickup (EnRoute) or dropoff (OnTrip).

### Pickup ETA Calculation
**Deterministic** (based on remaining distance and sampled speed)

```
remaining_distance_km = haversine_distance(driver_pos, pickup_pos)
pickup_eta_ms = (remaining_distance_km / speed_kmh) × 3600 × 1000
pickup_eta_ms = max(pickup_eta_ms, 1000)  // Minimum 1 second
```

Updated on each `MoveStep` during `EnRoute` phase.

---

## System Timing

### Configuration Parameters

| Parameter | Default | Type | Description |
|-----------|---------|------|-------------|
| `simulation_end_time_ms` | None | Option<u64> | Simulation stops when next event is at or after this time (ms) |
| `snapshot_interval_ms` | Configurable | u64 | Interval between telemetry snapshots |
| `check_driver_offduty_interval_ms` | 300,000 | u64 | Interval between OffDuty checks (default: 5 minutes) |

### Event Scheduling Delays
**Deterministic** (fixed delays)

| Event | Delay | Description |
|-------|-------|-------------|
| `ShowQuote` | 1 second | After rider spawns |
| `QuoteDecision` | 1 second | After quote is shown |
| `TryMatch` | 1 second | After quote accepted (if batch matching disabled) |
| `MatchAccepted` | 1 second | After match found |
| `DriverDecision` | 1 second | After match accepted |
| `MoveStep` | Calculated | Based on distance/speed for next H3 hop |
| `TripStarted` | 1 second | After driver reaches pickup |
| `TripCompleted` | 1 second | After driver reaches dropoff |
| `BatchMatchRun` | `batch_interval_secs` | Periodic batch matching (default: 5 seconds) |
| `CheckDriverOffDuty` | `check_driver_offduty_interval_ms` | Periodic checks (default: 5 minutes) |

---

## Spatial Configuration

### Configuration Parameters

| Parameter | Default | Type | Description |
|-----------|---------|------|-------------|
| `h3_resolution` | Resolution::Nine | Resolution | H3 grid resolution |
| `cell_size_km` | ~0.24 | f64 | Approximate cell size at resolution 9 (km) |

### Distance Calculation
**Deterministic** (Haversine formula)

```
distance_km = haversine_distance(cell1, cell2)
```

Uses H3 cell centroids (lat/lng) to calculate great-circle distance.

### Grid Distance
**Deterministic** (H3 grid topology)

```
grid_distance = h3o::CellIndex::grid_distance(cell1, cell2)
```

Number of H3 cell hops between two cells (used for match radius).

---

## Summary: Random vs Deterministic

### Random/Stochastic Elements
- ✅ Spawn inter-arrival times (exponential distribution, seeded)
- ✅ Rider/driver spawn positions (uniform within bounds, seeded)
- ✅ Rider destinations (uniform within distance range, seeded)
- ✅ Driver earnings targets ($100-$300, seeded)
- ✅ Driver fatigue thresholds (8-12 hours, seeded)
- ✅ Rider quote accept/reject decisions (when within limits, seeded)
- ✅ Rider pickup cancellation times (uniform, seeded)
- ✅ Vehicle speeds per movement step (20-60 km/h, seeded)

### Deterministic/Hard-Coded Elements
- ❌ Pricing formulas (base fare + distance × rate)
- ❌ Surge multiplier calculation (demand/supply ratio)
- ❌ Commission and earnings calculations
- ❌ Time-of-day spawn rate multipliers (hard-coded patterns)
- ❌ Matching algorithm scoring formulas
- ❌ Event scheduling delays (fixed 1-second delays)
- ❌ Distance calculations (Haversine)
- ❌ Movement time calculations (distance / speed)
- ❌ OffDuty threshold checks (earnings/fatigue)
- ❌ Quote rejection limits and give-up logic

### Reproducibility
All random elements use **seeded RNG** for reproducibility:
- Same seed → same sequence of random values
- Seeds are derived from scenario seed + entity IDs + offsets
- Allows deterministic simulation runs for testing and analysis
