//! Spawner systems: react to spawn events and create riders/drivers dynamically.

use bevy_ecs::prelude::{Commands, Res, ResMut};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::clock::{
    CurrentEvent, EventKind, EventSubject, SimulationClock, ONE_HOUR_MS, ONE_MIN_MS,
};
use crate::ecs::{
    Browsing, Driver, DriverEarnings, DriverFatigue, GeoPosition, Idle, Position, Rider,
};
#[cfg(feature = "osrm")]
use crate::routing::osrm_spawn::OsrmSpawnClient;
use crate::scenario::{random_destination, BatchMatchingConfig};
use crate::spatial::{cell_in_bounds, GeoIndex};
use crate::spawner::{random_cell_in_bounds, DriverSpawner, RiderSpawner, SpawnWeighting};
#[cfg(feature = "osrm")]
use h3o::Resolution;
use h3o::{CellIndex, LatLng};

#[cfg(feature = "osrm")]
type MaybeOsrmSpawnClient<'a> = Option<&'a OsrmSpawnClient>;
#[cfg(not(feature = "osrm"))]
type MaybeOsrmSpawnClient<'a> = ();

/// Create a seeded RNG for spawn operations.
/// Uses config seed + spawn count for deterministic but varied randomness.
fn create_spawn_rng(seed: u64, spawn_count: usize) -> StdRng {
    let combined_seed = seed.wrapping_add(spawn_count as u64);
    StdRng::seed_from_u64(combined_seed)
}

/// Generate a spawn position within the given bounds.
/// Falls back to center of bounds if coordinate generation fails.
///
/// # Safety
///
/// The `expect` call for fallback coordinates is safe because:
/// - We compute the center of valid geographic bounds (lat/lng in valid ranges)
/// - The bounds are validated when spawners are created (San Francisco Bay Area defaults)
/// - Center coordinates of valid bounds are always valid lat/lng values
fn generate_spawn_position<R: Rng>(
    rng: &mut R,
    lat_min: f64,
    lat_max: f64,
    lng_min: f64,
    lng_max: f64,
) -> CellIndex {
    random_cell_in_bounds(rng, lat_min, lat_max, lng_min, lng_max).unwrap_or_else(|_| {
        // Fallback to center of bounds if coordinate generation fails
        // This should not happen with valid bounds, but provides safety
        let lat = (lat_min + lat_max) / 2.0;
        let lng = (lng_min + lng_max) / 2.0;
        // Safe: center of valid geographic bounds is always valid
        let coord = h3o::LatLng::new(lat, lng)
            .expect("fallback coordinates should be valid (center of valid bounds)");
        coord.to_cell(h3o::Resolution::Nine)
    })
}

fn bounded_weighted_spawn_cell<R, F>(
    weighting: Option<&SpawnWeighting>,
    rng: &mut R,
    sampler: F,
    lat_min: f64,
    lat_max: f64,
    lng_min: f64,
    lng_max: f64,
) -> Option<CellIndex>
where
    R: Rng,
    F: Fn(&SpawnWeighting, &mut R) -> Option<CellIndex>,
{
    weighting
        .and_then(|w| sampler(w, rng))
        .filter(|cell| cell_in_bounds(*cell, lat_min, lat_max, lng_min, lng_max))
}

struct SpawnLocation {
    cell: CellIndex,
    geo: LatLng,
}

fn resolve_spawn_location<R, F>(
    rng: &mut R,
    weighting: Option<&SpawnWeighting>,
    sampler: F,
    lat_min: f64,
    lat_max: f64,
    lng_min: f64,
    lng_max: f64,
    _osrm_client: MaybeOsrmSpawnClient<'_>,
) -> SpawnLocation
where
    R: Rng,
    F: Fn(&SpawnWeighting, &mut R) -> Option<CellIndex>,
{
    let cell =
        bounded_weighted_spawn_cell(weighting, rng, sampler, lat_min, lat_max, lng_min, lng_max)
            .unwrap_or_else(|| generate_spawn_position(rng, lat_min, lat_max, lng_min, lng_max));

    let base_spawn_location = SpawnLocation {
        cell,
        geo: cell.into(),
    };
    #[cfg(feature = "osrm")]
    let mut spawn_location = base_spawn_location;
    #[cfg(not(feature = "osrm"))]
    let spawn_location = base_spawn_location;

    #[cfg(feature = "osrm")]
    if let Some(client) = _osrm_client {
        if let Some(snapped) = try_snap_spawn_location(
            client,
            rng,
            spawn_location.geo,
            lat_min,
            lat_max,
            lng_min,
            lng_max,
        ) {
            spawn_location.geo = snapped;
            spawn_location.cell = snapped.to_cell(Resolution::Nine);
        }
    }

    spawn_location
}

/// Helper function to spawn a single rider.
fn spawn_rider(
    commands: &mut Commands,
    clock: &mut SimulationClock,
    spawner: &mut RiderSpawner,
    current_time_ms: u64,
    weighting: Option<&SpawnWeighting>,
) -> bevy_ecs::prelude::Entity {
    // Create RNG for position/destination generation
    let mut rng = create_spawn_rng(spawner.config.seed, spawner.spawned_count());

    let lat_min = spawner.config.lat_min;
    let lat_max = spawner.config.lat_max;
    let lng_min = spawner.config.lng_min;
    let lng_max = spawner.config.lng_max;

    // Generate position: try weighted cell first, then fall back to uniform random
    let spawn_location = resolve_spawn_location(
        &mut rng,
        weighting,
        |w, rng| w.sample_rider_cell(rng),
        lat_min,
        lat_max,
        lng_min,
        lng_max,
        spawner.osrm_spawn_client(),
    );
    let position = spawn_location.cell;

    let geo = GeoIndex::default();
    let destination = random_destination(
        &mut rng,
        position,
        &geo,
        spawner.config.min_trip_cells,
        spawner.config.max_trip_cells,
        spawner.config.lat_min,
        spawner.config.lat_max,
        spawner.config.lng_min,
        spawner.config.lng_max,
    );

    // Spawn the rider
    let rider_entity = commands
        .spawn((
            Rider {
                matched_driver: None,
                assigned_trip: None,
                destination: Some(destination),
                requested_at: Some(current_time_ms),
                quote_rejections: 0,
                accepted_fare: None,
                last_rejection_reason: None,
            },
            Browsing,
            Position(position),
            GeoPosition(spawn_location.geo),
        ))
        .id();

    // Schedule ShowQuote event 1 second from now (quote with fare + ETA, then rider accept/reject)
    clock.schedule_in_secs(
        1,
        EventKind::ShowQuote,
        Some(EventSubject::Rider(rider_entity)),
    );

    rider_entity
}

/// Helper function to spawn a single driver.
fn spawn_driver(
    commands: &mut Commands,
    spawner: &mut DriverSpawner,
    current_time_ms: u64,
    weighting: Option<&SpawnWeighting>,
) {
    // Create RNG for position generation
    let mut rng = create_spawn_rng(spawner.config.seed, spawner.spawned_count());

    let lat_min = spawner.config.lat_min;
    let lat_max = spawner.config.lat_max;
    let lng_min = spawner.config.lng_min;
    let lng_max = spawner.config.lng_max;

    // Generate position: try weighted cell first, then fall back to uniform random
    let spawn_location = resolve_spawn_location(
        &mut rng,
        weighting,
        |w, rng| w.sample_driver_cell(rng),
        lat_min,
        lat_max,
        lng_min,
        lng_max,
        spawner.osrm_spawn_client(),
    );
    let position = spawn_location.cell;

    // Sample earnings target: $100-$300 range
    let daily_earnings_target = rng.gen_range(100.0..=300.0);

    // Sample fatigue threshold: 8-12 hours
    let fatigue_hours = rng.gen_range(8.0..=12.0);
    let fatigue_threshold_ms = (fatigue_hours * ONE_HOUR_MS as f64) as u64;

    // Spawn the driver with earnings and fatigue components
    commands.spawn((
        Driver {
            matched_rider: None,
            assigned_trip: None,
        },
        Idle,
        Position(position),
        GeoPosition(spawn_location.geo),
        DriverEarnings {
            daily_earnings: 0.0,
            daily_earnings_target,
            session_start_time_ms: current_time_ms,
            session_end_time_ms: None,
        },
        DriverFatigue {
            fatigue_threshold_ms,
        },
    ));
}

/// Initialize rider spawner and spawn initial riders.
fn initialize_rider_spawner(
    spawner: &mut RiderSpawner,
    commands: &mut Commands,
    clock: &mut SimulationClock,
    current_time_ms: u64,
    weighting: Option<&SpawnWeighting>,
) {
    if !spawner.initialized() {
        spawner.set_initialized(true);

        // Spawn initial riders immediately
        for _ in 0..spawner.config.initial_count {
            spawn_rider(commands, clock, spawner, current_time_ms, weighting);
            // Manually increment count since we're not calling advance() for initial spawns
            spawner.increment_spawned_count();
        }

        // Schedule first spawn event at next_spawn_time_ms (even if it's in the future)
        if spawner.should_spawn(spawner.next_spawn_time_ms()) {
            clock.schedule_at(spawner.next_spawn_time_ms(), EventKind::SpawnRider, None);
        }
    }
}

/// Initialize driver spawner and spawn initial drivers.
fn initialize_driver_spawner(
    spawner: &mut DriverSpawner,
    commands: &mut Commands,
    clock: &mut SimulationClock,
    current_time_ms: u64,
    weighting: Option<&SpawnWeighting>,
) {
    if !spawner.initialized() {
        spawner.set_initialized(true);

        // Spawn initial drivers immediately
        for _ in 0..spawner.config.initial_count {
            spawn_driver(commands, spawner, current_time_ms, weighting);
            // Manually increment count since we're not calling advance() for initial spawns
            spawner.increment_spawned_count();
        }

        // Schedule first spawn event at next_spawn_time_ms (even if it's in the future)
        if spawner.should_spawn(spawner.next_spawn_time_ms()) {
            clock.schedule_at(spawner.next_spawn_time_ms(), EventKind::SpawnDriver, None);
        }
    }
}

/// Process rider spawner event and create riders.
fn process_rider_spawner_event(
    spawner: &mut RiderSpawner,
    commands: &mut Commands,
    clock: &mut SimulationClock,
    current_time_ms: u64,
    weighting: Option<&SpawnWeighting>,
) {
    // Check if we're before start time (shouldn't happen, but be safe)
    if let Some(start_time) = spawner.config.start_time_ms {
        if current_time_ms < start_time {
            spawner.set_next_spawn_time_ms(start_time);
            clock.schedule_at(start_time, EventKind::SpawnRider, None);
            return;
        }
    }

    // Check if we should spawn
    if spawner.should_spawn(current_time_ms) {
        spawn_rider(commands, clock, spawner, current_time_ms, weighting);

        // Advance spawner to next spawn time (uses seeded distribution internally)
        spawner.advance(current_time_ms);

        // Schedule next spawn event if we should continue spawning
        if spawner.should_spawn(spawner.next_spawn_time_ms()) {
            clock.schedule_at(spawner.next_spawn_time_ms(), EventKind::SpawnRider, None);
        }
    }
}

/// Process driver spawner event and create drivers.
fn process_driver_spawner_event(
    spawner: &mut DriverSpawner,
    commands: &mut Commands,
    clock: &mut SimulationClock,
    current_time_ms: u64,
    weighting: Option<&SpawnWeighting>,
) {
    // Check if we're before start time (shouldn't happen, but be safe)
    if let Some(start_time) = spawner.config.start_time_ms {
        if current_time_ms < start_time {
            spawner.set_next_spawn_time_ms(start_time);
            clock.schedule_at(start_time, EventKind::SpawnDriver, None);
            return;
        }
    }

    // Check if we should spawn
    if spawner.should_spawn(current_time_ms) {
        spawn_driver(commands, spawner, current_time_ms, weighting);

        // Advance spawner to next spawn time (uses seeded distribution internally)
        spawner.advance(current_time_ms);

        // Schedule next spawn event if we should continue spawning
        if spawner.should_spawn(spawner.next_spawn_time_ms()) {
            clock.schedule_at(spawner.next_spawn_time_ms(), EventKind::SpawnDriver, None);
        }
    }
}

/// System that reacts to SimulationStarted event and initializes spawners.
pub fn simulation_started_system(
    mut commands: Commands,
    mut clock: ResMut<SimulationClock>,
    batch_config: Option<Res<BatchMatchingConfig>>,
    rider_spawner: Option<ResMut<RiderSpawner>>,
    driver_spawner: Option<ResMut<DriverSpawner>>,
    spawn_weighting: Option<Res<SpawnWeighting>>,
    event: Res<CurrentEvent>,
) {
    if event.0.kind != EventKind::SimulationStarted {
        return;
    }

    let current_time_ms = clock.now();
    let weighting = spawn_weighting.as_deref();

    // When batch matching is enabled, schedule the first BatchMatchRun at time 0
    if let Some(cfg) = batch_config.as_deref() {
        if cfg.enabled {
            clock.schedule_at(0, EventKind::BatchMatchRun, None);
        }
    }

    // Initialize rider spawner and spawn initial riders
    if let Some(mut spawner) = rider_spawner {
        initialize_rider_spawner(
            &mut spawner,
            &mut commands,
            &mut clock,
            current_time_ms,
            weighting,
        );
    }

    // Initialize driver spawner and spawn initial drivers
    if let Some(mut spawner) = driver_spawner {
        initialize_driver_spawner(
            &mut spawner,
            &mut commands,
            &mut clock,
            current_time_ms,
            weighting,
        );
    }

    // Schedule the first periodic OffDuty check (every 5 minutes)
    clock.schedule_in(5 * ONE_MIN_MS, EventKind::CheckDriverOffDuty, None);
}

/// System that processes rider spawner and creates riders.
pub fn rider_spawner_system(
    mut commands: Commands,
    mut clock: ResMut<SimulationClock>,
    mut spawner: ResMut<RiderSpawner>,
    spawn_weighting: Option<Res<SpawnWeighting>>,
    event: Res<CurrentEvent>,
) {
    if event.0.kind != EventKind::SpawnRider {
        return;
    }

    let current_time_ms = clock.now();
    let weighting = spawn_weighting.as_deref();
    process_rider_spawner_event(
        &mut spawner,
        &mut commands,
        &mut clock,
        current_time_ms,
        weighting,
    );
}

/// System that processes driver spawner and creates drivers.
pub fn driver_spawner_system(
    mut commands: Commands,
    mut clock: ResMut<SimulationClock>,
    mut spawner: ResMut<DriverSpawner>,
    spawn_weighting: Option<Res<SpawnWeighting>>,
    event: Res<CurrentEvent>,
) {
    if event.0.kind != EventKind::SpawnDriver {
        return;
    }

    let current_time_ms = clock.now();
    let weighting = spawn_weighting.as_deref();
    process_driver_spawner_event(
        &mut spawner,
        &mut commands,
        &mut clock,
        current_time_ms,
        weighting,
    );
}

#[cfg(feature = "osrm")]
const SPAWN_TRACE_JITTER_DEG: f64 = 0.00035;

#[cfg(feature = "osrm")]
fn try_snap_spawn_location<R: Rng>(
    client: &OsrmSpawnClient,
    rng: &mut R,
    candidate: LatLng,
    lat_min: f64,
    lat_max: f64,
    lng_min: f64,
    lng_max: f64,
) -> Option<LatLng> {
    let trace = build_spawn_trace(rng, candidate, lat_min, lat_max, lng_min, lng_max);
    if trace.is_empty() {
        return None;
    }

    let matching = client.snap_with_defaults(&trace).ok()?;
    let coordinate = matching.coordinate;
    if coordinate_within_bounds(&coordinate, lat_min, lat_max, lng_min, lng_max) {
        Some(coordinate)
    } else {
        None
    }
}

#[cfg(feature = "osrm")]
fn build_spawn_trace<R: Rng>(
    rng: &mut R,
    center: LatLng,
    lat_min: f64,
    lat_max: f64,
    lng_min: f64,
    lng_max: f64,
) -> Vec<LatLng> {
    let mut trace = Vec::with_capacity(3);
    trace.push(center);
    for _ in 0..2 {
        let lat = clamp_latlng(
            center.lat() + rng.gen_range(-SPAWN_TRACE_JITTER_DEG..=SPAWN_TRACE_JITTER_DEG),
            lat_min,
            lat_max,
            -90.0,
            90.0,
        );
        let lng = clamp_latlng(
            center.lng() + rng.gen_range(-SPAWN_TRACE_JITTER_DEG..=SPAWN_TRACE_JITTER_DEG),
            lng_min,
            lng_max,
            -180.0,
            180.0,
        );
        if let Ok(point) = LatLng::new(lat, lng) {
            trace.push(point);
        }
    }
    trace
}

#[cfg(feature = "osrm")]
fn clamp_latlng(
    value: f64,
    min_bound: f64,
    max_bound: f64,
    global_min: f64,
    global_max: f64,
) -> f64 {
    let bounded = value.clamp(global_min, global_max);
    bordered_clamp(bounded, min_bound, max_bound)
}

#[cfg(feature = "osrm")]
fn bordered_clamp(value: f64, min_bound: f64, max_bound: f64) -> f64 {
    let min = min_bound.min(max_bound);
    let max = min_bound.max(max_bound);
    value.clamp(min, max)
}

#[cfg(feature = "osrm")]
fn coordinate_within_bounds(
    location: &LatLng,
    lat_min: f64,
    lat_max: f64,
    lng_min: f64,
    lng_max: f64,
) -> bool {
    let lat = location.lat();
    let lng = location.lng();
    lat >= lat_min && lat <= lat_max && lng >= lng_min && lng <= lng_max
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn bounded_weighted_spawn_cell_rejects_out_of_bounds() {
        let weighting = SpawnWeighting::berlin_hotspots();
        let mut rng = StdRng::seed_from_u64(0);
        let result = bounded_weighted_spawn_cell(
            Some(&weighting),
            &mut rng,
            |w, rng| w.sample_rider_cell(rng),
            0.0,
            1.0,
            0.0,
            1.0,
        );
        assert!(result.is_none());
    }

    #[test]
    fn bounded_weighted_spawn_cell_allows_in_bounds() {
        let weighting = SpawnWeighting::berlin_hotspots();
        let mut rng = StdRng::seed_from_u64(0);
        let result = bounded_weighted_spawn_cell(
            Some(&weighting),
            &mut rng,
            |w, rng| w.sample_rider_cell(rng),
            52.2,
            52.6,
            13.0,
            13.6,
        );
        assert!(result.is_some());
    }
}
