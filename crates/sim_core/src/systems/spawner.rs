//! Spawner systems: react to spawn events and create riders/drivers dynamically.

use bevy_ecs::prelude::{Commands, Res, ResMut};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock, ONE_HOUR_MS};
use crate::ecs::{Driver, DriverFatigue, DriverEarnings, DriverState, Position, Rider, RiderState};
use crate::scenario::random_destination;
use crate::spatial::GeoIndex;
use crate::spawner::{random_cell_in_bounds, DriverSpawner, RiderSpawner};
use h3o::CellIndex;

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

/// Helper function to spawn a single rider.
fn spawn_rider(
    commands: &mut Commands,
    clock: &mut SimulationClock,
    spawner: &mut RiderSpawner,
    current_time_ms: u64,
) -> bevy_ecs::prelude::Entity {
    // Create RNG for position/destination generation
    let mut rng = create_spawn_rng(spawner.config.seed, spawner.spawned_count());

    // Generate position
    let position = generate_spawn_position(
        &mut rng,
        spawner.config.lat_min,
        spawner.config.lat_max,
        spawner.config.lng_min,
        spawner.config.lng_max,
    );

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
                state: RiderState::Browsing,
                matched_driver: None,
                destination: Some(destination),
                requested_at: Some(current_time_ms),
                quote_rejections: 0,
            },
            Position(position),
        ))
        .id();

    // Schedule ShowQuote event 1 second from now (quote with fare + ETA, then rider accept/reject)
    clock.schedule_in_secs(1, EventKind::ShowQuote, Some(EventSubject::Rider(rider_entity)));

    rider_entity
}

/// Helper function to spawn a single driver.
fn spawn_driver(
    commands: &mut Commands,
    spawner: &mut DriverSpawner,
    current_time_ms: u64,
) {
    // Create RNG for position generation
    let mut rng = create_spawn_rng(spawner.config.seed, spawner.spawned_count());

    // Generate position
    let position = generate_spawn_position(
        &mut rng,
        spawner.config.lat_min,
        spawner.config.lat_max,
        spawner.config.lng_min,
        spawner.config.lng_max,
    );

    // Sample earnings target: $100-$300 range
    let daily_earnings_target = rng.gen_range(100.0..=300.0);
    
    // Sample fatigue threshold: 8-12 hours
    let fatigue_hours = rng.gen_range(8.0..=12.0);
    let fatigue_threshold_ms = (fatigue_hours * ONE_HOUR_MS as f64) as u64;

    // Spawn the driver with earnings and fatigue components
    commands.spawn((
        Driver {
            state: DriverState::Idle,
            matched_rider: None,
        },
        Position(position),
        DriverEarnings {
            daily_earnings: 0.0,
            daily_earnings_target,
            session_start_time_ms: current_time_ms,
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
) {
    if !spawner.initialized() {
        spawner.set_initialized(true);
        
        // Spawn initial riders immediately
        for _ in 0..spawner.config.initial_count {
            spawn_rider(commands, clock, spawner, current_time_ms);
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
) {
    if !spawner.initialized() {
        spawner.set_initialized(true);
        
        // Spawn initial drivers immediately
        for _ in 0..spawner.config.initial_count {
            spawn_driver(commands, spawner, current_time_ms);
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
        spawn_rider(commands, clock, spawner, current_time_ms);

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
        spawn_driver(commands, spawner, current_time_ms);

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
    rider_spawner: Option<ResMut<RiderSpawner>>,
    driver_spawner: Option<ResMut<DriverSpawner>>,
    event: Res<CurrentEvent>,
) {
    if event.0.kind != EventKind::SimulationStarted {
        return;
    }

    let current_time_ms = clock.now();

    // Initialize rider spawner and spawn initial riders
    if let Some(mut spawner) = rider_spawner {
        initialize_rider_spawner(&mut *spawner, &mut commands, &mut clock, current_time_ms);
    }

    // Initialize driver spawner and spawn initial drivers
    if let Some(mut spawner) = driver_spawner {
        initialize_driver_spawner(&mut *spawner, &mut commands, &mut clock, current_time_ms);
    }
}

/// System that processes rider spawner and creates riders.
pub fn rider_spawner_system(
    mut commands: Commands,
    mut clock: ResMut<SimulationClock>,
    mut spawner: ResMut<RiderSpawner>,
    event: Res<CurrentEvent>,
) {
    if event.0.kind != EventKind::SpawnRider {
        return;
    }

    let current_time_ms = clock.now();
    process_rider_spawner_event(&mut *spawner, &mut commands, &mut clock, current_time_ms);
}

/// System that processes driver spawner and creates drivers.
pub fn driver_spawner_system(
    mut commands: Commands,
    mut clock: ResMut<SimulationClock>,
    mut spawner: ResMut<DriverSpawner>,
    event: Res<CurrentEvent>,
) {
    if event.0.kind != EventKind::SpawnDriver {
        return;
    }

    let current_time_ms = clock.now();
    process_driver_spawner_event(&mut *spawner, &mut commands, &mut clock, current_time_ms);
}
