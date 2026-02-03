//! Spawner systems: react to spawn events and create riders/drivers dynamically.

use bevy_ecs::prelude::{Commands, Res, ResMut};
use rand::rngs::StdRng;
use rand::SeedableRng;

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use crate::ecs::{Driver, DriverState, Position, Rider, RiderState};
use crate::scenario::random_destination;
use crate::spatial::GeoIndex;
use crate::spawner::{random_cell_in_bounds, DriverSpawner, RiderSpawner};

/// System that reacts to SimulationStarted event and initializes spawners.
pub fn simulation_started_system(
    mut clock: ResMut<SimulationClock>,
    rider_spawner: Option<ResMut<RiderSpawner>>,
    driver_spawner: Option<ResMut<DriverSpawner>>,
    event: Res<CurrentEvent>,
) {
    if event.0.kind != EventKind::SimulationStarted {
        return;
    }

    // Initialize rider spawner
    if let Some(mut spawner) = rider_spawner {
        if !spawner.initialized {
            spawner.initialized = true;
            // Schedule first spawn event at next_spawn_time_ms (even if it's in the future)
            clock.schedule_at(
                spawner.next_spawn_time_ms,
                EventKind::SpawnRider,
                None,
            );
        }
    }

    // Initialize driver spawner
    if let Some(mut spawner) = driver_spawner {
        if !spawner.initialized {
            spawner.initialized = true;
            // Schedule first spawn event at next_spawn_time_ms (even if it's in the future)
            clock.schedule_at(
                spawner.next_spawn_time_ms,
                EventKind::SpawnDriver,
                None,
            );
        }
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

    // Check if we're before start time (shouldn't happen, but be safe)
    if let Some(start_time) = spawner.config.start_time_ms {
        if current_time_ms < start_time {
            spawner.next_spawn_time_ms = start_time;
            clock.schedule_at(start_time, EventKind::SpawnRider, None);
            return;
        }
    }

    // Check if we should spawn
    if spawner.should_spawn(current_time_ms) {
        // Create RNG for position/destination generation (seed from config seed + spawn count for determinism)
        let seed = spawner.config.seed.wrapping_add(spawner.spawned_count as u64);
        let mut rng = StdRng::seed_from_u64(seed);

        // Generate position and destination
        let position = random_cell_in_bounds(
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
                },
                Position(position),
            ))
            .id();

        // Schedule QuoteAccepted event 1 second from now
        clock.schedule_in_secs(1, EventKind::QuoteAccepted, Some(EventSubject::Rider(rider_entity)));

        // Advance spawner to next spawn time (uses seeded distribution internally)
        spawner.advance(current_time_ms);
        
        // Schedule next spawn event if we should continue spawning
        if spawner.should_spawn(spawner.next_spawn_time_ms) {
            clock.schedule_at(
                spawner.next_spawn_time_ms,
                EventKind::SpawnRider,
                None,
            );
        }
    }
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

    // Check if we're before start time (shouldn't happen, but be safe)
    if let Some(start_time) = spawner.config.start_time_ms {
        if current_time_ms < start_time {
            spawner.next_spawn_time_ms = start_time;
            clock.schedule_at(start_time, EventKind::SpawnDriver, None);
            return;
        }
    }

    // Check if we should spawn
    if spawner.should_spawn(current_time_ms) {
        // Create RNG for position generation (seed from config seed + spawn count for determinism)
        let seed = spawner.config.seed.wrapping_add(spawner.spawned_count as u64);
        let mut rng = StdRng::seed_from_u64(seed);

        // Generate position
        let position = random_cell_in_bounds(
            &mut rng,
            spawner.config.lat_min,
            spawner.config.lat_max,
            spawner.config.lng_min,
            spawner.config.lng_max,
        );

        // Spawn the driver
        commands.spawn((
            Driver {
                state: DriverState::Idle,
                matched_rider: None,
            },
            Position(position),
        ));

        // Advance spawner to next spawn time (uses seeded distribution internally)
        spawner.advance(current_time_ms);
        
        // Schedule next spawn event if we should continue spawning
        if spawner.should_spawn(spawner.next_spawn_time_ms) {
            clock.schedule_at(
                spawner.next_spawn_time_ms,
                EventKind::SpawnDriver,
                None,
            );
        }
    }
}
